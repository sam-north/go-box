//! The connection to the server: a WebSocket carrying one encoded message per binary frame.
//! Never blocks the frame: `poll` returns whatever has arrived.

use go_box_protocol::{ClientMsg, ServerMsg, decode, encode};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Connecting,
    Open,
    Closed,
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::State;

    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn go_ws_open();
        fn go_ws_state() -> i32;
        fn go_ws_send(ptr: *const u8, len: u32);
        fn go_ws_next_len() -> i32;
        fn go_ws_take(ptr: *mut u8);
    }

    /// The page's own server (same host, `/ws`).
    pub struct Socket;

    impl Socket {
        pub fn open() -> Self {
            unsafe { go_ws_open() };
            Self
        }

        pub fn state(&self) -> State {
            match unsafe { go_ws_state() } {
                0 => State::Connecting,
                1 => State::Open,
                _ => State::Closed,
            }
        }

        pub fn send(&mut self, bytes: &[u8]) {
            unsafe { go_ws_send(bytes.as_ptr(), bytes.len() as u32) }
        }

        pub fn recv(&mut self) -> Option<Vec<u8>> {
            let len = unsafe { go_ws_next_len() };
            if len < 0 {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            unsafe { go_ws_take(buf.as_mut_ptr()) };
            Some(buf)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::State;
    use std::io::ErrorKind;
    use std::net::TcpStream;
    use std::sync::mpsc;
    use std::time::Duration;
    use tungstenite::{Message, WebSocket};

    type Ws = WebSocket<TcpStream>;

    /// A native WebSocket; connects on a background thread, then runs non-blocking.
    pub struct Socket {
        connecting: Option<mpsc::Receiver<Option<Ws>>>,
        ws: Option<Ws>,
        closed: bool,
    }

    impl Socket {
        pub fn open() -> Self {
            let url = crate::Options::get().server.clone();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let _ = tx.send(connect(&url));
            });
            Self { connecting: Some(rx), ws: None, closed: false }
        }

        fn settle(&mut self) {
            if let Some(rx) = &self.connecting {
                match rx.try_recv() {
                    Ok(ws) => {
                        self.closed = ws.is_none();
                        self.ws = ws;
                        self.connecting = None;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.closed = true;
                        self.connecting = None;
                    }
                }
            }
        }

        pub fn state(&mut self) -> State {
            self.settle();
            if self.closed {
                State::Closed
            } else if self.ws.is_some() {
                State::Open
            } else {
                State::Connecting
            }
        }

        pub fn send(&mut self, bytes: &[u8]) {
            let Some(ws) = &mut self.ws else { return };
            match ws.send(Message::Binary(bytes.to_vec().into())) {
                Ok(()) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(_) => self.closed = true,
            }
        }

        pub fn recv(&mut self) -> Option<Vec<u8>> {
            self.settle();
            let ws = self.ws.as_mut()?;
            let _ = ws.flush();
            loop {
                match ws.read() {
                    Ok(Message::Binary(b)) => return Some(b.to_vec()),
                    Ok(Message::Close(_)) => {
                        self.closed = true;
                        return None;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock => return None,
                    Err(_) => {
                        self.closed = true;
                        return None;
                    }
                }
            }
        }
    }

    fn connect(url: &str) -> Option<Ws> {
        let uri: tungstenite::http::Uri = url.parse().ok()?;
        let addr = format!("{}:{}", uri.host()?, uri.port_u16().unwrap_or(80));
        let stream = TcpStream::connect(&addr).ok()?;
        stream.set_nodelay(true).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
        let (ws, _) = tungstenite::client(url, stream).ok()?;
        ws.get_ref().set_nonblocking(true).ok()?;
        Some(ws)
    }
}

pub struct Net {
    socket: imp::Socket,
}

impl Net {
    pub fn connect() -> Self {
        Self { socket: imp::Socket::open() }
    }

    pub fn state(&mut self) -> State {
        self.socket.state()
    }

    pub fn send(&mut self, msg: &ClientMsg) {
        self.socket.send(&encode(msg));
    }

    pub fn poll(&mut self) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        while let Some(bytes) = self.socket.recv() {
            if let Some(msg) = decode(&bytes) {
                out.push(msg);
            }
        }
        out
    }
}
