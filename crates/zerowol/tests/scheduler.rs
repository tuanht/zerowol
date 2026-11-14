mod common;
use common::*;
use jiff::Timestamp;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use zerowol::app::AppState;
use zerowol::model::{
    AgentCfg, LogEntry, LogResult, Machine, Repeat, SchedAction, Schedule, new_secret,
};
use zerowol::scheduler::{startup_after, tick};
use zerowol::store::StoreError;

const HCM: &str = "Asia/Ho_Chi_Minh";
const DAY: &str = "2026-11-14";

fn local(s: &str) -> Timestamp {
    s.parse::<jiff::civil::DateTime>()
        .unwrap()
        .to_zoned(zerowol::schedule::zone(HCM).unwrap())
        .unwrap()
        .timestamp()
}

fn at(time: &str) -> Timestamp {
    local(&format!("{DAY}T{time}"))
}

fn edit(state: &AppState, f: impl FnOnce(&mut zerowol::model::State)) {
    state
        .store
        .update(|st| {
            f(st);
            Ok::<_, StoreError>(())
        })
        .unwrap();
}

/// Machine `id` on localhost; `agent` gives it an agent on that port.
fn add_machine(state: &AppState, id: &str, mac: &str, agent: Option<(u16, String)>, probe: u16) {
    edit(state, |st| {
        st.settings.timezone = Some(HCM.into());
        st.machines.push(Machine {
            id: id.into(),
            name: id.to_uppercase(),
            mac: mac.into(),
            ip: "127.0.0.1".parse().unwrap(),
            broadcast: "127.0.0.1".parse().unwrap(),
            wol_port: 9,
            probe_port: probe,
            agent: agent.map(|(port, secret)| AgentCfg { port, secret }),
        });
    });
}

/// Machine "m1" with a fake agent; returns the agent's seen-actions list.
async fn agent_machine(state: &AppState) -> Arc<Mutex<Vec<String>>> {
    let secret = new_secret();
    let (port, seen) = fake_agent(&secret, None).await;
    add_machine(state, "m1", "AA:BB:CC:DD:EE:01", Some((port, secret)), 1);
    seen
}

fn add_rule(state: &AppState, id: &str, mid: &str, action: SchedAction, repeat: Repeat) {
    edit(state, |st| {
        st.schedules.push(Schedule {
            id: id.into(),
            machine_id: mid.into(),
            action,
            time: "07:30".into(),
            repeat,
            enabled: true,
            skip: Vec::new(),
            created: 0,
        })
    });
}

fn log_of(state: &AppState) -> Vec<LogEntry> {
    state.store.snapshot().schedule_log
}

#[tokio::test]
async fn tick_fires_poweroff_and_logs_ok() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    assert_eq!(*seen.lock().unwrap(), ["shutdown"]);
    let log = log_of(&s);
    assert_eq!(log.len(), 1);
    assert_eq!(
        (log[0].schedule_id.as_str(), log[0].date.as_str()),
        ("r1", DAY)
    );
    assert_eq!(log[0].result, LogResult::Ok);
    assert_eq!(log[0].detail, None);
}

#[tokio::test]
async fn tick_reboot_sends_reboot() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Reboot, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    assert_eq!(*seen.lock().unwrap(), ["reboot"]);
    assert_eq!(log_of(&s)[0].result, LogResult::Ok);
}

#[tokio::test]
async fn tick_failure_logged() {
    let (_b, s, _d) = spawn(false).await;
    // A port nothing listens on: the agent is unreachable.
    let closed = {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    add_machine(
        &s,
        "m1",
        "AA:BB:CC:DD:EE:01",
        Some((closed, new_secret())),
        1,
    );
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    let log = log_of(&s);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].result, LogResult::Failed);
    assert_eq!(log[0].detail.as_deref(), Some("agent unreachable"));
}

#[tokio::test]
async fn tick_no_agent_failed() {
    let (_b, s, _d) = spawn(false).await;
    add_machine(&s, "m1", "AA:BB:CC:DD:EE:01", None, 1);
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    let log = log_of(&s);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].result, LogResult::Failed);
    assert_eq!(log[0].detail.as_deref(), Some("no agent"));
}

#[tokio::test]
async fn tick_once_disables_rule() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(
        &s,
        "r1",
        "m1",
        SchedAction::Poweroff,
        Repeat::Once { date: DAY.into() },
    );
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert!(!s.store.snapshot().schedules[0].enabled);
    assert_eq!(log_of(&s)[0].result, LogResult::Ok);
}

#[tokio::test]
async fn restart_after_fire_does_not_refire() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    s.store.touch_heartbeat(at("07:30:05").as_second() as u64);
    assert_eq!(seen.lock().unwrap().len(), 1);
    // Restart: a fresh loop starts from the persisted heartbeat.
    let now = at("07:30:40");
    let hb = s.store.snapshot().scheduler_heartbeat;
    tick(&s, startup_after(hb, now), now).await;
    // Even without a heartbeat (window reaches back 120 s) the log entry blocks it.
    tick(&s, startup_after(None, now), now).await;
    assert_eq!(seen.lock().unwrap().len(), 1);
    assert_eq!(log_of(&s).len(), 1);
}

