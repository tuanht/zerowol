use crate::auth;
use crate::config::Config;
use crate::store::{Store, StoreError};
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub cfg: Arc<Config>,
    pub limiter: Arc<Mutex<HashMap<String, Instant>>>,
}

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub String);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(_: StoreError) -> Self {
        ApiError(StatusCode::INTERNAL_SERVER_ERROR, "could not save".into())
    }
}

async fn healthz() -> &'static str {
    "ok"
}

async fn empty_list() -> Json<serde_json::Value> {
    Json(serde_json::json!([]))
}

async fn api_not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "not found".into())
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/machines", get(empty_list))
        .route("/tokens", get(empty_list))
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware,
        ));
    Router::new()
        .route("/healthz", get(healthz))
        .nest("/api/v1", api)
        .with_state(state)
}
