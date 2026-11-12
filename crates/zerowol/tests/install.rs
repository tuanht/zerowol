mod common;
use common::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::net::Ipv4Addr;
use zerowol::install::{AGENT_X86_64, install_command, install_host, nas_source_ip, render_script};
use zerowol::model::{AgentCfg, Machine};

fn machine() -> Machine {
    Machine {
        id: "abc123".into(),
        name: "Desktop".into(),
        mac: "D8:5E:D3:DB:F7:6B".into(),
        ip: "192.168.23.254".parse().unwrap(),
        broadcast: "192.168.23.255".parse().unwrap(),
        wol_port: 9,
        probe_port: 22,
        agent: Some(AgentCfg {
            port: 9009,
            secret: "ab".repeat(32),
        }),
    }
}

#[test]
fn script_contents() {
    let nas: Ipv4Addr = "192.168.23.10".parse().unwrap();
    let s = render_script(&machine(), "nas.local:8090", nas);
    assert!(s.contains(&"ab".repeat(32)));
    assert!(s.contains(r#"allow_from = ["192.168.23.10"]"#));
    assert!(s.contains("port = 9009"));
    assert!(s.contains("ignore_inhibitors = false"));
    assert!(s.contains("ProtectSystem=strict"));
    assert!(s.contains("firewall-cmd"));
    assert!(s.contains(&hex::encode(Sha256::digest(AGENT_X86_64))));
    assert!(s.contains("http://nas.local:8090/agent/bin/"));
    assert!(s.contains(
        r#"rule family="ipv4" source address="192.168.23.10" port port="9009" protocol="tcp" accept"#
    ));
    for d in [
        "ExecStart=/usr/local/bin/zerowol-agent run",
        "Restart=always",
        "ProtectHome=yes",
        "PrivateTmp=yes",
        "NoNewPrivileges=yes",
        "ProtectKernelTunables=yes",
        "ProtectControlGroups=yes",
        "RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX",
        "trap 'rm -f \"$TMP\"' EXIT",
        "trap 'exit 130' INT TERM",
        "systemctl is-active --quiet zerowol-agent",
        "journalctl -u zerowol-agent",
        "could not add firewalld rule; allow TCP 9009 from 192.168.23.10 manually",
        "mktemp",
        "zerowol-agent running",
    ] {
        assert!(s.contains(d), "missing {d}");
    }
    let mut child = std::process::Command::new("sh")
        .arg("-n")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(s.as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
}

#[test]
fn install_host_header_verbatim() {
    let ip: Ipv4Addr = "192.168.23.10".parse().unwrap();
    assert_eq!(
        install_host(Some("nas.tailnet:8090"), ip, 8090),
        "nas.tailnet:8090"
    );
    assert_eq!(install_host(None, ip, 8090), "192.168.23.10:8090");
    assert_eq!(install_host(Some(""), ip, 8090), "192.168.23.10:8090");
}

#[test]
fn install_host_rejects_shell_metacharacters() {
    let ip: Ipv4Addr = "192.168.23.10".parse().unwrap();
    for bad in ["x;rm -rf /", "a b", "$(id)", "a`id`", "a'b", "a\"b", "a|b"] {
        assert_eq!(install_host(Some(bad), ip, 8090), "192.168.23.10:8090");
    }
    assert_eq!(install_host(Some("[::1]:8090"), ip, 8090), "[::1]:8090");
}

#[test]
fn install_command_format() {
    assert_eq!(
        install_command("h:1", "abc"),
        "curl -fsSL http://h:1/agent/install/abc.sh | sudo sh"
    );
}

#[test]
fn nas_source_ip_loopback() {
    let ip = nas_source_ip("127.0.0.1".parse().unwrap()).unwrap();
    assert_eq!(ip, "127.0.0.1".parse::<Ipv4Addr>().unwrap());
}

async fn add_machine_with(base: &str, mac: &str, agent: bool) -> String {
    let (c, b) = req(
        "POST",
        &format!("{base}/api/v1/machines"),
        &[],
        Some(&json!({"name":"PC","mac":mac,"ip":"127.0.0.1","agent":agent}).to_string()),
    )
    .await;
    assert_eq!(c, 201);
    b["id"].as_str().unwrap().to_string()
}

async fn add_machine(base: &str, agent: bool) -> String {
    add_machine_with(base, "AA:BB:CC:DD:EE:FF", agent).await
}

fn secret_of(state: &zerowol::app::AppState, id: &str) -> String {
    state
        .store
        .snapshot()
        .machines
        .iter()
        .find(|m| m.id == id)
        .unwrap()
        .agent
        .as_ref()
        .unwrap()
        .secret
        .clone()
}

#[tokio::test]
async fn install_command_route() {
    let (base, _s, _d) = spawn(false).await;
    let id = add_machine(&base, true).await;
    let (c, b) = req(
        "GET",
        &format!("{base}/api/v1/machines/{id}/agent/install-command"),
        &[("Host", "nas.tailnet:8090")],
        None,
    )
    .await;
    assert_eq!(c, 200);
    assert_eq!(
        b["install_command"],
        format!("curl -fsSL http://nas.tailnet:8090/agent/install/{id}.sh | sudo sh")
    );
}

#[tokio::test]
async fn rotate_changes_secret() {
    let (base, state, _d) = spawn(false).await;
    let id = add_machine(&base, true).await;
    let before = secret_of(&state, &id);
    let (c, b) = req(
        "POST",
        &format!("{base}/api/v1/machines/{id}/agent/rotate"),
        &[("Host", "nas.local:8090")],
        None,
    )
    .await;
    assert_eq!(c, 200);
    assert_eq!(
        b["install_command"],
        format!("curl -fsSL http://nas.local:8090/agent/install/{id}.sh | sudo sh")
    );
    assert_ne!(secret_of(&state, &id), before);
    let id2 = add_machine_with(&base, "11:22:33:44:55:66", false).await;
    let (c, _) = req(
        "POST",
        &format!("{base}/api/v1/machines/{id2}/agent/rotate"),
        &[],
        None,
    )
    .await;
    assert_eq!(c, 409);
}

#[tokio::test]
async fn install_script_served_with_secret() {
    let (base, state, _d) = spawn(false).await;
    let id = add_machine(&base, true).await;
    let (c, h, body) = req_full(
        "GET",
        &format!("{base}/agent/install/{id}.sh"),
        &[("Host", "nas.local:8090")],
        None,
    )
    .await;
    assert_eq!(c, 200);
    assert!(
        h.iter()
            .any(|(k, v)| k == "content-type" && v == "text/x-shellscript")
    );
    assert!(body.contains(&secret_of(&state, &id)));
    assert!(body.contains("http://nas.local:8090/agent/bin/"));
    assert!(body.contains(r#"allow_from = ["127.0.0.1"]"#));
}

#[tokio::test]
async fn install_script_404_without_agent() {
    let (base, _s, _d) = spawn(false).await;
    let id = add_machine(&base, false).await;
    for p in [
        format!("/agent/install/{id}.sh"),
        "/agent/install/nope.sh".to_string(),
        format!("/agent/install/{id}"),
    ] {
        let (c, b) = req_raw("GET", &format!("{base}{p}"), &[], None).await;
        assert_eq!((c, b.as_str()), (404, "not found"), "{p}");
    }
}

#[tokio::test]
async fn agent_bin_503_when_empty() {
    let (base, _s, _d) = spawn(false).await;
    let (c, b) = req("GET", &format!("{base}/agent/bin/x86_64"), &[], None).await;
    assert_eq!(c, 503);
    assert_eq!(
        b,
        json!({"error":"agent binary not embedded in this build"})
    );
    let (c, _) = req("GET", &format!("{base}/agent/bin/arm"), &[], None).await;
    assert_eq!(c, 404);
}
