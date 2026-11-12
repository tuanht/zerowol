use crate::app::{ApiError, AppState};
use crate::model::Machine;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};
use std::net::{Ipv4Addr, UdpSocket};

pub static AGENT_X86_64: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/agent-x86_64"));
pub static AGENT_AARCH64: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/agent-aarch64"));

/// The local address the NAS uses to reach `target` (no packet is sent).
pub fn nas_source_ip(target: Ipv4Addr) -> std::io::Result<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0")?;
    s.connect((target, 9))?;
    match s.local_addr()?.ip() {
        std::net::IpAddr::V4(ip) => Ok(ip),
        std::net::IpAddr::V6(_) => Err(std::io::Error::other("not an IPv4 address")),
    }
}

fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= 255
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ':' | '-' | '[' | ']'))
}

/// Host used in install commands: the `Host` header verbatim when it is made
/// only of hostname characters (it ends up inside a shell script), else
/// `<nas_ip>:<port>`.
pub fn install_host(host_header: Option<&str>, nas_ip: Ipv4Addr, port: u16) -> String {
    match host_header {
        Some(h) if host_ok(h) => h.to_string(),
        _ => format!("{nas_ip}:{port}"),
    }
}

pub fn install_command(host: &str, id: &str) -> String {
    format!("curl -fsSL http://{host}/agent/install/{id}.sh | sudo sh")
}

fn embedded(arch: &str) -> Option<&'static [u8]> {
    match arch {
        "x86_64" => Some(AGENT_X86_64),
        "aarch64" => Some(AGENT_AARCH64),
        _ => None,
    }
}

const TEMPLATE: &str = r##"#!/bin/sh
# zerowol agent installer (idempotent: re-running updates binary, config and unit)
set -eu

if [ "$(id -u)" -eq 0 ]; then :; else
  echo "zerowol: run this as root (use sudo)" >&2
  exit 1
fi

NOAGENT=0
case "$(uname -m)" in
  x86_64)
    ARCH=x86_64; SHA=@SHA_X86_64@; NOAGENT=@NOAGENT_X86_64@ ;;
  aarch64|arm64)
    ARCH=aarch64; SHA=@SHA_AARCH64@; NOAGENT=@NOAGENT_AARCH64@ ;;
  *)
    echo "zerowol: unsupported architecture $(uname -m)" >&2
    exit 1 ;;
esac
if [ "$NOAGENT" -eq 1 ]; then
  echo "this zerowol build has no agent for $ARCH" >&2
  exit 1
fi

TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT INT TERM

URL="http://@HOST@/agent/bin/$ARCH"
echo "Downloading $URL"
if command -v curl >/dev/null 2>&1; then
  curl -fsSL -o "$TMP" "$URL"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$TMP" "$URL"
else
  echo "zerowol: need curl or wget" >&2
  exit 1
fi

echo "$SHA  $TMP" | sha256sum -c - >/dev/null || {
  echo "zerowol: checksum mismatch, aborting" >&2
  exit 1
}
install -m 0755 "$TMP" /usr/local/bin/zerowol-agent

umask 077
cat > /etc/zerowol-agent.toml <<'EOF_CONF'
secret = "@SECRET@"
port = @PORT@
allow_from = ["@NAS_IP@"]
ignore_inhibitors = false
EOF_CONF
chmod 600 /etc/zerowol-agent.toml
umask 022

cat > /etc/systemd/system/zerowol-agent.service <<'EOF_UNIT'
[Unit]
Description=zerowol agent
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/zerowol-agent run
Restart=always
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
NoNewPrivileges=yes
ProtectKernelTunables=yes
ProtectControlGroups=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX

[Install]
WantedBy=multi-user.target
EOF_UNIT
systemctl daemon-reload
systemctl enable zerowol-agent
systemctl restart zerowol-agent

if command -v firewall-cmd >/dev/null 2>&1 && firewall-cmd --state >/dev/null 2>&1; then
  firewall-cmd --permanent --add-rich-rule='rule family="ipv4" source address="@NAS_IP@" port port="@PORT@" protocol="tcp" accept'
  firewall-cmd --reload
else
  echo "No running firewalld found. If you use a firewall, allow TCP port @PORT@ from @NAS_IP@."
fi

echo "zerowol-agent running ✓"
"##;

/// POSIX sh installer for machine `m` (spec section 7, steps 1-5). The secret,
/// NAS IP and port are validated types; `host` must come from `install_host`.
pub fn render_script(m: &Machine, host: &str, nas_ip: Ipv4Addr) -> String {
    let (secret, port) = m
        .agent
        .as_ref()
        .map(|a| (a.secret.as_str(), a.port))
        .unwrap_or(("", 9009));
    let sha = |b: &[u8]| hex::encode(Sha256::digest(b));
    let flag = |b: &[u8]| if b.is_empty() { "1" } else { "0" };
    TEMPLATE
        .replace("@SHA_X86_64@", &sha(AGENT_X86_64))
        .replace("@SHA_AARCH64@", &sha(AGENT_AARCH64))
        .replace("@NOAGENT_X86_64@", flag(AGENT_X86_64))
        .replace("@NOAGENT_AARCH64@", flag(AGENT_AARCH64))
        .replace("@HOST@", host)
        .replace("@SECRET@", secret)
        .replace("@PORT@", &port.to_string())
        .replace("@NAS_IP@", &nas_ip.to_string())
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "not found").into_response()
}

/// Host + NAS IP for a machine as seen by this request.
pub fn host_for(state: &AppState, headers: &HeaderMap, m: &Machine) -> (String, Ipv4Addr) {
    let nas_ip = nas_source_ip(m.ip).unwrap_or(Ipv4Addr::LOCALHOST);
    let h = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    (install_host(h, nas_ip, state.cfg.port), nas_ip)
}

pub async fn script(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(file): Path<String>,
) -> Response {
    let Some(id) = file.strip_suffix(".sh") else {
        return not_found();
    };
    let m = state
        .store
        .snapshot()
        .machines
        .into_iter()
        .find(|m| m.id == id && m.agent.is_some());
    let Some(m) = m else {
        return not_found();
    };
    let (host, nas_ip) = host_for(&state, &headers, &m);
    (
        [(header::CONTENT_TYPE, "text/x-shellscript")],
        render_script(&m, &host, nas_ip),
    )
        .into_response()
}

pub async fn bin(Path(arch): Path<String>) -> Result<Response, ApiError> {
    let Some(bytes) = embedded(&arch) else {
        return Ok(not_found());
    };
    if bytes.is_empty() {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent binary not embedded in this build".into(),
        ));
    }
    Ok(([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response())
}
