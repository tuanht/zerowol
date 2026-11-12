use crate::app::{ApiError, AppState, JsonBody};
use crate::auth::{Source, generate_token};
use crate::model::{Token, new_id, now};
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
}

#[derive(Serialize)]
pub struct TokenView {
    id: String,
    name: String,
    created: u64,
    last_used: Option<u64>,
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<TokenView>> {
    Json(
        state
            .store
            .snapshot()
            .tokens
            .into_iter()
            .map(|t| TokenView {
                id: t.id,
                name: t.name,
                created: t.created,
                last_used: t.last_used,
            })
            .collect(),
    )
}

pub async fn create(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    payload: JsonBody<CreateBody>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let JsonBody(b) = payload;
    let name = b.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError(StatusCode::BAD_REQUEST, "invalid name".into()));
    }
    let (plain, hash) = generate_token();
    let id = state.store.update(|st| {
        let mut id = new_id();
        while st.tokens.iter().any(|t| t.id == id) {
            id = new_id();
        }
        st.tokens.push(Token {
            id: id.clone(),
            name: name.clone(),
            hash,
            created: now(),
            last_used: None,
        });
        Ok::<_, ApiError>(id)
    })?;
    tracing::info!("action=token_create machine={name} source={source} result=ok");
    Ok((
        StatusCode::CREATED,
        Json(json!({"id": id, "name": name, "token": plain})),
    ))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let name = state.store.update(|st| {
        let i = st
            .tokens
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| ApiError(StatusCode::NOT_FOUND, "not found".into()))?;
        Ok::<_, ApiError>(st.tokens.remove(i).name)
    })?;
    tracing::info!("action=token_delete machine={name} source={source} result=ok");
    Ok(StatusCode::NO_CONTENT)
}
