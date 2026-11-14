mod common;
use common::{req, req_full, req_raw};

fn header<'a>(hs: &'a [(String, String)], k: &str) -> &'a str {
    hs.iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

#[tokio::test]
async fn index_served() {
    let (base, _s, _d) = common::spawn(false).await;
    let (st, hs, body) = req_full("GET", &format!("{base}/"), &[], None).await;
    assert_eq!(st, 200);
    assert!(header(&hs, "content-type").starts_with("text/html"));
    assert_eq!(header(&hs, "cache-control"), "no-cache");
    assert!(body.contains("apple-mobile-web-app-capable"));
    assert!(body.contains("manifest.webmanifest"));
}

#[tokio::test]
async fn static_types() {
    let (base, _s, _d) = common::spawn(false).await;
    for (p, ct) in [
        ("/app.js", "javascript"),
        ("/app.css", "text/css"),
        ("/sw.js", "javascript"),
        ("/icon.svg", "image/svg+xml"),
    ] {
        let (st, hs, _) = req_full("GET", &format!("{base}{p}"), &[], None).await;
        assert_eq!(st, 200, "{p}");
        assert!(header(&hs, "content-type").contains(ct), "{p}");
    }
    // Framework7 is bundled into /app.js now; the old vendor files are gone.
    let (st, v) = req("GET", &format!("{base}/vendor/framework7.js"), &[], None).await;
    assert_eq!(st, 404);
    assert_eq!(v["error"], "not found");
}

#[tokio::test]
async fn service_worker_versioned() {
    let (base, _s, _d) = common::spawn(false).await;
    let (st, body) = req_raw("GET", &format!("{base}/sw.js"), &[], None).await;
    assert_eq!(st, 200);
    assert!(!body.contains("__VERSION__"));
    assert!(body.contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn app_js_versioned() {
    // The Vite build keeps the literal __VERSION__ for assets.rs to substitute.
    let (base, _s, _d) = common::spawn(false).await;
    let (st, body) = req_raw("GET", &format!("{base}/app.js"), &[], None).await;
    assert_eq!(st, 200);
    assert!(!body.contains("__VERSION__"));
    assert!(body.contains(env!("CARGO_PKG_VERSION")));
}

#[tokio::test]
async fn manifest_valid() {
    let (base, _s, _d) = common::spawn(false).await;
    let (st, hs, body) = req_full("GET", &format!("{base}/manifest.webmanifest"), &[], None).await;
    assert_eq!(st, 200);
    assert!(header(&hs, "content-type").starts_with("application/manifest+json"));
    let v: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["display"], "standalone");
    let icons = v["icons"].as_array().unwrap();
    assert_eq!(icons.len(), 4);
    assert!(icons.iter().any(|i| i["purpose"] == "maskable"));
}

#[tokio::test]
async fn icon_png() {
    let (base, _s, _d) = common::spawn(false).await;
    for p in [
        "icon-512.png",
        "icon-192.png",
        "icon-maskable-512.png",
        "apple-touch-icon.png",
    ] {
        let (st, hs, body) = req_full("GET", &format!("{base}/icons/{p}"), &[], None).await;
        assert_eq!(st, 200, "{p}");
        assert_eq!(header(&hs, "content-type"), "image/png");
        assert!(header(&hs, "cache-control").contains("max-age=86400"));
        assert!(body.starts_with("\u{FFFD}PNG"), "{p}");
    }
}

#[tokio::test]
async fn unknown_paths_404_json() {
    let (base, _s, _d) = common::spawn(false).await;
    let (st, v) = req("GET", &format!("{base}/nope"), &[], None).await;
    assert_eq!(st, 404);
    assert_eq!(v["error"], "not found");
    let (st, v) = req("GET", &format!("{base}/api/v1/nope"), &[], None).await;
    assert_eq!(st, 404);
    assert_eq!(v["error"], "not found");
}

#[test]
fn budget() {
    // Only hand-written files directly in web/ count; read_dir is non-recursive, so
    // the Vite build output in web/dist/ (budgeted by scripts/web-size.mjs),
    // web/src/ and web/node_modules/ are intentionally excluded.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web");
    let total: u64 = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            matches!(
                e.path().extension().and_then(|x| x.to_str()),
                Some("html" | "css" | "js" | "webmanifest")
            )
        })
        .map(|e| e.metadata().unwrap().len())
        .sum();
    assert!(total < 61_440, "web assets {total} bytes");
}
