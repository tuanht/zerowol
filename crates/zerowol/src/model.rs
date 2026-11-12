use rand::RngExt;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

fn d9() -> u16 {
    9
}
fn d22() -> u16 {
    22
}
fn d1() -> u32 {
    1
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AgentCfg {
    pub port: u16,
    pub secret: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub mac: String,
    pub ip: Ipv4Addr,
    pub broadcast: Ipv4Addr,
    #[serde(default = "d9")]
    pub wol_port: u16,
    #[serde(default = "d22")]
    pub probe_port: u16,
    #[serde(default)]
    pub agent: Option<AgentCfg>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Token {
    pub id: String,
    pub name: String,
    pub hash: String,
    pub created: u64,
    #[serde(default)]
    pub last_used: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    #[serde(default = "d1")]
    pub version: u32,
    #[serde(default)]
    pub machines: Vec<Machine>,
    #[serde(default)]
    pub tokens: Vec<Token>,
}

impl Default for State {
    fn default() -> Self {
        State {
            version: 1,
            machines: Vec::new(),
            tokens: Vec::new(),
        }
    }
}

pub fn parse_mac(s: &str) -> Result<[u8; 6], String> {
    let parts: Vec<&str> = s.trim().split([':', '-']).collect();
    if parts.len() != 6 {
        return Err("expected 6 hex octets".into());
    }
    let mut out = [0u8; 6];
    for (i, p) in parts.iter().enumerate() {
        if p.len() != 2 || !p.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("bad octet '{p}'"));
        }
        out[i] = u8::from_str_radix(p, 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

pub fn format_mac(m: [u8; 6]) -> String {
    m.iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

pub fn default_broadcast(ip: Ipv4Addr) -> Ipv4Addr {
    let o = ip.octets();
    Ipv4Addr::new(o[0], o[1], o[2], 255)
}

pub fn new_id() -> String {
    const CH: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut rng = rand::rng();
    (0..6)
        .map(|_| CH[rng.random_range(0..CH.len())] as char)
        .collect()
}

pub fn new_secret() -> String {
    let b: [u8; 32] = rand::rng().random();
    hex::encode(b)
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn mac_roundtrip() {
        let m = parse_mac("d8-5e-d3-db-f7-6b").unwrap();
        assert_eq!(format_mac(m), "D8:5E:D3:DB:F7:6B");
        assert_eq!(parse_mac("D8:5E:D3:DB:F7:6B").unwrap(), m);
    }

    #[test]
    fn mac_invalid() {
        assert!(parse_mac("D8:5E:D3").is_err());
        assert!(parse_mac("ZZ:5E:D3:DB:F7:6B").is_err());
    }

    #[test]
    fn broadcast_default() {
        assert_eq!(
            default_broadcast(Ipv4Addr::new(192, 168, 23, 254)),
            Ipv4Addr::new(192, 168, 23, 255)
        );
    }

    #[test]
    fn id_and_secret() {
        let id = new_id();
        assert_eq!(id.len(), 6);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
        );
        let s = new_secret();
        assert_eq!(s.len(), 64);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn state_defaults_and_optional_fields() {
        let st = State::default();
        assert_eq!(st.version, 1);
        let j = r#"{"version":1,"machines":[{"id":"a","name":"n","mac":"AA:BB:CC:DD:EE:FF","ip":"10.0.0.2","broadcast":"10.0.0.255"}]}"#;
        let st: State = serde_json::from_str(j).unwrap();
        assert_eq!(st.machines[0].wol_port, 9);
        assert_eq!(st.machines[0].probe_port, 22);
        assert!(st.machines[0].agent.is_none());
        assert!(st.tokens.is_empty());
    }
}
