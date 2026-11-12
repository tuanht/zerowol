mod common;
use common::*;
use serde_json::json;
use zerowol::app::AppState;
use zerowol::auth::{generate_token, hash_token};
use zerowol::model::Token;
use zerowol::store::StoreError;

fn add_token(state: &AppState, name: &str) -> String {
    let (plain, hash) = generate_token();
    state
        .store
        .update(|st| {
            st.tokens.push(Token {
                id: "t1".into(),
                name: name.into(),
                hash,
                created: 1,
                last_used: None,
            });
            Ok::<_, StoreError>(())
        })
        .unwrap();
    plain
}

#[tokio::test]
async fn healthz_ok() {
    let (base, _s, _d) = spawn(true).await;
    let (c, b) = req_raw("GET", &format!("{base}/healthz"), &[], None).await;
    assert_eq!((c, b.as_str()), (200, "ok"));
}

#[tokio::test]
async fn no_header_open_mode() {
    let (base, _s, _d) = spawn(false).await;
    let (c, b) = req("GET", &format!("{base}/api/v1/machines"), &[], None).await;
    assert_eq!((c, b), (200, json!([])));
}

#[tokio::test]
async fn valid_bearer_ok_and_touches() {
    let (base, s, _d) = spawn(true).await;
    let t = add_token(&s, "zc");
    let h = format!("Bearer {t}");
    let (c, _) = req(
        "GET",
        &format!("{base}/api/v1/machines"),
        &[("Authorization", &h)],
        None,
    )
    .await;
    assert_eq!(c, 200);
    assert!(s.store.snapshot().tokens[0].last_used.is_some());
}

#[tokio::test]
async fn invalid_bearer_401() {
    let (base, s, _d) = spawn(false).await;
    add_token(&s, "zc");
    let (c, b) = req(
        "GET",
        &format!("{base}/api/v1/machines"),
        &[("Authorization", "Bearer zw_nope")],
        None,
    )
    .await;
    assert_eq!((c, b), (401, json!({"error":"unauthorized"})));
    assert!(s.store.snapshot().tokens[0].last_used.is_none());
}

#[tokio::test]
async fn require_token_matrix() {
    let (base, _s, _d) = spawn(true).await;
    let url = format!("{base}/api/v1/machines");
    let (c, b) = req("GET", &url, &[], None).await;
    assert_eq!((c, b), (401, json!({"error":"unauthorized"})));
    let (c, _) = req("GET", &url, &[("Sec-Fetch-Site", "same-origin")], None).await;
    assert_eq!(c, 200);
    let (c, _) = req("GET", &url, &[("Sec-Fetch-Site", "cross-site")], None).await;
    assert_eq!(c, 401);
}

#[tokio::test]
async fn tokens_path_rejects_any_bearer() {
    let (base, s, _d) = spawn(false).await;
    let t = add_token(&s, "zc");
    let h = format!("Bearer {t}");
    let url = format!("{base}/api/v1/tokens");
    let (c, b) = req("GET", &url, &[("Authorization", &h)], None).await;
    assert_eq!((c, b), (403, json!({"error":"forbidden"})));
    let (c, _) = req("GET", &url, &[("Authorization", "Bearer bogus")], None).await;
    assert_eq!(c, 403);
    let (c, _) = req(
        "DELETE",
        &format!("{url}/t1"),
        &[("Authorization", &h)],
        None,
    )
    .await;
    assert_eq!(c, 403);
    let (c, b) = req("GET", &url, &[], None).await;
    assert_eq!((c, b), (200, json!([])));
}

#[test]
fn hash_matches_generate() {
    let (p, h) = generate_token();
    assert_eq!(hash_token(&p), h);
}
