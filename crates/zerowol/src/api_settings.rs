use crate::api_machines::log;
use crate::app::{ApiError, AppState, JsonBody};
use crate::auth::Source;
use crate::schedule;
use axum::Json;
use axum::extract::{Extension, State};
use axum::http::StatusCode;
use serde_json::{Value, json};

fn view(state: &AppState) -> Value {
    let st = state.store.snapshot();
    let tz_env = state.cfg.tz_env.as_deref();
    json!({
        "timezone": schedule::effective_tz(&st.settings, tz_env).map(|(n, _)| n),
        "source": schedule::tz_source(&st.settings, tz_env),
    })
}

pub async fn get(State(state): State<AppState>) -> Json<Value> {
    Json(view(&state))
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    payload: JsonBody<Value>,
) -> Result<Json<Value>, ApiError> {
    let JsonBody(b) = payload;
    let bad = |m: &str| ApiError(StatusCode::BAD_REQUEST, m.into());
    let tz = match b.get("timezone") {
        None => return Err(bad("bad request")),
        Some(Value::Null) => None,
        Some(Value::String(n)) if schedule::zone(n).is_some() => Some(n.clone()),
        Some(_) => return Err(bad("invalid timezone")),
    };
    state.store.update(|st| {
        st.settings.timezone = tz;
        Ok::<_, ApiError>(())
    })?;
    log("settings-update", "-", &source, "ok");
    Ok(Json(view(&state)))
}
