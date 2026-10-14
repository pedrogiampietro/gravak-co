//! gravak-ws-bridge: lets the browser client (docs/web-spike.md) reach the
//! game's TCP servers. A browser opens `ws://<bridge>/<port>` (behind nginx,
//! `wss://gravak.fun/ws/<port>`); the bridge connects to `<upstream>:<port>`
//! and carries the bytes both ways, one binary message per chunk. It reads
//! nothing: the TQ cipher and packets stay between the client and the
//! server.
//!
//!   gravak-ws-bridge [--listen 127.0.0.1:9960] [--upstream 127.0.0.1]
//!                    [--allow 9958,5816]
//!
//! Only the allowed ports are reached. The upstream connection is made
//! before the WebSocket is accepted, so a browser only sees an open socket
//! when the server answered (the login screen's world status relies on it).
//!
//! The servers see every browser player as coming from the bridge's
//! address.

use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

struct Options {
    listen: SocketAddr,
    upstream: String,
    allow: Vec<u16>,
}

fn options() -> Result<Options> {
    let mut options = Options {
        listen: "127.0.0.1:9960".parse()?,
        upstream: "127.0.0.1".into(),
        allow: vec![9958, 5816],
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
        match arg.as_str() {
            "--listen" => options.listen = value()?.parse()?,
            "--upstream" => options.upstream = value()?,
            "--allow" => {
                options.allow = value()?
                    .split(',')
                    .map(|port| port.trim().parse())
                    .collect::<std::result::Result<_, _>>()?
            }
            "--help" | "-h" => {
                println!("gravak-ws-bridge [--listen 127.0.0.1:9960] [--upstream 127.0.0.1] [--allow 9958,5816]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}; see --help").into()),
        }
    }
    Ok(options)
}

#[tokio::main]
async fn main() {
    let options = match options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("gravak-ws-bridge: {error}");
            std::process::exit(2);
        }
    };
    let listener = match TcpListener::bind(options.listen).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("gravak-ws-bridge: cannot listen on {}: {error}", options.listen);
            std::process::exit(1);
        }
    };
    println!(
        "gravak-ws-bridge on {}: ports {:?} of {}",
        options.listen, options.allow, options.upstream
    );
    let options = std::sync::Arc::new(options);
    loop {
        let Ok((browser, peer)) = listener.accept().await else { continue };
        let options = options.clone();
        tokio::spawn(async move {
            if let Err(error) = serve(browser, &options).await {
                println!("{peer}: {error}");
            }
        });
    }
}

/// The port asked for: the last part of the request's path (`/9958`,
/// `/ws/9958`).
fn requested_port(request_head: &str) -> Option<u16> {
    let path = request_head.lines().next()?.split_whitespace().nth(1)?;
    let path = path.split('?').next()?;
    path.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

/// The browser's real address when nginx passes it on.
fn forwarded_for(request_head: &str) -> Option<&str> {
    request_head.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim().eq_ignore_ascii_case("x-forwarded-for").then(|| value.trim())
    })
}

async fn refuse(mut browser: TcpStream, status: &str) -> Result<()> {
    let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    browser.write_all(response.as_bytes()).await?;
    Ok(())
}

async fn serve(browser: TcpStream, options: &Options) -> Result<()> {
    // The request line, read without taking it from the handshake.
    let mut head = vec![0; 4096];
    let mut seen = 0;
    for _ in 0..50 {
        seen = browser.peek(&mut head).await?;
        if head[..seen].windows(4).any(|window| window == b"\r\n\r\n") || seen == head.len() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let head = String::from_utf8_lossy(&head[..seen]).into_owned();
    let who = forwarded_for(&head).map(str::to_owned).unwrap_or_else(|| {
        browser.peer_addr().map(|address| address.to_string()).unwrap_or_default()
    });
    let Some(port) = requested_port(&head).filter(|port| options.allow.contains(port)) else {
        refuse(browser, "404 Not Found").await?;
        return Err("asked for a port that is not allowed".into());
    };
    let server = match TcpStream::connect((options.upstream.as_str(), port)).await {
        Ok(server) => server,
        Err(error) => {
            refuse(browser, "502 Bad Gateway").await?;
            return Err(format!("{who} -> {port}: server unavailable ({error})").into());
        }
    };
    server.set_nodelay(true)?;
    let socket = tokio_tungstenite::accept_async(browser).await?;
    println!("{who} -> {port}: open");
    let (mut to_browser, mut from_browser) = socket.split();
    let (mut from_server, mut to_server) = server.into_split();
    let upstream = async {
        while let Some(message) = from_browser.next().await {
            match message? {
                Message::Binary(bytes) => to_server.write_all(&bytes).await?,
                Message::Close(_) => break,
                // Pings are answered by tungstenite; text is not ours.
                _ => {}
            }
        }
        to_server.shutdown().await?;
        Result::Ok(())
    };
    let downstream = async {
        let mut buffer = vec![0; 16 * 1024];
        loop {
            let count = from_server.read(&mut buffer).await?;
            if count == 0 {
                break;
            }
            to_browser.send(Message::binary(buffer[..count].to_vec())).await?;
        }
        let _ = to_browser.send(Message::Close(None)).await;
        Result::Ok(())
    };
    // Either side ending ends both.
    let result = tokio::select! {
        result = upstream => result,
        result = downstream => result,
    };
    println!("{who} -> {port}: closed");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_is_the_last_part_of_the_path() {
        assert_eq!(requested_port("GET /9958 HTTP/1.1\r\nHost: x\r\n"), Some(9958));
        assert_eq!(requested_port("GET /ws/5816/ HTTP/1.1\r\n"), Some(5816));
        assert_eq!(requested_port("GET /ws/5816?x=1 HTTP/1.1\r\n"), Some(5816));
        assert_eq!(requested_port("GET /ws HTTP/1.1\r\n"), None);
        assert_eq!(forwarded_for("GET / HTTP/1.1\r\nX-Forwarded-For: 1.2.3.4\r\n"), Some("1.2.3.4"));
    }
}
