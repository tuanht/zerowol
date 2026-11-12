use crate::app::AppState;
use crate::model::now;
use axum::Json;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Ui,
    Token(String),
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Source::Ui => write!(f, "ui"),
            Source::Token(n) => write!(f, "token:{n}"),
        }
    }
}

pub fn hash_token(t: &str) -> String {
    hex::encode(Sha256::digest(t.as_bytes()))
}

pub fn generate_token() -> (String, String) {
    let b: [u8; 32] = rand::rng().random();
    let plain = format!("zw_{}", URL_SAFE_NO_PAD.encode(b));
    let hash = hash_token(&plain);
    (plain, hash)
}

/// Compares every byte of both strings without early exit.
fn ct_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut diff = (a.len() ^ b.len()) as u8;
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, rest) = v.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| rest.trim().to_string())
}

fn is_tokens_path(path: &str) -> bool {
    let p = path.strip_prefix("/api/v1").unwrap_or(path);
    p == "/tokens" || p.starts_with("/tokens/")
}

pub async fn middleware(State(app): State<AppState>, mut req: Request, next: Next) -> Response {
    let has_auth = req.headers().contains_key(header::AUTHORIZATION);
    if has_auth && is_tokens_path(req.uri().path()) {
        return err(StatusCode::FORBIDDEN, "forbidden");
    }
    let source = if has_auth {
        let Some(t) = bearer(req.headers()) else {
            return err(StatusCode::UNAUTHORIZED, "unauthorized");
        };
        let h = hash_token(&t);
        let snap = app.store.snapshot();
        let mut found = None;
        for tok in &snap.tokens {
            if ct_eq(&tok.hash, &h) {
                found = Some(tok);
            }
        }
        let Some(tok) = found else {
            return err(StatusCode::UNAUTHORIZED, "unauthorized");
        };
        app.store.touch_token(&tok.id, now());
        Source::Token(tok.name.clone())
    } else {
        if app.cfg.require_token {
            let same = req
                .headers()
                .get("sec-fetch-site")
                .and_then(|v| v.to_str().ok())
                == Some("same-origin");
            if !same {
                return err(StatusCode::UNAUTHORIZED, "unauthorized");
            }
        }
        Source::Ui
    };
    req.extensions_mut().insert(source);
    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_shape() {
        let (p, h) = generate_token();
        assert!(p.starts_with("zw_"));
        assert_eq!(p.len(), 46);
        assert_eq!(hash_token(&p), h);
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn source_display() {
        assert_eq!(Source::Ui.to_string(), "ui");
        assert_eq!(Source::Token("zc".into()).to_string(), "token:zc");
    }

    #[test]
    fn ct_eq_works() {
        assert!(ct_eq("abc", "abc"));
        assert!(!ct_eq("abc", "abd"));
        assert!(!ct_eq("abc", "abcd"));
    }
}
