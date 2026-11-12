#![allow(dead_code)]
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use zerowol::app::{AppState, router};
use zerowol::config::Config;
use zerowol::store::Store;

pub async fn spawn(require_token: bool) -> (String, AppState, TempDir) {
    let dir = TempDir::new().unwrap();
    let store = Arc::new(Store::open(dir.path()).unwrap());
    let cfg = Arc::new(Config {
        port: 0,
        data: dir.path().to_path_buf(),
        require_token,
        bind: "127.0.0.1".parse().unwrap(),
    });
    let state = AppState {
        store,
        cfg,
        limiter: Arc::new(Mutex::new(Default::default())),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = router(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), state, dir)
}

fn dechunk(mut b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(p) = b.windows(2).position(|w| w == b"\r\n") {
        let size =
            usize::from_str_radix(std::str::from_utf8(&b[..p]).unwrap().trim(), 16).unwrap_or(0);
        b = &b[p + 2..];
        if size == 0 || b.len() < size {
            break;
        }
        out.extend_from_slice(&b[..size]);
        b = b.get(size + 2..).unwrap_or(&[]);
    }
    out
}

/// Raw HTTP/1.1 request; returns (status, headers (lowercased names), body).
pub async fn req_full(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, Vec<(String, String)>, String) {
    let rest = url.strip_prefix("http://").unwrap();
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let mut s = TcpStream::connect(host).await.unwrap();
    let mut r = format!("{method} {path} HTTP/1.1\r\n");
    if !headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host")) {
        r.push_str(&format!("Host: {host}\r\n"));
    }
    for (k, v) in headers {
        r.push_str(&format!("{k}: {v}\r\n"));
    }
    if let Some(b) = body {
        r.push_str(&format!("Content-Length: {}\r\n", b.len()));
    }
    r.push_str("Connection: close\r\n\r\n");
    if let Some(b) = body {
        r.push_str(b);
    }
    s.write_all(r.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).await.unwrap();
    let split = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    let raw_body = &buf[split + 4..];
    let mut lines = head.split("\r\n");
    let status: u16 = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let hs: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let chunked = hs
        .iter()
        .any(|(k, v)| k == "transfer-encoding" && v.to_ascii_lowercase().contains("chunked"));
    let body_bytes = if chunked {
        dechunk(raw_body)
    } else {
        raw_body.to_vec()
    };
    (status, hs, String::from_utf8_lossy(&body_bytes).to_string())
}

pub async fn req_raw(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, String) {
    let (s, _, b) = req_full(method, url, headers, body).await;
    (s, b)
}

pub async fn req(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, serde_json::Value) {
    let (s, b) = req_raw(method, url, headers, body).await;
    (
        s,
        serde_json::from_str(&b).unwrap_or(serde_json::Value::Null),
    )
}
