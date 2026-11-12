use crate::agent_client::{self, AgentError};
use crate::app::{ApiError, AppState, JsonBody};
use crate::auth::Source;
use crate::install;
use crate::model::{
    AgentCfg, Machine, default_broadcast, format_mac, new_id, new_secret, parse_mac,
};
use crate::status::{self, Status};
use crate::wol;
use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};
use zerowol_proto::Action;

const RATE_WINDOW: Duration = Duration::from_secs(10);
const DEFAULT_AGENT_PORT: u16 = 9009;

type ApiResult<T> = Result<T, ApiError>;

#[derive(Serialize)]
pub struct AgentView {
    port: u16,
}

#[derive(Serialize)]
pub struct MachineView {
    id: String,
    name: String,
    mac: String,
    ip: Ipv4Addr,
    broadcast: Ipv4Addr,
    wol_port: u16,
    probe_port: u16,
    agent: Option<AgentView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<Status>,
}

impl MachineView {
    fn new(m: &Machine, status: Option<Status>) -> Self {
        MachineView {
            id: m.id.clone(),
            name: m.name.clone(),
            mac: m.mac.clone(),
            ip: m.ip,
            broadcast: m.broadcast,
            wol_port: m.wol_port,
            probe_port: m.probe_port,
            agent: m.agent.as_ref().map(|a| AgentView { port: a.port }),
            status,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
    mac: String,
    ip: String,
    broadcast: Option<String>,
    wol_port: Option<u16>,
    probe_port: Option<u16>,
    #[serde(default)]
    agent: bool,
}

#[derive(Deserialize)]
pub struct PatchBody {
    name: Option<String>,
    mac: Option<String>,
    ip: Option<String>,
    broadcast: Option<String>,
    wol_port: Option<u16>,
    probe_port: Option<u16>,
    agent: Option<bool>,
}

fn bad(msg: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "not found".into())
}

fn valid_name(s: &str) -> ApiResult<String> {
    let t = s.trim();
    if t.is_empty() || t.chars().count() > 64 {
        return Err(bad("invalid name"));
    }
    Ok(t.to_string())
}

fn valid_mac(s: &str) -> ApiResult<String> {
    parse_mac(s).map(format_mac).map_err(|_| bad("invalid mac"))
}

fn valid_ip(s: &str, what: &str) -> ApiResult<Ipv4Addr> {
    s.trim().parse().map_err(|_| bad(what))
}

fn log(action: &str, machine: &str, source: &Source, result: &str) {
    tracing::info!("action={action} machine={machine} source={source} result={result}");
}

fn find(state: &AppState, id: &str) -> ApiResult<Machine> {
    state
        .store
        .snapshot()
        .machines
        .into_iter()
        .find(|m| m.id == id)
        .ok_or_else(not_found)
}

pub async fn list(State(state): State<AppState>) -> Json<Vec<MachineView>> {
    let ms = state.store.snapshot().machines;
    let sts = status::all_statuses(&ms).await;
    Json(
        ms.iter()
            .zip(sts)
            .map(|(m, s)| MachineView::new(m, Some(s)))
            .collect(),
    )
}

pub async fn create(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    payload: JsonBody<CreateBody>,
) -> ApiResult<Response> {
    let JsonBody(b) = payload;
    let name = valid_name(&b.name)?;
    let mac = valid_mac(&b.mac)?;
    let ip = valid_ip(&b.ip, "invalid ip")?;
    let broadcast = match &b.broadcast {
        Some(s) => valid_ip(s, "invalid broadcast")?,
        None => default_broadcast(ip),
    };
    let m = state.store.update(|st| {
        if st.machines.iter().any(|m| m.mac == mac) {
            return Err(bad("invalid mac: already used"));
        }
        let mut id = new_id();
        while st.machines.iter().any(|m| m.id == id) {
            id = new_id();
        }
        let m = Machine {
            id,
            name,
            mac,
            ip,
            broadcast,
            wol_port: b.wol_port.unwrap_or(9),
            probe_port: b.probe_port.unwrap_or(22),
            agent: b.agent.then(|| AgentCfg {
                port: DEFAULT_AGENT_PORT,
                secret: new_secret(),
            }),
        };
        st.machines.push(m.clone());
        Ok::<_, ApiError>(m)
    })?;
    log("create", &m.name, &source, "ok");
    Ok((StatusCode::CREATED, Json(MachineView::new(&m, None))).into_response())
}

pub async fn patch(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
    payload: JsonBody<PatchBody>,
) -> ApiResult<Json<MachineView>> {
    let JsonBody(b) = payload;
    let name = b.name.as_deref().map(valid_name).transpose()?;
    let mac = b.mac.as_deref().map(valid_mac).transpose()?;
    let ip =
        b.ip.as_deref()
            .map(|s| valid_ip(s, "invalid ip"))
            .transpose()?;
    let broadcast = b
        .broadcast
        .as_deref()
        .map(|s| valid_ip(s, "invalid broadcast"))
        .transpose()?;
    let m = state.store.update(|st| {
        if let Some(mac) = &mac
            && st.machines.iter().any(|m| m.id != id && &m.mac == mac)
        {
            return Err(bad("invalid mac: already used"));
        }
        let m = st
            .machines
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(not_found)?;
        if let Some(v) = name {
            m.name = v;
        }
        if let Some(v) = mac {
            m.mac = v;
        }
        if let Some(v) = ip {
            m.ip = v;
        }
        if let Some(v) = broadcast {
            m.broadcast = v;
        }
        if let Some(v) = b.wol_port {
            m.wol_port = v;
        }
        if let Some(v) = b.probe_port {
            m.probe_port = v;
        }
        match b.agent {
            Some(false) => m.agent = None,
            Some(true) if m.agent.is_none() => {
                m.agent = Some(AgentCfg {
                    port: DEFAULT_AGENT_PORT,
                    secret: new_secret(),
                })
            }
            _ => {}
        }
        Ok::<_, ApiError>(m.clone())
    })?;
    log("update", &m.name, &source, "ok");
    Ok(Json(MachineView::new(&m, None)))
}

pub async fn delete(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let name = state.store.update(|st| {
        let i = st
            .machines
            .iter()
            .position(|m| m.id == id)
            .ok_or_else(not_found)?;
        Ok::<_, ApiError>(st.machines.remove(i).name)
    })?;
    state
        .limiter
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    log("delete", &name, &source, "ok");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Status>> {
    let m = find(&state, &id)?;
    Ok(Json(status::machine_status(&m).await))
}

pub async fn wake(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let m = find(&state, &id)?;
    if status::machine_status(&m).await.online {
        log("wake", &m.name, &source, "already online");
        return Ok(Json(json!({"result":"already online"})));
    }
    let mac = parse_mac(&m.mac).map_err(|_| bad("invalid mac"))?;
    match wol::send(mac, m.broadcast, m.wol_port).await {
        Ok(()) => {
            log("wake", &m.name, &source, "sent");
            Ok(Json(json!({"result":"sent"})))
        }
        Err(e) => {
            log("wake", &m.name, &source, "error");
            Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

async fn power(
    state: AppState,
    source: Source,
    id: String,
    name: &str,
    action: Action,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let m = find(&state, &id)?;
    let Some(agent) = &m.agent else {
        return Err(ApiError(StatusCode::CONFLICT, "no agent".into()));
    };
    {
        let mut lim = state.limiter.lock().unwrap_or_else(|e| e.into_inner());
        let t = Instant::now();
        if let Some(prev) = lim.get(&m.id)
            && t.duration_since(*prev) < RATE_WINDOW
        {
            return Err(ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "rate limited".into(),
            ));
        }
        lim.insert(m.id.clone(), t);
    }
    match agent_client::power(m.ip, agent.port, &agent.secret, action).await {
        Ok(()) => {
            log(name, &m.name, &source, "accepted");
            Ok((StatusCode::ACCEPTED, Json(json!({"result":"accepted"}))))
        }
        Err(e @ (AgentError::Unreachable | AgentError::Rejected(_))) => {
            log(name, &m.name, &source, &e.message());
            Err(ApiError(StatusCode::BAD_GATEWAY, e.message()))
        }
    }
}

pub async fn shutdown(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    power(state, source, id, "shutdown", Action::Shutdown).await
}

pub async fn reboot(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    power(state, source, id, "reboot", Action::Reboot).await
}

pub async fn rotate(
    State(state): State<AppState>,
    Extension(source): Extension<Source>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let m = state.store.update(|st| {
        let m = st
            .machines
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(not_found)?;
        let a = m
            .agent
            .as_mut()
            .ok_or_else(|| ApiError(StatusCode::CONFLICT, "no agent".into()))?;
        a.secret = new_secret();
        Ok::<_, ApiError>(m.clone())
    })?;
    log("rotate", &m.name, &source, "ok");
    let (host, _) = install::host_for(&state, &headers, &m);
    Ok(Json(
        json!({"install_command": install::install_command(&host, &m.id)}),
    ))
}

pub async fn install_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let m = find(&state, &id)?;
    if m.agent.is_none() {
        return Err(ApiError(StatusCode::CONFLICT, "no agent".into()));
    }
    let (host, _) = install::host_for(&state, &headers, &m);
    Ok(Json(
        json!({"install_command": install::install_command(&host, &m.id)}),
    ))
}
