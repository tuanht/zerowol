use std::net::IpAddr;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub data: PathBuf,
    pub require_token: bool,
    pub bind: IpAddr,
    /// `TZ` env var, trimmed; None when unset or empty.
    pub tz_env: Option<String>,
}

fn parse_bool(s: &str) -> Result<bool, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(format!("ZEROWOL_REQUIRE_TOKEN: invalid value '{s}'")),
    }
}

pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Config, String> {
    let port = match get("ZEROWOL_PORT") {
        Some(v) => v
            .trim()
            .parse::<u16>()
            .map_err(|e| format!("ZEROWOL_PORT: invalid value '{v}': {e}"))?,
        None => 8090,
    };
    let data = PathBuf::from(get("ZEROWOL_DATA").unwrap_or_else(|| "/data".into()));
    let require_token = match get("ZEROWOL_REQUIRE_TOKEN") {
        Some(v) => parse_bool(&v)?,
        None => false,
    };
    let bind = match get("ZEROWOL_BIND") {
        Some(v) => v
            .trim()
            .parse::<IpAddr>()
            .map_err(|e| format!("ZEROWOL_BIND: invalid value '{v}': {e}"))?,
        None => IpAddr::from([0, 0, 0, 0]),
    };
    let tz_env = get("TZ")
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    Ok(Config {
        port,
        data,
        require_token,
        bind,
        tz_env,
    })
}

pub fn from_env() -> Result<Config, String> {
    from_vars(|k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        from_vars(|k| m.get(k).cloned())
    }

    #[test]
    fn defaults() {
        let c = cfg(&[]).unwrap();
        assert_eq!(c.port, 8090);
        assert_eq!(c.data, PathBuf::from("/data"));
        assert!(!c.require_token);
        assert_eq!(c.bind.to_string(), "0.0.0.0");
    }

    #[test]
    fn overrides() {
        let c = cfg(&[
            ("ZEROWOL_PORT", "9000"),
            ("ZEROWOL_DATA", "/tmp/x"),
            ("ZEROWOL_REQUIRE_TOKEN", "true"),
            ("ZEROWOL_BIND", "127.0.0.1"),
        ])
        .unwrap();
        assert_eq!(c.port, 9000);
        assert!(c.require_token);
        assert_eq!(c.bind.to_string(), "127.0.0.1");
        assert!(
            cfg(&[("ZEROWOL_REQUIRE_TOKEN", "1")])
                .unwrap()
                .require_token
        );
    }

    #[test]
    fn tz_env_trimmed_and_empty_is_none() {
        assert_eq!(cfg(&[]).unwrap().tz_env, None);
        assert_eq!(cfg(&[("TZ", "  ")]).unwrap().tz_env, None);
        assert_eq!(
            cfg(&[("TZ", " Europe/Berlin ")]).unwrap().tz_env.as_deref(),
            Some("Europe/Berlin")
        );
    }

    #[test]
    fn invalid_errors() {
        assert!(cfg(&[("ZEROWOL_PORT", "abc")]).is_err());
        assert!(cfg(&[("ZEROWOL_REQUIRE_TOKEN", "maybe")]).is_err());
        assert!(cfg(&[("ZEROWOL_BIND", "nope")]).is_err());
    }
}
