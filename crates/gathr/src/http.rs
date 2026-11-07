//! The one HTTP client: plain HTTP/1.1 to the service on 127.0.0.1, for the command line and tests.
//! The service answers with a full body and closes the connection, so a short read loop is enough.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// A response: its status code and body.
#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub body: String,
}

impl Response {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).unwrap_or(serde_json::Value::Null)
    }
}

/// Sends a request with no body. `headers` may replace the default `Host`.
pub fn request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> io::Result<Response> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nConnection: close\r\nContent-Length: 0\r\n");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("host"))
    {
        head.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    let mut raw = String::new();
    stream.read_to_string(&mut raw)?;
    let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((&raw, ""));
    let status = head.split(' ').nth(1).and_then(|s| s.parse().ok());
    let status = status.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "the service sent no status line",
        )
    })?;
    Ok(Response {
        status,
        body: body.to_string(),
    })
}

/// `request` with the service's token.
pub fn call(port: u16, token: &str, method: &str, path: &str) -> io::Result<Response> {
    request(
        port,
        method,
        path,
        &[("Authorization", &format!("Bearer {token}"))],
    )
}
