//! One WebSocket connection: the hello handshake, creating/joining/leaving rooms, and passing
//! everything else to the player's room.

use crate::room::{Joined, RoomCmd};
use crate::{ConnectionSlot, Hub};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use go_box_protocol::{CLOSE_OUT_OF_DATE, CLOSE_SERVER_FULL, ClientMsg, MAX_NAME_LEN, PROTOCOL_VERSION, PlayerId, Refusal, ServerMsg, decode, encode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// The server pings every connection this often; one silent for IDLE_TIMEOUT is dropped.
const PING_EVERY: Duration = Duration::from_secs(15);
const IDLE_TIMEOUT: Duration = Duration::from_secs(45);
/// Per-connection message limits (see `Limits`).
const INPUTS_PER_SEC: f32 = 120.0;
const OTHER_PER_SEC: f32 = 10.0;
const OTHER_BURST: f32 = 20.0;
const MAX_BAD_CODES: u32 = 5;

/// Outgoing messages queued per client. Snapshots are dropped if a client falls this far behind.
pub const OUTBOX: usize = 64;

/// A client's outbox: encoded ServerMsgs.
pub type Outbox = mpsc::Sender<Vec<u8>>;

pub async fn run(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sink, mut stream) = socket.split();
    let Some(_slot) = ConnectionSlot::take(&hub) else {
        close(&mut sink, CLOSE_SERVER_FULL).await;
        return;
    };

    // The first message must be a Hello on our protocol version, and soon. Anything else (a
    // client on another version, whose messages may not even decode) is turned away with a close
    // code, which every version can read.
    let first = tokio::time::timeout(Duration::from_secs(10), next_frame(&mut stream)).await;
    let name = match first {
        Ok(Some(bytes)) => match decode::<ClientMsg>(&bytes) {
            Some(ClientMsg::Hello { version, name }) if version == PROTOCOL_VERSION => clean_name(&name),
            _ => {
                close(&mut sink, CLOSE_OUT_OF_DATE).await;
                return;
            }
        },
        _ => return,
    };

    let (out, mut outbox) = mpsc::channel::<Vec<u8>>(OUTBOX);
    let writer = tokio::spawn(async move {
        // Also pings now and then, so a connection that's gone quiet (a closed laptop, a dead
        // phone tab) shows up as silence and gets dropped. Browsers answer pings by themselves.
        let mut ping = tokio::time::interval(PING_EVERY);
        loop {
            let msg = tokio::select! {
                bytes = outbox.recv() => match bytes {
                    Some(bytes) => Message::Binary(bytes.into()),
                    None => break,
                },
                _ = ping.tick() => Message::Ping(Default::default()),
            };
            if sink.send(msg).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let id: PlayerId = hub.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    tracing::debug!(id, %name, "connected");
    let _ = out.send(encode(&ServerMsg::Welcome { you: id })).await;

    let mut room: Option<mpsc::Sender<RoomCmd>> = None;
    let mut limits = Limits::new();
    while let Some(msg) = next_msg(&mut stream).await {
        match limits.allow(&msg) {
            Ok(true) => {}
            // An input over the limit is just ignored: a burst is usually a network stall
            // catching up, not abuse, and dropping the player for it would be worse.
            Ok(false) => continue,
            Err(why) => {
                tracing::info!(id, why, "dropping a connection over its limits");
                break;
            }
        }
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::CreateRoom { .. } | ClientMsg::JoinRoom { .. } => {
                if let Some(r) = room.take() {
                    let _ = r.send(RoomCmd::Leave { id }).await;
                }
                let target = match &msg {
                    ClientMsg::CreateRoom { passphrase } if !hub.may_create(passphrase) => Err(Refusal::WrongPassphrase),
                    ClientMsg::CreateRoom { .. } => hub.create_room().map(|(_, tx)| tx).ok_or(Refusal::TooManyRooms),
                    ClientMsg::JoinRoom { code } => hub.find_room(code).ok_or(Refusal::NoSuchRoom),
                    _ => unreachable!(),
                };
                let joined = match target {
                    Ok(tx) => join(&tx, id, &name, &out).await.map(|()| tx),
                    Err(e) => Err(e),
                };
                match joined {
                    Ok(tx) => room = Some(tx),
                    Err(e) => {
                        // Guessing either is limited the same way.
                        if matches!(e, Refusal::NoSuchRoom | Refusal::WrongPassphrase) {
                            limits.bad_codes += 1;
                        }
                        let _ = out.send(encode(&ServerMsg::Error(e))).await;
                    }
                }
            }
            ClientMsg::LeaveRoom => {
                if let Some(r) = room.take() {
                    let _ = r.send(RoomCmd::Leave { id }).await;
                }
                let _ = out.send(encode(&ServerMsg::LeftRoom)).await;
            }
            other => {
                if let Some(r) = &room {
                    // Inputs are frequent and disposable: never wait on a busy room for one.
                    let input = matches!(other, ClientMsg::Input { .. });
                    let cmd = RoomCmd::Msg { id, msg: other };
                    let gone = if input { r.try_send(cmd).is_err() && r.is_closed() } else { r.send(cmd).await.is_err() };
                    if gone {
                        room = None;
                    }
                }
            }
        }
    }
    if let Some(r) = room {
        let _ = r.send(RoomCmd::Leave { id }).await;
    }
    tracing::debug!(id, "disconnected");
    drop(out);
    writer.abort();
}

async fn join(room: &mpsc::Sender<RoomCmd>, id: PlayerId, name: &str, out: &Outbox) -> Result<(), Refusal> {
    let (reply, answer) = oneshot::channel();
    let cmd = RoomCmd::Join(Joined { id, name: name.to_string(), out: out.clone(), reply });
    if room.send(cmd).await.is_err() {
        return Err(Refusal::RoomClosed);
    }
    answer.await.unwrap_or(Err(Refusal::RoomClosed))
}

/// Next decodable client message; None when the connection ends. Undecodable frames are
/// skipped (a mismatched or hostile client can't crash anything).
/// The next binary frame's bytes (None if the connection closed).
async fn next_frame(stream: &mut futures_util::stream::SplitStream<WebSocket>) -> Option<Vec<u8>> {
    loop {
        match stream.next().await? {
            Ok(Message::Binary(bytes)) => return Some(bytes.to_vec()),
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// Closes the connection with one of the protocol's close codes.
async fn close(sink: &mut futures_util::stream::SplitSink<WebSocket, Message>, code: u16) {
    let frame = axum::extract::ws::CloseFrame { code, reason: "".into() };
    let _ = sink.send(Message::Close(Some(frame))).await;
}

/// The next message, or None when the connection closes or has been silent for IDLE_TIMEOUT
/// (anything counts as a sign of life, including the answers to our pings).
async fn next_msg(stream: &mut futures_util::stream::SplitStream<WebSocket>) -> Option<ClientMsg> {
    loop {
        match tokio::time::timeout(IDLE_TIMEOUT, stream.next()).await.ok()?? {
            Ok(Message::Binary(bytes)) => {
                if let Some(msg) = decode::<ClientMsg>(&bytes) {
                    return Some(msg);
                }
            }
            Ok(Message::Close(_)) | Err(_) => return None,
            Ok(_) => {}
        }
    }
}

/// Trims, drops control characters, caps the length; never empty.
pub fn clean_name(name: &str) -> String {
    let name: String = name.chars().filter(|c| !c.is_control()).take(MAX_NAME_LEN).collect();
    let name = name.trim();
    if name.is_empty() { "Player".to_string() } else { name.to_string() }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_are_cleaned() {
        assert_eq!(super::clean_name("  Sam\n "), "Sam");
        assert_eq!(super::clean_name("\u{7}\u{8}"), "Player");
        assert_eq!(super::clean_name("abcdefghijklmnopqrstuvwxyz").chars().count(), 16);
    }
}

/// Each connection's allowance. Inputs over it are ignored (they arrive 60 times a second, but a
/// network stall can deliver seconds of them at once); anything else over it closes the
/// connection (it's all clicks, so no honest client comes close).
struct Limits {
    inputs: Bucket,
    other: Bucket,
    /// Wrong room codes and creation passphrases tried (guessing either must be slow).
    bad_codes: u32,
}

impl Limits {
    fn new() -> Self {
        Self { inputs: Bucket::new(INPUTS_PER_SEC, INPUTS_PER_SEC), other: Bucket::new(OTHER_PER_SEC, OTHER_BURST), bad_codes: 0 }
    }

    /// Ok(true) if `msg` is within the limits, Ok(false) for an input over them (ignore it), else
    /// why the connection should close.
    fn allow(&mut self, msg: &ClientMsg) -> Result<bool, &'static str> {
        if self.bad_codes >= MAX_BAD_CODES {
            return Err("too many wrong room codes or passphrases");
        }
        if matches!(msg, ClientMsg::Input { .. }) {
            return Ok(self.inputs.take());
        }
        if self.other.take() { Ok(true) } else { Err("too many messages") }
    }
}

/// A token bucket: `rate` tokens a second, holding at most `burst`.
struct Bucket {
    tokens: f32,
    rate: f32,
    burst: f32,
    last: tokio::time::Instant,
}

impl Bucket {
    fn new(rate: f32, burst: f32) -> Self {
        Self { tokens: burst, rate, burst, last: tokio::time::Instant::now() }
    }

    fn take(&mut self) -> bool {
        let now = tokio::time::Instant::now();
        self.tokens = (self.tokens + (now - self.last).as_secs_f32() * self.rate).min(self.burst);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
