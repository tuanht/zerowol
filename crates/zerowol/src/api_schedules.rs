use crate::api_machines::log;
use crate::app::{ApiError, AppState, JsonBody};
use crate::auth::Source;
use crate::model::{Machine, Repeat, SchedAction, Schedule, State, Weekday, new_id, now};
use crate::schedule;
use axum::Json;
use axum::extract::{Extension, Path, Query, State as AxState};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use jiff::tz::TimeZone;
use serde_json::{Value, json};
use std::collections::HashMap;

type ApiResult<T> = Result<T, ApiError>;

const MAX_RANGE_DAYS: i32 = 62;

fn bad(msg: &str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, msg.into())
}

fn not_found() -> ApiError {
    ApiError(StatusCode::NOT_FOUND, "not found".into())
}

fn no_tz() -> ApiError {
    ApiError(StatusCode::CONFLICT, "timezone not set".into())
}

fn needs_agent() -> ApiError {
    bad("invalid action: needs agent")
}

fn tz_of(state: &AppState, st: &State) -> Option<(String, TimeZone)> {
    schedule::effective_tz(&st.settings, state.cfg.tz_env.as_deref())
}

fn today(tz: &TimeZone) -> jiff::civil::Date {
    jiff::Timestamp::now().to_zoned(tz.clone()).date()
}

fn parse_action(v: &Value) -> ApiResult<SchedAction> {
    serde_json::from_value(v.clone()).map_err(|_| bad("invalid action"))
}

fn parse_time(v: &Value) -> ApiResult<String> {
    let s = v.as_str().ok_or_else(|| bad("invalid time"))?;
    schedule::parse_time(s).ok_or_else(|| bad("invalid time"))?;
    Ok(s.to_string())
}

fn parse_repeat(v: &Value) -> ApiResult<Repeat> {
    let kind = v
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("invalid repeat"))?;
    match kind {
        "daily" => Ok(Repeat::Daily),
        "weekdays" => Ok(Repeat::Weekdays),
        "weekends" => Ok(Repeat::Weekends),
        "once" => {
            let d = v
                .get("date")
                .and_then(Value::as_str)
                .ok_or_else(|| bad("invalid date"))?;
            schedule::parse_date(d).ok_or_else(|| bad("invalid date"))?;
            Ok(Repeat::Once {
                date: d.to_string(),
            })
        }
        "weekly" => {
            let arr = v
                .get("days")
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
                .ok_or_else(|| bad("invalid days"))?;
            let mut days: Vec<Weekday> = Vec::new();
            for d in arr {
                let d: Weekday =
                    serde_json::from_value(d.clone()).map_err(|_| bad("invalid days"))?;
                if !days.contains(&d) {
                    days.push(d);
                }
            }
            Ok(Repeat::Weekly { days })
        }
        _ => Err(bad("invalid repeat")),
    }
}

fn repeat_kind(r: &Repeat) -> &'static str {
    match r {
        Repeat::Once { .. } => "once",
        Repeat::Daily => "daily",
        Repeat::Weekdays => "weekdays",
        Repeat::Weekends => "weekends",
        Repeat::Weekly { .. } => "weekly",
    }
}

/// Drop skip dates before today (effective timezone); unparseable ones too.
fn prune_skips(s: &mut Schedule, tz: &TimeZone) {
    let t = today(tz);
    s.skip
        .retain(|d| schedule::parse_date(d).is_some_and(|d| d >= t));
}

/// An enabled power action needs the machine's agent.
fn check_agent(s: &Schedule, m: &Machine) -> ApiResult<()> {
    if s.enabled && s.action != SchedAction::Wake && m.agent.is_none() {
        return Err(needs_agent());
    }
    Ok(())
}

/// A one-off rule must lie in the future.
fn check_future(s: &Schedule, tz: &TimeZone) -> ApiResult<()> {
    if let Repeat::Once { date } = &s.repeat {
        let (Some(d), Some(t)) = (schedule::parse_date(date), schedule::parse_time(&s.time)) else {
            return Ok(());
        };
        // parse_date bounds the year, so `instant` is always Some here; an
        // unrepresentable instant is rejected rather than accepted.
        let Some(at) = schedule::instant(tz, d, t) else {
            return Err(bad("invalid date"));
        };
        if at <= jiff::Timestamp::now() {
            return Err(bad("time already passed"));
        }
    }
    Ok(())
}

pub async fn list(AxState(state): AxState<AppState>) -> Json<Vec<Schedule>> {
    Json(state.store.snapshot().schedules)
}

pub async fn create(
    AxState(state): AxState<AppState>,
    Extension(source): Extension<Source>,
    payload: JsonBody<Value>,
) -> ApiResult<Response> {
    let JsonBody(b) = payload;
    let s = state.store.update(|st| {
        let (_, tz) = tz_of(&state, st).ok_or_else(no_tz)?;
        let m = b
            .get("machine_id")
            .and_then(Value::as_str)
            .and_then(|id| st.machines.iter().find(|m| m.id == id))
            .ok_or_else(|| bad("invalid machine"))?;
        let action = parse_action(b.get("action").unwrap_or(&Value::Null))?;
        let enabled = match b.get("enabled") {
            None => true,
            Some(Value::Bool(v)) => *v,
            Some(_) => return Err(bad("bad request")),
        };
        let mut s = Schedule {
            id: String::new(),
            machine_id: m.id.clone(),
            action,
            time: String::new(),
            repeat: Repeat::Daily,
            enabled,
            skip: Vec::new(),
            created: now(),
        };
        check_agent(&s, m)?;
        s.time = parse_time(b.get("time").unwrap_or(&Value::Null))?;
        s.repeat = parse_repeat(b.get("repeat").unwrap_or(&Value::Null))?;
        if s.enabled {
            check_future(&s, &tz)?;
        }
        let mut id = new_id();
        while st.schedules.iter().any(|x| x.id == id) {
            id = new_id();
        }
        s.id = id;
        st.schedules.push(s.clone());
        Ok::<_, ApiError>(s)
    })?;
    log("schedule-create", &s.machine_id, &source, "ok");
    Ok((StatusCode::CREATED, Json(s)).into_response())
}

