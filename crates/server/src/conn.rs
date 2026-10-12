//! One WebSocket connection: the hello handshake, creating/joining/leaving rooms, and passing
//! everything else to the player's room.

use crate::room::{Joined, RoomCmd};
use crate::{ConnectionSlot, Hub};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use go_box_protocol::{ClientMsg, MAX_NAME_LEN, PROTOCOL_VERSION, PlayerId, Refusal, ServerMsg, decode, encode};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// Outgoing messages queued per client. Snapshots are dropped if a client falls this far behind.
pub const OUTBOX: usize = 64;

/// A client's outbox: encoded ServerMsgs.
pub type Outbox = mpsc::Sender<Vec<u8>>;

pub async fn run(socket: WebSocket, hub: Arc<Hub>) {
    let (mut sink, mut stream) = socket.split();
    let Some(_slot) = ConnectionSlot::take(&hub) else {
        let _ = sink.send(Message::Binary(encode(&ServerMsg::Error(Refusal::ServerFull)).into())).await;
        return;
    };
    let (out, mut outbox) = mpsc::channel::<Vec<u8>>(OUTBOX);
    let writer = tokio::spawn(async move {
        while let Some(bytes) = outbox.recv().await {
            if sink.send(Message::Binary(bytes.into())).await.is_err() {
                break;
            }
        }
        let _ = sink.close().await;
    });

    // The first message must be a Hello on our protocol version, and soon.
    let hello = tokio::time::timeout(Duration::from_secs(10), next_msg(&mut stream)).await;
    let name = match hello {
        Ok(Some(ClientMsg::Hello { version, name })) if version == PROTOCOL_VERSION => clean_name(&name),
        Ok(Some(ClientMsg::Hello { .. })) => {
            let _ = out.send(encode(&ServerMsg::Error(Refusal::OutOfDate))).await;
            drop(out);
            let _ = writer.await;
            return;
        }
        _ => return,
    };
    let id: PlayerId = hub.next_id.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    tracing::debug!(id, %name, "connected");
    let _ = out.send(encode(&ServerMsg::Welcome { you: id })).await;

    let mut room: Option<mpsc::Sender<RoomCmd>> = None;
    while let Some(msg) = next_msg(&mut stream).await {
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::CreateRoom | ClientMsg::JoinRoom { .. } => {
                if let Some(r) = room.take() {
                    let _ = r.send(RoomCmd::Leave { id }).await;
                }
                let target = match &msg {
                    ClientMsg::CreateRoom => hub.create_room().map(|(_, tx)| tx).ok_or(Refusal::TooManyRooms),
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
async fn next_msg(stream: &mut futures_util::stream::SplitStream<WebSocket>) -> Option<ClientMsg> {
    loop {
        match stream.next().await? {
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
