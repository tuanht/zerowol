use crate::config::AgentConfig;
use std::path::Path;
use std::process::{Command, Stdio};

fn is_root() -> bool {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).map(|u| u == "0"))
        })
        .unwrap_or(false)
}

fn quiet(cmd: &str, args: &[&str]) -> bool {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn uninstall(cfg_path: &Path) -> Result<(), String> {
    if !is_root() {
        return Err("uninstall must be run as root".into());
    }
    if let Ok(cfg) = AgentConfig::load(cfg_path)
        && quiet("firewall-cmd", &["--state"])
    {
        for ip in &cfg.allow_from {
            let rule = format!(
                "rule family=\"ipv4\" source address=\"{ip}\" port port=\"{}\" protocol=\"tcp\" accept",
                cfg.port
            );
            quiet(
                "firewall-cmd",
                &["--permanent", "--remove-rich-rule", &rule],
            );
        }
        quiet("firewall-cmd", &["--reload"]);
    }
    quiet("systemctl", &["disable", "--now", "zerowol-agent"]);
    for f in [
        "/etc/systemd/system/zerowol-agent.service",
        &cfg_path.to_string_lossy(),
        "/usr/local/bin/zerowol-agent",
    ] {
        match std::fs::remove_file(f) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("cannot remove {f}: {e}")),
        }
    }
    quiet("systemctl", &["daemon-reload"]);
    println!("zerowol-agent uninstalled");
    Ok(())
}
