use serde::Deserialize;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug)]
pub struct AgentConfig {
    pub secret: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub allow_from: Vec<Ipv4Addr>,
    #[serde(default)]
    pub ignore_inhibitors: bool,
    #[serde(default = "default_listen")]
    pub listen: Ipv4Addr,
    #[serde(default)]
    pub power_cmd: Option<PathBuf>,
}

fn default_port() -> u16 {
    9009
}

fn default_listen() -> Ipv4Addr {
    Ipv4Addr::UNSPECIFIED
}

impl AgentConfig {
    pub fn load(path: &Path) -> Result<AgentConfig, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let cfg: AgentConfig =
            toml::from_str(&text).map_err(|e| format!("invalid config {}: {e}", path.display()))?;
        zerowol_proto::parse_secret(&cfg.secret)
            .map_err(|_| "invalid secret: expected 64 hex characters".to_string())?;
        Ok(cfg)
    }
}
