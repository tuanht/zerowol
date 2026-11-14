//! Wake and power actions shared by the HTTP handlers and the scheduler.

use crate::agent_client::{self, AgentError};
use crate::app::{ApiError, AppState};
use crate::auth::Source;
use crate::model::{Machine, parse_mac};
use crate::status;
use crate::wol;
use axum::http::StatusCode;
use std::time::{Duration, Instant};
use zerowol_proto::Action;

const RATE_WINDOW: Duration = Duration::from_secs(10);

pub(crate) fn log(action: &str, machine: &str, source: &Source, result: &str) {
    tracing::info!("action={action} machine={machine} source={source} result={result}");
}

#[derive(Debug, PartialEq)]
pub enum WakeOutcome {
    Sent,
    AlreadyOnline,
}

/// Sends the magic packet unless the machine is already online.
pub async fn wake(
    _state: &AppState,
    m: &Machine,
    source: &Source,
) -> Result<WakeOutcome, ApiError> {
    if status::machine_status(m).await.online {
        log("wake", &m.name, source, "already online");
        return Ok(WakeOutcome::AlreadyOnline);
    }
    let mac =
        parse_mac(&m.mac).map_err(|_| ApiError(StatusCode::BAD_REQUEST, "invalid mac".into()))?;
    #[cfg(feature = "dev-fakes")]
    if crate::fake::wake(m).is_some() {
        log("wake", &m.name, source, "sent (simulated)");
        return Ok(WakeOutcome::Sent);
    }
    match wol::send(mac, m.broadcast, m.wol_port).await {
        Ok(()) => {
            log("wake", &m.name, source, "sent");
            Ok(WakeOutcome::Sent)
        }
        Err(e) => {
            log("wake", &m.name, source, "error");
            Err(ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// Asks the machine's agent to shut down or reboot; at most one request per
/// machine every `RATE_WINDOW`. `name` is the action name used in the log.
pub async fn power(
    state: &AppState,
    m: &Machine,
    source: &Source,
    name: &str,
    action: Action,
) -> Result<(), ApiError> {
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
    #[cfg(feature = "dev-fakes")]
    let sim = crate::fake::power(m, action);
    #[cfg(not(feature = "dev-fakes"))]
    let sim = None;
    let res = match sim {
        Some(r) => r,
        None => agent_client::power(m.ip, agent.port, &agent.secret, action).await,
    };
    match res {
        Ok(()) => {
            log(name, &m.name, source, "accepted");
            Ok(())
        }
        Err(e @ (AgentError::Unreachable | AgentError::Rejected(_))) => {
            log(name, &m.name, source, &e.message());
            Err(ApiError(StatusCode::BAD_GATEWAY, e.message()))
        }
    }
}
