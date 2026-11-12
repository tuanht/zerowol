use crate::config::AgentConfig;
use crate::{power, sysinfo};
use std::io::{self, Read, Write};
use std::net::{IpAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use zerowol_proto::{Action, MAX_BODY, ReplayGuard, SIG_HEADER, parse_secret, verify};

const TIMEOUT: Duration = Duration::from_secs(5);
const MAX_HEADERS: usize = 8192;

struct State {
    cfg: AgentConfig,
    secret: Vec<u8>,
    guard: Mutex<ReplayGuard>,
}

pub fn run(cfg: AgentConfig) -> io::Result<()> {
    let listener = TcpListener::bind((cfg.listen, cfg.port))?;
    log(&format!(
        "zerowol-agent {} listening on {}:{}",
        env!("CARGO_PKG_VERSION"),
        cfg.listen,
        cfg.port
    ));
    let secret = parse_secret(&cfg.secret)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "bad secret"))?;
    let state = Arc::new(State {
        cfg,
        secret,
        guard: Mutex::new(ReplayGuard::new()),
    });
    for conn in listener.incoming() {
        let Ok(stream) = conn else { continue };
        let state = Arc::clone(&state);
        std::thread::spawn(move || {
            let _ = handle(&state, stream);
        });
    }
    Ok(())
}

/// Log to stdout; a closed pipe must never panic the agent.
pub fn log(msg: &str) {
    let _ = writeln!(io::stdout(), "{msg}");
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn respond(s: &mut TcpStream, status: u16, body: &str) -> io::Result<()> {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "Error",
    };
    write!(
        s,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    s.flush()
}

fn err(s: &mut TcpStream, status: u16, msg: &str) -> io::Result<()> {
    let body = serde_json::json!({ "error": msg }).to_string();
    respond(s, status, &body)
}

struct Request {
    path: String,
    sig: String,
    body: Vec<u8>,
}

/// Parse an HTTP request. Any failure (I/O error, malformed or oversized
/// request) returns `Err(())`; the caller just closes the connection.
fn read_request(s: &mut TcpStream) -> Result<Request, ()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p;
        }
        if buf.len() > MAX_HEADERS {
            return Err(());
        }
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = std::str::from_utf8(&buf[..head_end]).map_err(|_| ())?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next().ok_or(())?.split(' ');
    let (method, path, ver) = (first.next(), first.next(), first.next());
    if method != Some("POST") || !ver.is_some_and(|v| v.starts_with("HTTP/1.")) {
        return Err(());
    }
    let path = path.ok_or(())?.to_string();
    let (mut len, mut sig) = (0usize, String::new());
    for l in lines {
        let (k, v) = l.split_once(':').ok_or(())?;
        let (k, v) = (k.trim(), v.trim());
        if k.eq_ignore_ascii_case("content-length") {
            len = v.parse().map_err(|_| ())?;
        } else if k.eq_ignore_ascii_case(SIG_HEADER) {
            sig = v.to_string();
        }
    }
    if len > MAX_BODY {
        return Err(());
    }
    let mut body = buf[head_end + 4..].to_vec();
    if body.len() > len {
        body.truncate(len);
    }
    while body.len() < len {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => body.extend_from_slice(&chunk[..n.min(len - body.len())]),
        }
    }
    Ok(Request { path, sig, body })
}

fn handle(st: &State, mut s: TcpStream) -> io::Result<()> {
    // Source-IP check first: disallowed peers get the connection closed, no bytes.
    if !st.cfg.allow_from.is_empty() {
        let ok = match s.peer_addr()?.ip() {
            IpAddr::V4(ip) => st.cfg.allow_from.contains(&ip),
            _ => false,
        };
        if !ok {
            return Ok(());
        }
    }
    s.set_read_timeout(Some(TIMEOUT))?;
    s.set_write_timeout(Some(TIMEOUT))?;
    let Ok(req) = read_request(&mut s) else {
        return err(&mut s, 400, "bad request");
    };
    let allowed: &[Action] = match req.path.as_str() {
        "/v1/health" => &[Action::Health],
        "/v1/power" => &[Action::Shutdown, Action::Reboot],
        _ => return err(&mut s, 404, "not found"),
    };
    let res = {
        let mut g = st.guard.lock().unwrap_or_else(|e| e.into_inner());
        verify(&st.secret, &req.body, &req.sig, allowed, &mut g, now())
    };
    let cmd = match res {
        Ok(c) => c,
        Err(r) => {
            let status = match r {
                zerowol_proto::Reject::BadRequest | zerowol_proto::Reject::WrongAction => 400,
                _ => 401,
            };
            return err(&mut s, status, &r.message());
        }
    };
    if cmd.action == Action::Health {
        let body = serde_json::json!({
            "hostname": sysinfo::hostname(),
            "version": env!("CARGO_PKG_VERSION"),
            "uptime": sysinfo::uptime_secs(),
        })
        .to_string();
        respond(&mut s, 200, &body)
    } else {
        log(&format!("action={:?} accepted", cmd.action));
        let r = respond(&mut s, 202, r#"{"result":"accepted"}"#);
        power::schedule(power::power_command(&st.cfg, cmd.action));
        r
    }
}
