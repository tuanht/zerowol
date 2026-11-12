use crate::config::AgentConfig;
use std::process::Command;
use std::time::Duration;
use zerowol_proto::Action;

pub fn power_command(cfg: &AgentConfig, action: Action) -> Command {
    let word = match action {
        Action::Reboot => "reboot",
        _ => "shutdown",
    };
    if let Some(p) = &cfg.power_cmd {
        let mut c = Command::new(p);
        c.arg(word);
        return c;
    }
    let mut c = Command::new("systemctl");
    c.arg(if action == Action::Reboot {
        "reboot"
    } else {
        "poweroff"
    });
    if cfg.ignore_inhibitors {
        c.arg("-i");
    }
    c
}

/// Run `cmd` on a background thread after a 1 s delay so the response flushes first.
pub fn schedule(mut cmd: Command) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        match cmd.status() {
            Ok(s) => crate::server::log(&format!("power command exited with {s}")),
            Err(e) => crate::server::log(&format!("power command failed: {e}")),
        }
    });
}
