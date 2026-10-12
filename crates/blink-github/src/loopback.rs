use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::error::{Error, Result};

pub const CALLBACK_PATH: &str = "/callback";
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Blink</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:90vh\">\
<div><h2>Signed in to Blink</h2><p>You can close this tab and return to the app.</p></div>";

const FAILED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>Blink</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:90vh\">\
<div><h2>Sign-in did not complete</h2><p>Return to Blink and try again.</p></div>";

pub struct LoopbackServer {
    listener: TcpListener,
    addr: SocketAddr,
}

enum Callback {
    Code(String),
    Failed(Error),
    Ignored,
}

impl LoopbackServer {
    pub async fn bind() -> Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let addr = listener.local_addr()?;
        Ok(Self { listener, addr })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}{CALLBACK_PATH}", self.addr.port())
    }

    pub async fn wait_for_code(self, expected_state: &str, timeout: Duration) -> Result<String> {
        tokio::time::timeout(timeout, async {
            loop {
                let (stream, _) = self.listener.accept().await?;
                match handle(stream, expected_state).await? {
                    Callback::Code(code) => return Ok(code),
                    Callback::Failed(e) => return Err(e),
                    Callback::Ignored => continue,
                }
            }
        })
        .await
        .map_err(|_| Error::Timeout)?
    }
}

async fn handle(mut stream: TcpStream, expected_state: &str) -> Result<Callback> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf);
    let target = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("/");
    let outcome = parse_callback(target, expected_state);

    let (status, body) = match &outcome {
        Callback::Code(_) => ("200 OK", DONE_PAGE),
        Callback::Failed(_) => ("400 Bad Request", FAILED_PAGE),
        Callback::Ignored => ("404 Not Found", ""),
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
    Ok(outcome)
}

fn parse_callback(target: &str, expected_state: &str) -> Callback {
    let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else { return Callback::Ignored };
    if url.path() != CALLBACK_PATH {
        return Callback::Ignored;
    }
    let param = |name: &str| url.query_pairs().find(|(k, _)| k == name).map(|(_, v)| v.into_owned());
    if param("state").as_deref() != Some(expected_state) {
        return Callback::Failed(Error::StateMismatch);
    }
    if let Some(error) = param("error") {
        return Callback::Failed(if error == "access_denied" {
            Error::Denied
        } else {
            Error::OAuth { error, description: param("error_description") }
        });
    }
    match param("code") {
        Some(code) => Callback::Code(code),
        None => Callback::Failed(Error::OAuth { error: "missing_code".into(), description: None }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn get(url: &str) {
        let url = url::Url::parse(url).unwrap();
        let mut stream = TcpStream::connect((url.host_str().unwrap(), url.port().unwrap())).await.unwrap();
        let path = format!("{}?{}", url.path(), url.query().unwrap_or_default());
        stream.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).await.unwrap();
        let mut out = Vec::new();
        let _ = stream.read_to_end(&mut out).await;
    }

    #[tokio::test]
    async fn receives_code_and_ignores_favicon() {
        let server = LoopbackServer::bind().await.unwrap();
        let redirect = server.redirect_uri();
        let base = redirect.trim_end_matches(CALLBACK_PATH).to_string();
        let client = tokio::spawn(async move {
            get(&format!("{base}/favicon.ico?")).await;
            get(&format!("{redirect}?code=abc&state=s1")).await;
        });
        let code = server.wait_for_code("s1", Duration::from_secs(5)).await.unwrap();
        assert_eq!(code, "abc");
        client.await.unwrap();
    }

    #[tokio::test]
    async fn rejects_wrong_state() {
        let server = LoopbackServer::bind().await.unwrap();
        let redirect = server.redirect_uri();
        tokio::spawn(async move { get(&format!("{redirect}?code=abc&state=evil")).await });
        assert!(matches!(server.wait_for_code("s1", Duration::from_secs(5)).await, Err(Error::StateMismatch)));
    }

    #[test]
    fn denied_maps_to_error() {
        assert!(matches!(
            parse_callback("/callback?error=access_denied&state=s", "s"),
            Callback::Failed(Error::Denied)
        ));
    }
}
