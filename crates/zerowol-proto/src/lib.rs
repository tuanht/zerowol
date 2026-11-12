//! Signed-command message format shared by the zerowol server and agent.
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;

pub const SIG_HEADER: &str = "X-Zerowol-Sig";
pub const MAX_SKEW_SECS: u64 = 30;
pub const NONCE_TTL_SECS: u64 = 60;
pub const MAX_BODY: usize = 1024;

type HmacSha256 = Hmac<Sha256>;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Health,
    Shutdown,
    Reboot,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Command {
    pub action: Action,
    pub ts: u64,
    pub nonce: String,
}

#[derive(Debug, PartialEq)]
pub enum Reject {
    BadRequest,
    BadSignature,
    WrongAction,
    Skew(i64),
    Replay,
}

impl Reject {
    pub fn message(&self) -> String {
        match self {
            Reject::BadRequest | Reject::WrongAction => "bad request".into(),
            Reject::BadSignature => "bad signature".into(),
            Reject::Skew(n) => format!("clock skew {}s", n.unsigned_abs()),
            Reject::Replay => "replay".into(),
        }
    }
}

fn mac(secret: &[u8]) -> HmacSha256 {
    HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length")
}

/// HMAC-SHA256 of `body`, lowercase hex.
pub fn sign(secret: &[u8], body: &[u8]) -> String {
    let mut m = mac(secret);
    m.update(body);
    hex::encode(m.finalize().into_bytes())
}

/// Build a JSON body and its signature.
pub fn build_request(
    secret: &[u8],
    action: Action,
    now: u64,
    nonce: [u8; 16],
) -> (Vec<u8>, String) {
    let cmd = Command {
        action,
        ts: now,
        nonce: hex::encode(nonce),
    };
    let body = serde_json::to_vec(&cmd).expect("command serializes");
    let sig = sign(secret, &body);
    (body, sig)
}

/// Parse a 64-hex-char secret into 32 bytes.
pub fn parse_secret(hex_str: &str) -> Result<Vec<u8>, Reject> {
    if hex_str.len() != 64 {
        return Err(Reject::BadRequest);
    }
    hex::decode(hex_str).map_err(|_| Reject::BadRequest)
}

/// Recently seen nonces (nonce -> time first seen).
#[derive(Default)]
pub struct ReplayGuard {
    seen: HashMap<String, u64>,
}

impl ReplayGuard {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn verify(
    secret: &[u8],
    body: &[u8],
    sig_hex: &str,
    allowed: &[Action],
    guard: &mut ReplayGuard,
    now: u64,
) -> Result<Command, Reject> {
    if body.len() > MAX_BODY {
        return Err(Reject::BadRequest);
    }
    let sig = hex::decode(sig_hex.trim()).map_err(|_| Reject::BadSignature)?;
    let mut m = mac(secret);
    m.update(body);
    m.verify_slice(&sig).map_err(|_| Reject::BadSignature)?;
    let cmd: Command = serde_json::from_slice(body).map_err(|_| Reject::BadRequest)?;
    if !allowed.contains(&cmd.action) {
        return Err(Reject::WrongAction);
    }
    let skew = now as i64 - cmd.ts as i64;
    if skew.unsigned_abs() > MAX_SKEW_SECS {
        return Err(Reject::Skew(skew.abs()));
    }
    guard
        .seen
        .retain(|_, seen| now.saturating_sub(*seen) <= NONCE_TTL_SECS);
    if guard.seen.contains_key(&cmd.nonce) {
        return Err(Reject::Replay);
    }
    guard.seen.insert(cmd.nonce.clone(), now);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;
    const S: &[u8] = &[7u8; 32];

    fn req(action: Action, ts: u64, nonce: u8) -> (Vec<u8>, String) {
        build_request(S, action, ts, [nonce; 16])
    }

    #[test]
    fn roundtrip_ok() {
        let (b, s) = build_request(S, Action::Shutdown, 1000, [1; 16]);
        let c = verify(
            S,
            &b,
            &s,
            &[Action::Shutdown, Action::Reboot],
            &mut ReplayGuard::new(),
            1000,
        )
        .unwrap();
        assert_eq!(c.action, Action::Shutdown);
        assert_eq!(c.nonce.len(), 32);
    }

    #[test]
    fn tampered_body() {
        let (mut b, s) = req(Action::Shutdown, 1000, 1);
        b[3] ^= 1;
        let r = verify(
            S,
            &b,
            &s,
            &[Action::Shutdown],
            &mut ReplayGuard::new(),
            1000,
        );
        assert_eq!(r.unwrap_err(), Reject::BadSignature);
    }

    #[test]
    fn wrong_secret() {
        let (b, s) = req(Action::Shutdown, 1000, 1);
        let r = verify(
            &[8u8; 32],
            &b,
            &s,
            &[Action::Shutdown],
            &mut ReplayGuard::new(),
            1000,
        );
        assert_eq!(r.unwrap_err(), Reject::BadSignature);
    }

    #[test]
    fn skew_edges() {
        let a = [Action::Shutdown];
        for (now, n) in [(1030u64, 1u8), (970, 2)] {
            let (b, s) = req(Action::Shutdown, 1000, n);
            assert!(verify(S, &b, &s, &a, &mut ReplayGuard::new(), now).is_ok());
        }
        for (now, n) in [(1031u64, 3u8), (969, 4)] {
            let (b, s) = req(Action::Shutdown, 1000, n);
            let r = verify(S, &b, &s, &a, &mut ReplayGuard::new(), now);
            assert_eq!(r.unwrap_err(), Reject::Skew(31));
        }
    }

    #[test]
    fn replay_window() {
        let a = [Action::Shutdown];
        let mut g = ReplayGuard::new();
        let (b, s) = req(Action::Shutdown, 1000, 9);
        assert!(verify(S, &b, &s, &a, &mut g, 1000).is_ok());
        assert_eq!(
            verify(S, &b, &s, &a, &mut g, 1000).unwrap_err(),
            Reject::Replay
        );
        let (b2, s2) = req(Action::Shutdown, 1061, 9);
        assert!(verify(S, &b2, &s2, &a, &mut g, 1061).is_ok());
    }

    #[test]
    fn wrong_action() {
        let (b, s) = req(Action::Health, 1000, 1);
        let r = verify(
            S,
            &b,
            &s,
            &[Action::Shutdown, Action::Reboot],
            &mut ReplayGuard::new(),
            1000,
        );
        assert_eq!(r.unwrap_err(), Reject::WrongAction);
    }

    #[test]
    fn oversized() {
        let b = vec![b'a'; 1025];
        let s = sign(S, &b);
        let r = verify(S, &b, &s, &[Action::Health], &mut ReplayGuard::new(), 1000);
        assert_eq!(r.unwrap_err(), Reject::BadRequest);
    }

    #[test]
    fn messages() {
        assert_eq!(Reject::Skew(-45).message(), "clock skew 45s");
        assert_eq!(Reject::BadSignature.message(), "bad signature");
        assert_eq!(Reject::WrongAction.message(), "bad request");
        assert_eq!(Reject::Replay.message(), "replay");
    }

    #[test]
    fn secret_parse() {
        assert_eq!(parse_secret(&"ab".repeat(32)).unwrap().len(), 32);
        assert_eq!(parse_secret(&"a".repeat(63)), Err(Reject::BadRequest));
        assert_eq!(parse_secret(&"zz".repeat(32)), Err(Reject::BadRequest));
    }
}
