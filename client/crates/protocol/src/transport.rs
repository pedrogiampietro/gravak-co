//! The byte stream a session runs on: TCP where there are sockets, and in
//! the browser a WebSocket to Gravak's bridge (`crates/ws-bridge`), which
//! carries the same bytes to the server's TCP port. The TQ cipher and
//! framing stay in the client either way.

#[cfg(not(target_arch = "wasm32"))]
pub type Stream = tokio::net::TcpStream;

#[cfg(not(target_arch = "wasm32"))]
pub async fn connect(address: &str) -> std::io::Result<Stream> {
    tokio::net::TcpStream::connect(address).await
}

#[cfg(target_arch = "wasm32")]
pub use web::{Stream, connect, set_bridge};

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{
        cell::RefCell,
        collections::VecDeque,
        io,
        pin::Pin,
        rc::Rc,
        task::{Context, Poll, Waker},
    };
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{BinaryType, MessageEvent, WebSocket};

    thread_local! {
        static BRIDGE: RefCell<String> = RefCell::new("ws://127.0.0.1:9960".into());
    }

    /// Where the bridge listens, e.g. `wss://gravak.fun/ws`; a server's
    /// port is added to it (`wss://gravak.fun/ws/9958`).
    pub fn set_bridge(url: &str) {
        BRIDGE.with(|bridge| *bridge.borrow_mut() = url.trim_end_matches('/').to_owned());
    }

    #[derive(Default)]
    struct Shared {
        open: bool,
        closed: bool,
        received: VecDeque<u8>,
        reader: Option<Waker>,
        opener: Option<Waker>,
    }

    /// A WebSocket as a tokio byte stream.
    pub struct Stream {
        socket: WebSocket,
        shared: Rc<RefCell<Shared>>,
        // Kept alive as long as the socket calls them.
        _handlers: [Closure<dyn FnMut(web_sys::Event)>; 3],
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            self.socket.set_onopen(None);
            self.socket.set_onmessage(None);
            self.socket.set_onclose(None);
            self.socket.set_onerror(None);
            let _ = self.socket.close();
        }
    }

    /// `address` is the server's `host:port`; the bridge reaches that
    /// port on the server's own machine.
    pub async fn connect(address: &str) -> io::Result<Stream> {
        let port = address
            .rsplit_once(':')
            .map(|(_, port)| port)
            .filter(|port| port.parse::<u16>().is_ok())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("no port in {address}")))?;
        let url = BRIDGE.with(|bridge| format!("{}/{port}", bridge.borrow()));
        let failed = |error: wasm_bindgen::JsValue| io::Error::other(format!("{url}: {error:?}"));
        let socket = WebSocket::new(&url).map_err(failed)?;
        socket.set_binary_type(BinaryType::Arraybuffer);
        let shared = Rc::new(RefCell::new(Shared::default()));
        let wake = |shared: &Rc<RefCell<Shared>>| {
            let mut shared = shared.borrow_mut();
            if let Some(waker) = shared.reader.take() {
                waker.wake();
            }
            if let Some(waker) = shared.opener.take() {
                waker.wake();
            }
        };
        let on_open = {
            let shared = shared.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                shared.borrow_mut().open = true;
                wake(&shared);
            })
        };
        let on_message = {
            let shared = shared.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
                let Some(event) = event.dyn_ref::<MessageEvent>() else { return };
                if let Ok(buffer) = event.data().dyn_into::<js_sys::ArrayBuffer>() {
                    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
                    shared.borrow_mut().received.extend(bytes);
                    wake(&shared);
                }
            })
        };
        // An error is always followed by a close.
        let on_close = {
            let shared = shared.clone();
            Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
                shared.borrow_mut().closed = true;
                wake(&shared);
            })
        };
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        let stream = Stream { socket, shared, _handlers: [on_open, on_message, on_close] };
        std::future::poll_fn(|context| {
            let mut shared = stream.shared.borrow_mut();
            if shared.open {
                Poll::Ready(Ok(()))
            } else if shared.closed {
                Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("{url}: the bridge or the server did not answer"),
                )))
            } else {
                shared.opener = Some(context.waker().clone());
                Poll::Pending
            }
        })
        .await?;
        Ok(stream)
    }

    impl AsyncRead for Stream {
        fn poll_read(self: Pin<&mut Self>, context: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
            let mut shared = self.shared.borrow_mut();
            if !shared.received.is_empty() {
                let count = buf.remaining().min(shared.received.len());
                let bytes: Vec<u8> = shared.received.drain(..count).collect();
                buf.put_slice(&bytes);
                Poll::Ready(Ok(()))
            } else if shared.closed {
                // The end of the stream.
                Poll::Ready(Ok(()))
            } else {
                shared.reader = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }

    impl AsyncWrite for Stream {
        fn poll_write(self: Pin<&mut Self>, _context: &mut Context<'_>, data: &[u8]) -> Poll<io::Result<usize>> {
            if self.shared.borrow().closed {
                return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
            }
            Poll::Ready(
                self.socket
                    .send_with_u8_array(data)
                    .map(|()| data.len())
                    .map_err(|error| io::Error::other(format!("{error:?}"))),
            )
        }

        fn poll_flush(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<io::Result<()>> {
            let _ = self.socket.close();
            Poll::Ready(Ok(()))
        }
    }
}
