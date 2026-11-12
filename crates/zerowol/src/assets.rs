use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::borrow::Cow;

const NO_CACHE: &str = "no-cache";
const DAY: &str = "public, max-age=86400";

type Asset = (Cow<'static, [u8]>, &'static str, &'static str);

const SW: &str = include_str!("../web/sw.js");
const JS: &str = include_str!("../web/app.js");

pub fn get(path: &str) -> Option<Asset> {
    let b = |d: &'static [u8], ct, cc| Some((Cow::Borrowed(d), ct, cc));
    match path {
        "/" | "/index.html" => b(
            include_bytes!("../web/index.html"),
            "text/html; charset=utf-8",
            NO_CACHE,
        ),
        "/sw.js" => Some((
            Cow::Owned(
                SW.replace("__VERSION__", env!("CARGO_PKG_VERSION"))
                    .into_bytes(),
            ),
            "text/javascript; charset=utf-8",
            NO_CACHE,
        )),
        "/app.js" => Some((
            Cow::Owned(
                JS.replace("__VERSION__", env!("CARGO_PKG_VERSION"))
                    .into_bytes(),
            ),
            "text/javascript; charset=utf-8",
            NO_CACHE,
        )),
        "/app.css" => b(
            include_bytes!("../web/app.css"),
            "text/css; charset=utf-8",
            NO_CACHE,
        ),
        "/manifest.webmanifest" => b(
            include_bytes!("../web/manifest.webmanifest"),
            "application/manifest+json",
            NO_CACHE,
        ),
        "/icon.svg" => b(
            include_bytes!("../../../assets/icon.svg"),
            "image/svg+xml",
            DAY,
        ),
        "/icons/apple-touch-icon.png" => b(
            include_bytes!("../web/icons/apple-touch-icon.png"),
            "image/png",
            DAY,
        ),
        "/icons/icon-192.png" => b(
            include_bytes!("../web/icons/icon-192.png"),
            "image/png",
            DAY,
        ),
        "/icons/icon-512.png" => b(
            include_bytes!("../web/icons/icon-512.png"),
            "image/png",
            DAY,
        ),
        "/icons/icon-maskable-512.png" => b(
            include_bytes!("../web/icons/icon-maskable-512.png"),
            "image/png",
            DAY,
        ),
        _ => None,
    }
}

/// Router fallback: static assets for GET/HEAD, JSON 404 otherwise.
pub async fn serve(req: axum::extract::Request) -> Response {
    if matches!(
        *req.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) && let Some((body, ct, cc)) = get(req.uri().path())
    {
        return (
            [
                (header::CONTENT_TYPE, HeaderValue::from_static(ct)),
                (header::CACHE_CONTROL, HeaderValue::from_static(cc)),
            ],
            body.into_owned(),
        )
            .into_response();
    }
    (
        StatusCode::NOT_FOUND,
        axum::Json(serde_json::json!({ "error": "not found" })),
    )
        .into_response()
}