pub async fn patch(
    AxState(state): AxState<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
    payload: JsonBody<Value>,
) -> ApiResult<Json<Schedule>> {
    let JsonBody(b) = payload;
    let s = state.store.update(|st| {
        let tz = tz_of(&state, st).map(|(_, z)| z);
        let idx = st
            .schedules
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(not_found)?;
        let mut s = st.schedules[idx].clone();
        let mut timing = false;
        if let Some(v) = b.get("action") {
            s.action = parse_action(v)?;
        }
        if let Some(v) = b.get("time") {
            s.time = parse_time(v)?;
            timing = true;
        }
        if let Some(v) = b.get("repeat") {
            s.repeat = parse_repeat(v)?;
            timing = true;
        }
        if let Some(v) = b.get("enabled") {
            s.enabled = v.as_bool().ok_or_else(|| bad("bad request"))?;
            timing |= s.enabled;
        }
        let m = st
            .machines
            .iter()
            .find(|m| m.id == s.machine_id)
            .ok_or_else(not_found)?;
        check_agent(&s, m)?;
        if s.enabled && timing {
            check_future(&s, tz.as_ref().ok_or_else(no_tz)?)?;
        }
        if let Some(tz) = &tz {
            prune_skips(&mut s, tz);
        }
        st.schedules[idx] = s.clone();
        Ok::<_, ApiError>(s)
    })?;
    log("schedule-update", &s.machine_id, &source, "ok");
    Ok(Json(s))
}

pub async fn delete(
    AxState(state): AxState<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let machine = state.store.update(|st| {
        let i = st
            .schedules
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(not_found)?;
        let s = st.schedules.remove(i);
        st.schedule_log.retain(|e| e.schedule_id != id);
        Ok::<_, ApiError>(s.machine_id)
    })?;
    log("schedule-delete", &machine, &source, "ok");
    Ok(StatusCode::NO_CONTENT)
}

fn edit_skip(
    state: &AppState,
    source: &Source,
    id: &str,
    date: &str,
    add: bool,
) -> ApiResult<Json<Schedule>> {
    let d = schedule::parse_date(date).ok_or_else(|| bad("invalid date"))?;
    let s = state.store.update(|st| {
        let (_, tz) = tz_of(state, st).ok_or_else(no_tz)?;
        let s = st
            .schedules
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(not_found)?;
        prune_skips(s, &tz);
        if add {
            if schedule::expand(s, &tz, d, d).is_empty() {
                return Err(bad("invalid date"));
            }
            if !s.skip.iter().any(|x| x == date) {
                s.skip.push(date.to_string());
                s.skip.sort();
            }
        } else {
            s.skip.retain(|x| x != date);
        }
        Ok::<_, ApiError>(s.clone())
    })?;
    log(
        if add {
            "schedule-skip"
        } else {
            "schedule-unskip"
        },
        &s.machine_id,
        source,
        "ok",
    );
    Ok(Json(s))
}

pub async fn skip(
    AxState(state): AxState<AppState>,
    Extension(source): Extension<Source>,
    Path(id): Path<String>,
    payload: JsonBody<Value>,
) -> ApiResult<Json<Schedule>> {
    let JsonBody(b) = payload;
    let date = b
        .get("date")
        .and_then(Value::as_str)
        .ok_or_else(|| bad("invalid date"))?;
    edit_skip(&state, &source, &id, date, true)
}

pub async fn unskip(
    AxState(state): AxState<AppState>,
    Extension(source): Extension<Source>,
    Path((id, date)): Path<(String, String)>,
) -> ApiResult<Json<Schedule>> {
    edit_skip(&state, &source, &id, &date, false)
}

pub async fn occurrences(
    AxState(state): AxState<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<Value>>> {
    let invalid = || bad("invalid range");
    let date = |k: &str| {
        q.get(k)
            .and_then(|s| schedule::parse_date(s))
            .ok_or_else(invalid)
    };
    let (from, to) = (date("from")?, date("to")?);
    if from > to || (to - from).get_days() > MAX_RANGE_DAYS {
        return Err(invalid());
    }
    let st = state.store.snapshot();
    let Some((_, tz)) = tz_of(&state, &st) else {
        return Ok(Json(Vec::new()));
    };
    let mut out: Vec<(i64, String, Value)> = Vec::new();
    for s in &st.schedules {
        let Some(m) = st.machines.iter().find(|m| m.id == s.machine_id) else {
            continue;
        };
        for o in schedule::expand(s, &tz, from, to) {
            let date = o.date.to_string();
            let outcome = st
                .schedule_log
                .iter()
                .rev()
                .find(|e| e.schedule_id == s.id && e.date == date)
                .map(|e| json!({"result": e.result, "detail": e.detail, "at": e.at}));
            let at = o.at.as_second();
            out.push((
                at,
                s.id.clone(),
                json!({
                    "schedule_id": s.id,
                    "machine_id": s.machine_id,
                    "machine_name": m.name,
                    "action": s.action,
                    "repeat_kind": repeat_kind(&s.repeat),
                    "date": date,
                    "time": s.time,
                    "at": at,
                    "skipped": o.skipped,
                    "enabled": s.enabled,
                    "outcome": outcome,
                }),
            ));
        }
    }
    out.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    Ok(Json(out.into_iter().map(|(_, _, v)| v).collect()))
}
