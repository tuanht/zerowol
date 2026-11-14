//! Fires due schedules. Every firing is logged (`firing`) before the action
//! runs, so an occurrence never fires twice, even across a crash or restart.

use crate::actions::{self, WakeOutcome};
use crate::app::{ApiError, AppState};
use crate::auth::Source;
use crate::model::{LogEntry, LogResult, Machine, Repeat, SchedAction};
use crate::schedule::{self, Due, GRACE_SECS, LOOKBACK_SECS};
use crate::store::StoreError;
use jiff::{SignedDuration, Timestamp};
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::task::JoinSet;
use zerowol_proto::Action;

const TICK: Duration = Duration::from_secs(15);

fn secs(t: Timestamp) -> u64 {
    t.as_second().max(0) as u64
}

/// Where the startup pass starts looking: the last heartbeat, but at most
/// `LOOKBACK_SECS` back; without a heartbeat, just the grace window.
pub fn startup_after(heartbeat: Option<u64>, now: Timestamp) -> Timestamp {
    match heartbeat {
        Some(hb) => {
            let floor = now - SignedDuration::from_secs(LOOKBACK_SECS);
            let hb = i64::try_from(hb)
                .ok()
                .and_then(|s| Timestamp::from_second(s).ok())
                .unwrap_or(floor);
            hb.max(floor)
        }
        None => now - SignedDuration::from_secs(GRACE_SECS),
    }
}

/// The scheduler loop: a startup pass from the last heartbeat (older
/// occurrences are logged missed), then a pass every 15 s.
pub async fn run(state: AppState) {
    let mut prev = Timestamp::now();
    let after = startup_after(state.store.snapshot().scheduler_heartbeat, prev);
    tick(&state, after, prev).await;
    state.store.touch_heartbeat(secs(prev));
    let mut every = tokio::time::interval(TICK);
    every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    every.tick().await; // the first tick completes immediately
    loop {
        every.tick().await;
        let now = Timestamp::now();
        tick(&state, prev, now).await;
        state.store.touch_heartbeat(secs(now));
        prev = now;
    }
}

/// One pass over `(after, now]`: logs missed occurrences, then fires the due
/// ones and waits for them. Machines fire concurrently; one machine's
/// occurrences fire in time order. Does nothing without an effective timezone.
pub async fn tick(state: &AppState, after: Timestamp, now: Timestamp) {
    let snap = state.store.snapshot();
    let Some((_, tz)) = schedule::effective_tz(&snap.settings, state.cfg.tz_env.as_deref()) else {
        return;
    };
    let (fire, missed) = schedule::select(&snap, &tz, after, now);
    let stamp = secs(now);
    if !missed.is_empty() {
        log_missed(state, &snap.machines, &missed, stamp);
    }
    let mut by_machine: BTreeMap<String, Vec<Due>> = BTreeMap::new();
    for d in fire {
        by_machine.entry(d.machine_id.clone()).or_default().push(d);
    }
    let mut set = JoinSet::new();
    for (_, dues) in by_machine {
        let state = state.clone();
        set.spawn(async move {
            for d in dues {
                fire_one(&state, d, stamp).await;
            }
        });
    }
    while let Some(r) = set.join_next().await {
        if let Err(e) = r {
            tracing::error!("scheduler: firing task failed: {e}");
        }
    }
}

fn logged(st: &crate::model::State, d: &Due) -> bool {
    st.schedule_log
        .iter()
        .any(|e| e.schedule_id == d.schedule_id && e.date == d.date)
}

fn entry(d: &Due, at: u64, result: LogResult, detail: Option<String>) -> LogEntry {
    LogEntry {
        schedule_id: d.schedule_id.clone(),
        date: d.date.clone(),
        at,
        result,
        detail,
    }
}

fn action_name(a: &SchedAction) -> &'static str {
    match a {
        SchedAction::Wake => "wake",
        SchedAction::Reboot => "reboot",
        SchedAction::Poweroff => "poweroff",
    }
}

