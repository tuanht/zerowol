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
    assert_eq!(c, 200);
    assert_eq!(b[0]["name"], "zc");
}

#[test]
fn hash_matches_generate() {
    let (p, h) = generate_token();
    assert_eq!(hash_token(&p), h);
}

// ---------- Task 7: machines & tokens ----------

fn mk(base: &str) -> String {
    format!("{base}/api/v1/machines")
}

async fn create(base: &str, body: serde_json::Value) -> (u16, serde_json::Value) {
    req("POST", &mk(base), &[], Some(&body.to_string())).await
}

fn set_machine(state: &AppState, id: &str, f: impl FnOnce(&mut zerowol::model::Machine)) {
    state
        .store
        .update(|st| {
            f(st.machines.iter_mut().find(|m| m.id == id).unwrap());
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

fn agent_secret(state: &AppState, id: &str) -> String {
    let s = state.store.snapshot();
    s.machines
        .iter()
        .find(|m| m.id == id)
        .unwrap()
        .agent
        .as_ref()
        .unwrap()
        .secret
        .clone()
}

/// Machine with an agent whose fake runs on localhost.
async fn agent_machine(
    base: &str,
    state: &AppState,
    reply: Option<(&'static str, &'static str)>,
) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    let (c, m) = create(
        base,
        json!({"name":"PC","mac":"AA:BB:CC:DD:EE:01","ip":"127.0.0.1","agent":true}),
    )
    .await;
    assert_eq!(c, 201);
    let id = m["id"].as_str().unwrap().to_string();
    let (port, seen) = fake_agent(&agent_secret(state, &id), reply).await;
    set_machine(state, &id, |m| m.agent.as_mut().unwrap().port = port);
    (id, seen)
}

#[tokio::test]
async fn create_list_get_patch_delete() {
    let (base, _s, _d) = spawn(false).await;
    let (c, m) = create(
        &base,
        json!({"name":"Desktop","mac":"d8-5e-d3-db-f7-6b","ip":"192.168.23.254","agent":false}),
    )
    .await;
    assert_eq!(c, 201);
    let id = m["id"].as_str().unwrap().to_string();
    assert_eq!(id.len(), 6);
    assert_eq!(m["mac"], "D8:5E:D3:DB:F7:6B");
    assert_eq!(m["broadcast"], "192.168.23.255");
    assert_eq!(
        (m["wol_port"].clone(), m["probe_port"].clone()),
        (json!(9), json!(22))
    );
    assert_eq!(m["agent"], json!(null));

    let (c, raw) = req_raw("GET", &mk(&base), &[], None).await;
    assert_eq!(c, 200);
    assert!(!raw.contains("secret"));
    let list: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(list[0]["id"], id.as_str());
    assert_eq!(list[0]["status"]["agent"], "none");

    // local machine with closed probe port -> offline, agent none
    let (c, _) = req(
        "PATCH",
        &format!("{}/{id}", mk(&base)),
        &[],
        Some(r#"{"ip":"127.0.0.1","probe_port":1}"#),
    )
    .await;
    assert_eq!(c, 200);
    let (c, st) = req("GET", &format!("{}/{id}/status", mk(&base)), &[], None).await;
    assert_eq!((c, st), (200, json!({"online":false,"agent":"none"})));

    let (c, p) = req(
        "PATCH",
        &format!("{}/{id}", mk(&base)),
        &[],
        Some(r#"{"name":"Renamed"}"#),
    )
    .await;
    assert_eq!((c, p["name"].clone()), (200, json!("Renamed")));
    assert_eq!(p["broadcast"], "192.168.23.255");

    let (c, _) = req("DELETE", &format!("{}/{id}", mk(&base)), &[], None).await;
    assert_eq!(c, 204);
    let (c, b) = req("GET", &mk(&base), &[], None).await;
    assert_eq!((c, b), (200, json!([])));
    let (c, b) = req("DELETE", &format!("{}/{id}", mk(&base)), &[], None).await;
    assert_eq!((c, b), (404, json!({"error":"not found"})));
}

#[tokio::test]
async fn invalid_inputs() {
    let (base, _s, _d) = spawn(false).await;
    let ok = json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"10.0.0.2","agent":false});
    let cases = [
        (("mac", json!("nope")), "invalid mac"),
        (("ip", json!("999.1.1.1")), "invalid ip"),
        (("broadcast", json!("x")), "invalid broadcast"),
        (("name", json!("")), "invalid name"),
        (("name", json!("x".repeat(65))), "invalid name"),
    ];
    for ((k, v), msg) in cases {
        let mut body = ok.clone();
        body[k] = v;
        let (c, b) = create(&base, body).await;
        assert_eq!((c, b), (400, json!({"error":msg})), "{k}");
    }
    let (c, b) = req("POST", &mk(&base), &[], Some("{not json")).await;
    assert_eq!((c, b), (400, json!({"error":"bad request"})));
    let (c, b) = req("PATCH", &format!("{}/zzzzzz", mk(&base)), &[], Some("{}")).await;
    assert_eq!((c, b), (404, json!({"error":"not found"})));
}

#[tokio::test]
async fn duplicate_mac_rejected() {
    let (base, _s, _d) = spawn(false).await;
    let (_, a) = create(
        &base,
        json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"10.0.0.2","agent":false}),
    )
    .await;
    let (c, b) = create(
        &base,
        json!({"name":"B","mac":"aa-bb-cc-dd-ee-01","ip":"10.0.0.3","agent":false}),
    )
    .await;
    assert_eq!((c, b), (400, json!({"error":"invalid mac: already used"})));
    let (_, other) = create(
        &base,
        json!({"name":"C","mac":"AA:BB:CC:DD:EE:02","ip":"10.0.0.4","agent":false}),
    )
    .await;
    let oid = other["id"].as_str().unwrap();
    let (c, b) = req(
        "PATCH",
        &format!("{}/{oid}", mk(&base)),
        &[],
        Some(r#"{"mac":"AA:BB:CC:DD:EE:01"}"#),
    )
    .await;
    assert_eq!((c, b), (400, json!({"error":"invalid mac: already used"})));
    // patching a machine to its own MAC is fine
    let aid = a["id"].as_str().unwrap();
    let (c, _) = req(
        "PATCH",
        &format!("{}/{aid}", mk(&base)),
        &[],
        Some(r#"{"mac":"aa:bb:cc:dd:ee:01"}"#),
    )
    .await;
    assert_eq!(c, 200);
}

#[tokio::test]
async fn agent_toggle() {
    let (base, s, _d) = spawn(false).await;
    let (_, m) = create(
        &base,
        json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"10.0.0.2","agent":true}),
    )
    .await;
    let id = m["id"].as_str().unwrap();
    assert_eq!(m["agent"], json!({"port":9009}));
    let secret = agent_secret(&s, id);
    assert_eq!(secret.len(), 64);
    assert!(secret.chars().all(|c| c.is_ascii_hexdigit()));
    let url = format!("{}/{id}", mk(&base));
    let (_, p) = req("PATCH", &url, &[], Some(r#"{"agent":true}"#)).await;
    assert_eq!(p["agent"], json!({"port":9009}));
    assert_eq!(agent_secret(&s, id), secret, "secret kept");
    let (_, p) = req("PATCH", &url, &[], Some(r#"{"agent":false}"#)).await;
    assert_eq!(p["agent"], json!(null));
    let (_, p) = req("PATCH", &url, &[], Some(r#"{"agent":true}"#)).await;
    assert_ne!(p["agent"], json!(null));
    assert_ne!(agent_secret(&s, id), secret);
}

#[tokio::test]
async fn rotate_changes_secret() {
    let (base, s, _d) = spawn(false).await;
    let (id, _) = agent_machine(&base, &s, None).await;
    let old = agent_secret(&s, &id);
    let (c, b) = req(
        "POST",
        &format!("{}/{id}/agent/rotate", mk(&base)),
        &[],
        None,
    )
    .await;
    assert_eq!(c, 200);
    assert!(
        b["install_command"]
            .as_str()
            .unwrap()
            .contains(&format!("/agent/install/{id}.sh"))
    );
    assert_ne!(agent_secret(&s, &id), old);
}

#[tokio::test]
async fn wake_sends_packet() {
    let (base, _s, _d) = spawn(false).await;
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let wol_port = sock.local_addr().unwrap().port();
    let (_, m) = create(
        &base,
        json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"127.0.0.1","broadcast":"127.0.0.1",
               "wol_port":wol_port,"probe_port":1,"agent":false}),
    )
    .await;
    let id = m["id"].as_str().unwrap();
    let (c, b) = req("POST", &format!("{}/{id}/wake", mk(&base)), &[], None).await;
    assert_eq!((c, b), (200, json!({"result":"sent"})));
    let mut buf = [0u8; 256];
    let n = tokio::time::timeout(std::time::Duration::from_secs(2), sock.recv(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(n, 102);
}

#[tokio::test]
async fn wake_already_online() {
    let (base, _s, _d) = spawn(false).await;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = l.local_addr().unwrap().port();
    let (_, m) = create(
        &base,
        json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"127.0.0.1","probe_port":port,"agent":false}),
    )
    .await;
    let id = m["id"].as_str().unwrap();
    let (c, b) = req("POST", &format!("{}/{id}/wake", mk(&base)), &[], None).await;
    assert_eq!((c, b), (200, json!({"result":"already online"})));
}

#[tokio::test]
async fn shutdown_no_agent() {
    let (base, _s, _d) = spawn(false).await;
    let (_, m) = create(
        &base,
        json!({"name":"A","mac":"AA:BB:CC:DD:EE:01","ip":"10.0.0.2","agent":false}),
    )
    .await;
    let id = m["id"].as_str().unwrap();
    let (c, _) = req("POST", &format!("{}/{id}/shutdown", mk(&base)), &[], None).await;
    assert_eq!(c, 409);
}

#[tokio::test]
async fn shutdown_ok_then_rate_limited_and_reboot() {
    let (base, s, _d) = spawn(false).await;
    let (id, seen) = agent_machine(&base, &s, None).await;
    let (c, b) = req("POST", &format!("{}/{id}/shutdown", mk(&base)), &[], None).await;
    assert_eq!((c, b), (202, json!({"result":"accepted"})));
    assert_eq!(*seen.lock().unwrap(), vec!["shutdown"]);
    let (c, b) = req("POST", &format!("{}/{id}/shutdown", mk(&base)), &[], None).await;
    assert_eq!((c, b), (429, json!({"error":"rate limited"})));
    let (c, _) = req("POST", &format!("{}/{id}/reboot", mk(&base)), &[], None).await;
    assert_eq!(c, 429, "limit is per machine across both actions");
    assert_eq!(seen.lock().unwrap().len(), 1);
    // Age the limiter entry instead of sleeping 10 s.
    {
        let mut lim = s.limiter.lock().unwrap();
        let old = std::time::Instant::now() - std::time::Duration::from_secs(11);
        lim.insert(id.clone(), old);
    }
    let (c, _) = req("POST", &format!("{}/{id}/reboot", mk(&base)), &[], None).await;
    assert_eq!(c, 202);
    assert_eq!(*seen.lock().unwrap(), vec!["shutdown", "reboot"]);
}

#[tokio::test]
async fn agent_skew_passthrough() {
    let (base, s, _d) = spawn(false).await;
    let (id, _) = agent_machine(
        &base,
        &s,
        Some(("401 Unauthorized", r#"{"error":"clock skew 45s"}"#)),
    )
    .await;
    let (c, b) = req("POST", &format!("{}/{id}/shutdown", mk(&base)), &[], None).await;
    assert_eq!((c, b), (502, json!({"error":"clock skew 45s"})));
}

#[tokio::test]
async fn agent_unreachable() {
    let (base, s, _d) = spawn(false).await;
    let (id, _) = agent_machine(&base, &s, None).await;
    let closed = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    set_machine(&s, &id, |m| m.agent.as_mut().unwrap().port = closed);
    let (c, b) = req("POST", &format!("{}/{id}/shutdown", mk(&base)), &[], None).await;
    assert_eq!((c, b), (502, json!({"error":"agent unreachable"})));
}

#[tokio::test]
async fn paired_status_in_list() {
    let (base, s, _d) = spawn(false).await;
    let (id, _) = agent_machine(&base, &s, None).await;
    let (_, st) = req("GET", &format!("{}/{id}/status", mk(&base)), &[], None).await;
    assert_eq!(st["agent"], "paired");
    assert_eq!(st["hostname"], "fake");
}

#[tokio::test]
async fn tokens_lifecycle() {
    let (base, s, _d) = spawn(false).await;
    let turl = format!("{base}/api/v1/tokens");
    let (c, b) = req("POST", &turl, &[], Some(r#"{"name":""}"#)).await;
    assert_eq!((c, b), (400, json!({"error":"invalid name"})));
    let (c, t) = req("POST", &turl, &[], Some(r#"{"name":"bot"}"#)).await;
    assert_eq!(c, 201);
    let token = t["token"].as_str().unwrap().to_string();
    assert!(token.starts_with("zw_"));
    let id = t["id"].as_str().unwrap().to_string();

    let (c, raw) = req_raw("GET", &turl, &[], None).await;
    assert_eq!(c, 200);
    assert!(!raw.contains("token") && !raw.contains("hash"));
    let l: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(l[0]["name"], "bot");
    assert_eq!(l[0]["last_used"], json!(null));

    let h = format!("Bearer {token}");
    let (c, _) = req("GET", &mk(&base), &[("Authorization", &h)], None).await;
    assert_eq!(c, 200);
    assert!(s.store.snapshot().tokens[0].last_used.is_some());

    let (c, _) = req("DELETE", &format!("{turl}/{id}"), &[], None).await;
    assert_eq!(c, 204);
    let (c, _) = req("GET", &mk(&base), &[("Authorization", &h)], None).await;
    assert_eq!(c, 401);
    let (c, _) = req("DELETE", &format!("{turl}/{id}"), &[], None).await;
    assert_eq!(c, 404);
}
