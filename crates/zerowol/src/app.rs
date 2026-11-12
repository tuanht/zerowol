use crate::config::Config;
use crate::store::{Store, StoreError};
use crate::{api_machines, api_tokens, assets, auth, install};
use axum::Json;
use axum::Router;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
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

async fn api_not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "not found".into())
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route(
            "/machines",
            get(api_machines::list).post(api_machines::create),
        )
        .route(
            "/machines/{id}",
            patch(api_machines::patch).delete(api_machines::delete),
        )
        .route("/machines/{id}/status", get(api_machines::get_status))
        .route("/machines/{id}/wake", post(api_machines::wake))
        .route("/machines/{id}/shutdown", post(api_machines::shutdown))
        .route("/machines/{id}/reboot", post(api_machines::reboot))
        .route("/machines/{id}/agent/rotate", post(api_machines::rotate))
        .route(
            "/machines/{id}/agent/install-command",
            get(api_machines::install_command),
        )
        .route("/tokens", get(api_tokens::list).post(api_tokens::create))
        .route("/tokens/{id}", delete(api_tokens::delete))
        .fallback(api_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::middleware,
        ));
    Router::new()
        .route("/healthz", get(healthz))
        .route("/agent/install/{file}", get(install::script))
        .route("/agent/bin/{arch}", get(install::bin))
        .nest("/api/v1", api)
        .fallback(assets::serve)
        .with_state(state)
}

/// JSON body extractor that ignores Content-Type (curl -d sends form-encoded)
/// and reports failures as the JSON `bad request` error.
pub struct JsonBody<T>(pub T);

impl<T, S> axum::extract::FromRequest<S> for JsonBody<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, ApiError> {
        let bad = || ApiError(StatusCode::BAD_REQUEST, "bad request".into());
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|_| bad())?;
        serde_json::from_slice(&bytes)
            .map(JsonBody)
            .map_err(|_| bad())
    }
}