fn log_missed(state: &AppState, machines: &[Machine], missed: &[Due], at: u64) {
    let res = state.store.update(|st| {
        for d in missed {
            if !logged(st, d) {
                schedule::push_log(&mut st.schedule_log, entry(d, at, LogResult::Missed, None));
            }
        }
        Ok::<_, StoreError>(())
    });
    if let Err(e) = res {
        tracing::error!("scheduler: could not log missed occurrences: {e}");
        return;
    }
    for d in missed {
        let name = machines
            .iter()
            .find(|m| m.id == d.machine_id)
            .map_or(d.machine_id.as_str(), |m| m.name.as_str());
        actions::log(action_name(&d.action), name, &Source::Schedule, "missed");
    }
}

/// Write-ahead firing of one occurrence: persist `firing` (and disable a
/// one-off rule), run the action, then replace the entry with `ok`/`failed`.
async fn fire_one(state: &AppState, d: Due, at: u64) {
    // Re-checked under the store lock: the rule may have been disabled,
    // deleted or fired since the snapshot `select` saw.
    let started = state.store.update(|st| {
        let Some(m) = st.machines.iter().find(|m| m.id == d.machine_id).cloned() else {
            return Ok(None);
        };
        if logged(st, &d) {
            return Ok(None);
        }
        let Some(s) = st
            .schedules
            .iter_mut()
            .find(|s| s.id == d.schedule_id && s.enabled)
        else {
            return Ok(None);
        };
        if matches!(s.repeat, Repeat::Once { .. }) {
            s.enabled = false;
        }
        schedule::push_log(&mut st.schedule_log, entry(&d, at, LogResult::Firing, None));
        Ok::<_, StoreError>(Some(m))
    });
    let m = match started {
        Ok(Some(m)) => m,
        Ok(None) => return,
        Err(e) => {
            tracing::error!("scheduler: not firing {}: {e}", d.schedule_id);
            return;
        }
    };
    let src = Source::Schedule;
    let res = match d.action {
        SchedAction::Wake => actions::wake(state, &m, &src)
            .await
            .map(|o| (o == WakeOutcome::AlreadyOnline).then(|| "already online".to_string())),
        SchedAction::Poweroff => actions::power(state, &m, &src, "poweroff", Action::Shutdown)
            .await
            .map(|()| None),
        SchedAction::Reboot => actions::power(state, &m, &src, "reboot", Action::Reboot)
            .await
            .map(|()| None),
    };
    let done = match res {
        Ok(detail) => entry(&d, at, LogResult::Ok, detail),
        Err(ApiError(_, msg)) => entry(&d, at, LogResult::Failed, Some(msg)),
    };
    let res = state.store.update(|st| {
        let slot = st.schedule_log.iter_mut().rev().find(|e| {
            e.schedule_id == d.schedule_id && e.date == d.date && e.result == LogResult::Firing
        });
        match slot {
            Some(e) => *e = done,
            // The rule (and its log) went away while firing: nothing to record.
            None if !st.schedules.iter().any(|s| s.id == d.schedule_id) => {}
            // Pushed out of the ring buffer meanwhile.
            None => schedule::push_log(&mut st.schedule_log, done),
        }
        Ok::<_, StoreError>(())
    });
    if let Err(e) = res {
        tracing::error!("scheduler: could not log result of {}: {e}", d.schedule_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    #[test]
    fn startup_after_caps_lookback() {
        let now = ts("2026-11-14T09:00:00Z");
        let ten_days = secs(ts("2026-11-04T09:00:00Z"));
        assert_eq!(
            startup_after(Some(ten_days), now),
            ts("2026-11-07T09:00:00Z")
        );
        let yesterday = secs(ts("2026-11-13T06:00:00Z"));
        assert_eq!(
            startup_after(Some(yesterday), now),
            ts("2026-11-13T06:00:00Z")
        );
        assert_eq!(startup_after(None, now), ts("2026-11-14T08:58:00Z"));
    }
}