#[tokio::test]
async fn crash_mid_fire_not_refired() {
    let (base, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    edit(&s, |st| {
        st.schedule_log.push(LogEntry {
            schedule_id: "r1".into(),
            date: DAY.into(),
            at: at("07:30:01").as_second() as u64,
            result: LogResult::Firing,
            detail: None,
        })
    });
    tick(&s, startup_after(None, at("07:30:30")), at("07:30:30")).await;
    assert!(seen.lock().unwrap().is_empty());
    assert_eq!(log_of(&s).len(), 1);
    let (c, v) = req(
        "GET",
        &format!("{base}/api/v1/schedules/occurrences?from={DAY}&to={DAY}"),
        &[],
        None,
    )
    .await;
    assert_eq!(c, 200);
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["outcome"]["result"], Value::from("firing"));
}

#[tokio::test]
async fn startup_marks_missed() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    let hb = local("2026-11-13T06:00:00").as_second() as u64;
    let now = at("09:00:00");
    tick(&s, startup_after(Some(hb), now), now).await;
    assert!(seen.lock().unwrap().is_empty());
    let log = log_of(&s);
    assert_eq!(
        log.iter()
            .map(|e| (e.date.as_str(), e.result.clone()))
            .collect::<Vec<_>>(),
        [("2026-11-13", LogResult::Missed), (DAY, LogResult::Missed)]
    );
}

#[tokio::test]
async fn missed_once_rule_is_disabled() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(
        &s,
        "r1",
        "m1",
        SchedAction::Poweroff,
        Repeat::Once { date: DAY.into() },
    );
    add_rule(&s, "r2", "m1", SchedAction::Poweroff, Repeat::Daily);
    let now = at("09:00:00");
    tick(
        &s,
        startup_after(Some(at("06:00:00").as_second() as u64), now),
        now,
    )
    .await;
    assert!(seen.lock().unwrap().is_empty());
    let st = s.store.snapshot();
    assert!(!st.schedules[0].enabled, "missed one-off stays enabled");
    assert!(st.schedules[1].enabled, "repeating rule stays enabled");
    assert!(
        st.schedule_log
            .iter()
            .all(|e| e.result == LogResult::Missed)
    );
    assert_eq!(st.schedule_log.len(), 2);
}

#[tokio::test]
async fn wake_already_online_is_ok() {
    let (_b, s, _d) = spawn(false).await;
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let probe = l.local_addr().unwrap().port();
    add_machine(&s, "m1", "AA:BB:CC:DD:EE:01", None, probe);
    add_rule(&s, "r1", "m1", SchedAction::Wake, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    let log = log_of(&s);
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].result, LogResult::Ok);
    assert_eq!(log[0].detail.as_deref(), Some("already online"));
}

#[tokio::test]
async fn wake_sends_packet_is_ok() {
    let (_b, s, _d) = spawn(false).await;
    let sock = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let wol = sock.local_addr().unwrap().port();
    add_machine(&s, "m1", "AA:BB:CC:DD:EE:01", None, 1);
    edit(&s, |st| st.machines[0].wol_port = wol);
    add_rule(&s, "r1", "m1", SchedAction::Wake, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    let mut buf = [0u8; 256];
    let n = tokio::time::timeout(std::time::Duration::from_secs(2), sock.recv(&mut buf))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(n, 102);
    let log = log_of(&s);
    assert_eq!((log.len(), log[0].result.clone()), (1, LogResult::Ok));
    assert_eq!(log[0].detail, None);
}

#[tokio::test]
async fn tick_without_timezone_does_nothing() {
    let (_b, s, _d) = spawn(false).await;
    let seen = agent_machine(&s).await;
    add_rule(&s, "r1", "m1", SchedAction::Poweroff, Repeat::Daily);
    edit(&s, |st| st.settings.timezone = None);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    assert!(seen.lock().unwrap().is_empty());
    assert!(log_of(&s).is_empty());
}

#[tokio::test]
async fn machines_fire_concurrently_in_time_order() {
    let (_b, s, _d) = spawn(false).await;
    let seen1 = agent_machine(&s).await;
    let secret = new_secret();
    let (port, seen2) = fake_agent(&secret, None).await;
    add_machine(&s, "m2", "AA:BB:CC:DD:EE:02", Some((port, secret)), 1);
    add_rule(&s, "a", "m1", SchedAction::Poweroff, Repeat::Daily);
    add_rule(&s, "b", "m2", SchedAction::Reboot, Repeat::Daily);
    tick(&s, at("07:29:50"), at("07:30:05")).await;
    assert_eq!(*seen1.lock().unwrap(), ["shutdown"]);
    assert_eq!(*seen2.lock().unwrap(), ["reboot"]);
    let log = log_of(&s);
    assert_eq!(log.len(), 2);
    assert!(log.iter().all(|e| e.result == LogResult::Ok));
}

#[test]
fn touch_heartbeat_sets_value() {
    let d = tempfile::TempDir::new().unwrap();
    let st = zerowol::store::Store::open(d.path()).unwrap();
    st.touch_heartbeat(42);
    assert_eq!(st.snapshot().scheduler_heartbeat, Some(42));
    st.flush().unwrap();
    let again = zerowol::store::Store::open(d.path()).unwrap();
    assert_eq!(again.snapshot().scheduler_heartbeat, Some(42));
}
