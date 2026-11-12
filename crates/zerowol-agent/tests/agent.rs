use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use zerowol_proto::{Action, SIG_HEADER, build_request};

const SECRET: [u8; 32] = [7u8; 32];

struct Agent(Child);
impl Drop for Agent {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_agent(allow_from: &str) -> (Agent, u16, TempDir) {
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("power.sh");
    let calls = dir.path().join("calls");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$1\" >> {}\n", calls.display()),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let cfg = dir.path().join("agent.toml");
    std::fs::write(
        &cfg,
        format!(
            "secret = \"{}\"\nport = {port}\nlisten = \"127.0.0.1\"\nallow_from = [{allow_from}]\npower_cmd = \"{}\"\n",
            "07".repeat(32),
            script.display()
        ),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_zerowol-agent"))
        .args(["run", "--config"])
        .arg(&cfg)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert!(line.contains("listening"), "got {line:?}");
    (Agent(child), port, dir)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn post_raw(port: u16, raw: &[u8]) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(8))).unwrap();
    s.write_all(raw).unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    let text = String::from_utf8_lossy(&out).to_string();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
    (status, body)
}

fn post(port: u16, path: &str, body: &[u8], sig: &str) -> (u16, String) {
    let mut raw = format!(
        "POST {path} HTTP/1.1\r\nHost: x\r\n{SIG_HEADER}: {sig}\r\nContent-Length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(body);
    post_raw(port, &raw)
}

fn signed(action: Action, nonce: u8) -> (Vec<u8>, String) {
    build_request(&SECRET, action, now(), [nonce; 16])
}

fn wait_calls(dir: &TempDir, want: &str) -> bool {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end {
        if std::fs::read_to_string(dir.path().join("calls"))
            .ok()
            .as_deref()
            == Some(want)
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

#[test]
fn health_ok() {
    let (_a, port, _d) = spawn_agent("");
    let (b, s) = signed(Action::Health, 1);
    let (st, body) = post(port, "/v1/health", &b, &s);
    assert_eq!(st, 200);
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(!v["hostname"].as_str().unwrap().is_empty());
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert!(v["uptime"].as_u64().unwrap() > 0);
}

#[test]
fn shutdown_accepted_then_runs() {
    let (_a, port, d) = spawn_agent("");
    let (b, s) = signed(Action::Shutdown, 2);
    let (st, body) = post(port, "/v1/power", &b, &s);
    assert_eq!(st, 202);
    assert_eq!(body, r#"{"result":"accepted"}"#);
    assert!(wait_calls(&d, "shutdown\n"));
}

#[test]
fn reboot_runs() {
    let (_a, port, d) = spawn_agent("");
    let (b, s) = signed(Action::Reboot, 3);
    assert_eq!(post(port, "/v1/power", &b, &s).0, 202);
    assert!(wait_calls(&d, "reboot\n"));
}

#[test]
fn bad_signature_rejected() {
    let (_a, port, d) = spawn_agent("");
    let (b, _) = signed(Action::Shutdown, 4);
    let (st, body) = post(port, "/v1/power", &b, &"00".repeat(32));
    assert_eq!(st, 401);
    assert_eq!(body, r#"{"error":"bad signature"}"#);
    std::thread::sleep(Duration::from_secs(2));
    assert!(!d.path().join("calls").exists());
}

#[test]
fn replay_rejected() {
    let (_a, port, d) = spawn_agent("");
    let (b, s) = signed(Action::Shutdown, 5);
    assert_eq!(post(port, "/v1/power", &b, &s).0, 202);
    let (st, body) = post(port, "/v1/power", &b, &s);
    assert_eq!(st, 401);
    assert_eq!(body, r#"{"error":"replay"}"#);
    assert!(wait_calls(&d, "shutdown\n"));
    std::thread::sleep(Duration::from_millis(1500));
    assert_eq!(
        std::fs::read_to_string(d.path().join("calls")).unwrap(),
        "shutdown\n"
    );
}

#[test]
fn health_on_power_path_rejected() {
    let (_a, port, _d) = spawn_agent("");
    let (b, s) = signed(Action::Health, 6);
    assert_eq!(post(port, "/v1/power", &b, &s).0, 400);
}

#[test]
fn unknown_path_404() {
    let (_a, port, _d) = spawn_agent("");
    let (b, s) = signed(Action::Health, 7);
    let (st, body) = post(port, "/v1/nope", &b, &s);
    assert_eq!(st, 404);
    assert_eq!(body, r#"{"error":"not found"}"#);
}

#[test]
fn disallowed_ip_closed() {
    let (_a, port, _d) = spawn_agent("\"10.255.255.1\"");
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let (b, sig) = signed(Action::Health, 8);
    let _ = s.write_all(
        format!(
            "POST /v1/health HTTP/1.1\r\n{SIG_HEADER}: {sig}\r\nContent-Length: {}\r\n\r\n",
            b.len()
        )
        .as_bytes(),
    );
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf);
    assert!(buf.is_empty());
}

#[test]
fn survives_garbage() {
    let (_a, port, _d) = spawn_agent("");
    assert_eq!(post_raw(port, b"garbage\r\n\r\n").0, 400);
    let big = vec![b'a'; 2000];
    assert_eq!(post(port, "/v1/health", &big, "00").0, 400);
    // slow client holds a socket open without sending anything
    let _idle = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let (b, s) = signed(Action::Health, 9);
    assert_eq!(post(port, "/v1/health", &b, &s).0, 200);
    std::thread::sleep(Duration::from_secs(6));
    let (b, s) = signed(Action::Health, 10);
    assert_eq!(post(port, "/v1/health", &b, &s).0, 200);
}
