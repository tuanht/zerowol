use rand::Rng;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use zerowol_proto::{Action, SIG_HEADER, build_request, parse_secret};

const CALL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct Health {
    pub hostname: String,
    pub version: String,
    pub uptime: u64,
}

#[derive(Debug, PartialEq)]
pub enum AgentError {
    Unreachable,
    Rejected(String),
}

impl AgentError {
    pub fn message(&self) -> String {
        match self {
            AgentError::Unreachable => "agent unreachable".into(),
            AgentError::Rejected(s) => s.clone(),
        }
    }
}

/// POST a signed command; returns (status code, body bytes).
async fn post(
    ip: Ipv4Addr,
    port: u16,
    path: &str,
    secret: &str,
    action: Action,
) -> Result<(u16, Vec<u8>), AgentError> {
    let key = parse_secret(secret).map_err(|_| AgentError::Unreachable)?;
    let nonce: [u8; 16] = rand::rng().random();
    let (body, sig) = build_request(&key, action, crate::model::now(), nonce);
    let io = async {
        let mut s = TcpStream::connect((ip, port)).await?;
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {ip}:{port}\r\n{SIG_HEADER}: {sig}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        s.write_all(head.as_bytes()).await?;
        s.write_all(&body).await?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    };
    let raw = tokio::time::timeout(CALL_TIMEOUT, io)
        .await
        .map_err(|_| AgentError::Unreachable)?
        .map_err(|_| AgentError::Unreachable)?;
    parse_response(&raw).ok_or(AgentError::Unreachable)
}

fn parse_response(raw: &[u8]) -> Option<(u16, Vec<u8>)> {
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n")?;
    let head = std::str::from_utf8(&raw[..split]).ok()?;
    let code = head
        .lines()
        .next()?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    Some((code, raw[split + 4..].to_vec()))
}

fn rejection(body: &[u8]) -> AgentError {
    #[derive(Deserialize)]
    struct E {
        error: String,
    }
    match serde_json::from_slice::<E>(body) {
        Ok(e) => AgentError::Rejected(e.error),
        Err(_) => AgentError::Unreachable,
    }
}

pub async fn health(ip: Ipv4Addr, port: u16, secret: &str) -> Result<Health, AgentError> {
    let (code, body) = post(ip, port, "/v1/health", secret, Action::Health).await?;
    if (200..300).contains(&code) {
        serde_json::from_slice(&body).map_err(|_| AgentError::Unreachable)
    } else {
        Err(rejection(&body))
    }
}

pub async fn power(
    ip: Ipv4Addr,
    port: u16,
    secret: &str,
    action: Action,
) -> Result<(), AgentError> {
    let (code, body) = post(ip, port, "/v1/power", secret, action).await?;
    if code == 202 {
        Ok(())
    } else if (200..300).contains(&code) {
        Err(AgentError::Unreachable)
    } else {
        Err(rejection(&body))
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use std::net::Ipv4Addr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use zerowol_proto::{Action, ReplayGuard, SIG_HEADER, parse_secret, verify};

    /// Fake agent: verifies the signature, then replies with `status` + `json`.
    /// Returns the port. Replies 401 `bad signature` if verification fails.
    pub async fn fake_agent(secret: &str, path: &str, status: &str, json: &'static str) -> u16 {
        let key = parse_secret(secret).unwrap();
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let status = status.to_string();
        let path = path.to_string();
        tokio::spawn(async move {
            let mut guard = ReplayGuard::new();
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 2048];
                loop {
                    let n = s.read(&mut tmp).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&tmp[..n]);
                    if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&buf[..p]).to_string();
                        let len: usize = head
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse().unwrap())
                            })
                            .unwrap_or(0);
                        if buf.len() >= p + 4 + len {
                            break;
                        }
                    }
                }
                let p = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
                let head = String::from_utf8_lossy(&buf[..p]).to_string();
                let sig = head
                    .lines()
                    .find_map(|l| {
                        let lower = l.to_ascii_lowercase();
                        lower
                            .starts_with(&SIG_HEADER.to_ascii_lowercase())
                            .then(|| l.split_once(':').unwrap().1.trim().to_string())
                    })
                    .unwrap_or_default();
                let body = &buf[p + 4..];
                let ok = verify(
                    &key,
                    body,
                    &sig,
                    &[Action::Health, Action::Shutdown, Action::Reboot],
                    &mut guard,
                    crate::model::now(),
                )
                .is_ok()
                    && head.starts_with(&format!("POST {path} HTTP/1.1"));
                let (st, js) = if ok {
                    (status.as_str(), json)
                } else {
                    ("401 Unauthorized", r#"{"error":"bad signature"}"#)
                };
                let resp = format!(
                    "HTTP/1.1 {st}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{js}",
                    js.len()
                );
                let _ = s.write_all(resp.as_bytes()).await;
            }
        });
        port
    }

    /// A listener that accepts connections and never answers.
    pub async fn silent_agent() -> u16 {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((s, _)) = l.accept().await {
                held.push(s);
            }
        });
        port
    }

    pub async fn closed_port() -> u16 {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        l.local_addr().unwrap().port()
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    const LO: Ipv4Addr = Ipv4Addr::LOCALHOST;
    fn secret() -> String {
        crate::model::new_secret()
    }

    #[tokio::test]
    async fn health_ok() {
        let s = secret();
        let port = fake_agent(
            &s,
            "/v1/health",
            "200 OK",
            r#"{"hostname":"pc","version":"0.1.0","uptime":42}"#,
        )
        .await;
        let h = health(LO, port, &s).await.unwrap();
        assert_eq!(h.hostname, "pc");
        assert_eq!(h.uptime, 42);
    }

    #[tokio::test]
    async fn health_wrong_secret_rejected() {
        let port = fake_agent(&secret(), "/v1/health", "200 OK", "{}").await;
        let e = health(LO, port, &secret()).await.unwrap_err();
        assert_eq!(e, AgentError::Rejected("bad signature".into()));
    }

    #[tokio::test]
    async fn power_accepted() {
        let s = secret();
        let port = fake_agent(&s, "/v1/power", "202 Accepted", r#"{"result":"accepted"}"#).await;
        assert_eq!(power(LO, port, &s, Action::Shutdown).await, Ok(()));
    }

    #[tokio::test]
    async fn power_rejected_reason() {
        let s = secret();
        let port = fake_agent(
            &s,
            "/v1/power",
            "401 Unauthorized",
            r#"{"error":"clock skew 45s"}"#,
        )
        .await;
        let e = power(LO, port, &s, Action::Reboot).await.unwrap_err();
        assert_eq!(e, AgentError::Rejected("clock skew 45s".into()));
        assert_eq!(e.message(), "clock skew 45s");
    }

    #[tokio::test]
    async fn unreachable() {
        let port = closed_port().await;
        let e = health(LO, port, &secret()).await.unwrap_err();
        assert_eq!(e, AgentError::Unreachable);
        assert_eq!(e.message(), "agent unreachable");
    }
}
