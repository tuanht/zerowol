mod config;
mod power;
mod server;
mod sysinfo;
mod uninstall;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut cmd = "run".to_string();
    let mut cfg = PathBuf::from("/etc/zerowol-agent.toml");
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--version" => {
                println!("zerowol-agent {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            "--config" => match args.next() {
                Some(p) => cfg = PathBuf::from(p),
                None => return fail("--config needs a path"),
            },
            "run" | "uninstall" => cmd = a,
            _ => return fail(&format!("unknown argument: {a}")),
        }
    }
    let result = if cmd == "uninstall" {
        uninstall::uninstall(&cfg)
    } else {
        config::AgentConfig::load(&cfg).and_then(|c| server::run(c).map_err(|e| e.to_string()))
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(&e),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("zerowol-agent: {msg}");
    ExitCode::FAILURE
}
