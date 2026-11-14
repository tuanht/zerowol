mod common;
use common::*;
use serde_json::{Value, json};
use zerowol::store::StoreError;

const HCM: &str = "Asia/Ho_Chi_Minh";

async fn call(method: &str, base: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let b = body.map(|v| v.to_string());
    req(method, &format!("{base}/api/v1{path}"), &[], b.as_deref()).await
}

async fn set_tz(base: &str, tz: &str) {
    let (c, _) = call("PATCH", base, "/settings", Some(json!({"timezone": tz}))).await;
    assert_eq!(c, 200);
}

async fn machine(base: &str, mac: &str, agent: bool) -> String {
    let (c, m) = call(
        "POST",
        base,
        "/machines",
        Some(json!({"name":"PC","mac":mac,"ip":"127.0.0.1","agent":agent})),
    )
    .await;
    assert_eq!(c, 201);
    m["id"].as_str().unwrap().to_string()
}

async fn sched(base: &str, mid: &str, action: &str, time: &str, repeat: Value) -> Value {
    let (c, s) = call(
        "POST",
        base,
        "/schedules",
        Some(json!({"machine_id":mid,"action":action,"time":time,"repeat":repeat})),
    )
    .await;
    assert_eq!(c, 201, "{s}");
    s
}

fn today() -> jiff::civil::Date {
    jiff::Zoned::now()
        .with_time_zone(zerowol::schedule::zone(HCM).unwrap())
        .date()
}

fn err(v: &Value) -> &str {
    v["error"].as_str().unwrap_or("")
}

#[tokio::test]
async fn settings_roundtrip() {
    let (base, _s, _d) = spawn(false).await;
    let (c, v) = call("GET", &base, "/settings", None).await;
    assert_eq!((c, v), (200, json!({"timezone":null,"source":null})));
    let (c, v) = call(
        "PATCH",
        &base,
        "/settings",
        Some(json!({"timezone":"Europe/Berlin"})),
    )
    .await;
    assert_eq!(
        (c, v),
        (200, json!({"timezone":"Europe/Berlin","source":"settings"}))
    );
    let (c, v) = call(
        "PATCH",
        &base,
        "/settings",
        Some(json!({"timezone":"Mars/Base"})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "invalid timezone"));
    let (c, v) = call("PATCH", &base, "/settings", Some(json!({"timezone":null}))).await;
    assert_eq!((c, v), (200, json!({"timezone":null,"source":null})));
}

#[tokio::test]
async fn create_requires_timezone() {
    let (base, _s, _d) = spawn(false).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let (c, v) = call(
        "POST",
        &base,
        "/schedules",
        Some(json!({"machine_id":mid,"action":"wake","time":"08:00","repeat":{"kind":"daily"}})),
    )
    .await;
    assert_eq!((c, err(&v)), (409, "timezone not set"));
}

#[tokio::test]
async fn create_list_patch_delete() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let s = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    assert_eq!(s["enabled"], true);
    assert_eq!(s["machine_id"], mid.as_str());
    let id = s["id"].as_str().unwrap();
    let (_, l) = call("GET", &base, "/schedules", None).await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    let (c, p) = call(
        "PATCH",
        &base,
        &format!("/schedules/{id}"),
        Some(json!({"time":"09:15"})),
    )
    .await;
    assert_eq!((c, p["time"].clone()), (200, json!("09:15")));
    let (c, _) = call("DELETE", &base, &format!("/schedules/{id}"), None).await;
    assert_eq!(c, 204);
    let (_, l) = call("GET", &base, "/schedules", None).await;
    assert!(l.as_array().unwrap().is_empty());
    let (c, v) = call("DELETE", &base, &format!("/schedules/{id}"), None).await;
    assert_eq!((c, err(&v)), (404, "not found"));
}

#[tokio::test]
async fn validation_messages() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let plain = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let post = |b: Value| {
        let base = base.clone();
        async move {
            let (c, v) = call("POST", &base, "/schedules", Some(b)).await;
            (c, err(&v).to_string())
        }
    };
    let ok =
        |m: &str| json!({"machine_id":m,"action":"wake","time":"08:00","repeat":{"kind":"daily"}});
    let with = |k: &str, v: Value| {
        let mut b = ok(&plain);
        b[k] = v;
        b
    };
    assert_eq!(post(ok("nope")).await, (400, "invalid machine".to_string()));
    assert_eq!(
        post(with("action", json!("explode"))).await,
        (400, "invalid action".to_string())
    );
    assert_eq!(
        post(with("action", json!("poweroff"))).await,
        (400, "invalid action: needs agent".to_string())
    );
    assert_eq!(
        post(with("action", json!("reboot"))).await,
        (400, "invalid action: needs agent".to_string())
    );
    assert_eq!(
        post(with("time", json!("8:00"))).await,
        (400, "invalid time".to_string())
    );
    assert_eq!(
        post(with("repeat", json!({"kind":"once","date":"2099-02-30"}))).await,
        (400, "invalid date".to_string())
    );
    assert_eq!(
        post(with("repeat", json!({"kind":"weekly","days":[]}))).await,
        (400, "invalid days".to_string())
    );
    assert_eq!(
        post(with("repeat", json!({"kind":"weekly","days":["xyz"]}))).await,
        (400, "invalid days".to_string())
    );
    assert_eq!(
        post(with("repeat", json!({"kind":"hourly"}))).await,
        (400, "invalid repeat".to_string())
    );
    // once in the future is fine
    let (c, _) = call(
        "POST",
        &base,
        "/schedules",
        Some(with("repeat", json!({"kind":"once","date":"2099-01-01"}))),
    )
    .await;
    assert_eq!(c, 201);
    // unknown id
    let (c, v) = call("PATCH", &base, "/schedules/zzzzzz", Some(json!({}))).await;
    assert_eq!((c, err(&v)), (404, "not found"));
    // PATCH re-validates the merged result
    let s = sched(&base, &plain, "wake", "08:00", json!({"kind":"daily"})).await;
    let id = s["id"].as_str().unwrap();
    let (c, v) = call(
        "PATCH",
        &base,
        &format!("/schedules/{id}"),
        Some(json!({"action":"poweroff"})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "invalid action: needs agent"));
    let yday = today().yesterday().unwrap().to_string();
    let (c, v) = call(
        "PATCH",
        &base,
        &format!("/schedules/{id}"),
        Some(json!({"repeat":{"kind":"once","date":yday}})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "time already passed"));
}

#[tokio::test]
async fn once_in_past_rejected() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let body = |date: String, time: String| json!({"machine_id":mid,"action":"wake","time":time,"repeat":{"kind":"once","date":date}});
    let now = jiff::Zoned::now().with_time_zone(zerowol::schedule::zone(HCM).unwrap());
    let yday = now.date().yesterday().unwrap().to_string();
    let (c, v) = call(
        "POST",
        &base,
        "/schedules",
        Some(body(yday, "23:59".into())),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "time already passed"));
    if now.hour() == 0 && now.minute() == 0 {
        return;
    }
    let prev = now.checked_sub(jiff::SignedDuration::from_mins(1)).unwrap();
    let t = format!("{:02}:{:02}", prev.hour(), prev.minute());
    let (c, v) = call(
        "POST",
        &base,
        "/schedules",
        Some(body(prev.date().to_string(), t)),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "time already passed"));
}

#[tokio::test]
async fn occurrences_shape() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let s = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    let from = today();
    let to = from.checked_add(jiff::Span::new().days(6)).unwrap();
    let (c, o) = call(
        "GET",
        &base,
        &format!("/schedules/occurrences?from={from}&to={to}"),
        None,
    )
    .await;
    assert_eq!(c, 200);
    let a = o.as_array().unwrap();
    assert_eq!(a.len(), 7);
    let f = &a[0];
    assert_eq!(f["schedule_id"], s["id"]);
    assert_eq!(f["machine_id"], mid.as_str());
    assert_eq!(f["machine_name"], "PC");
    assert_eq!(f["action"], "wake");
    assert_eq!(f["repeat_kind"], "daily");
    assert_eq!(f["date"], from.to_string());
    assert_eq!(f["time"], "08:00");
    assert_eq!(f["skipped"], false);
    assert_eq!(f["enabled"], true);
    assert_eq!(f["outcome"], Value::Null);
    let expect = zerowol::schedule::instant(
        &zerowol::schedule::zone(HCM).unwrap(),
        from,
        zerowol::schedule::parse_time("08:00").unwrap(),
    )
    .as_second();
    assert_eq!(f["at"].as_i64().unwrap(), expect);
    let ats: Vec<i64> = a.iter().map(|x| x["at"].as_i64().unwrap()).collect();
    assert!(ats.windows(2).all(|w| w[0] < w[1]));
    let far = from.checked_add(jiff::Span::new().days(63)).unwrap();
    for q in [
        format!("from={from}&to={far}"),
        format!("from={to}&to={from}"),
        "from=bad&to=2026-01-01".to_string(),
        format!("from={from}"),
    ] {
        let (c, v) = call("GET", &base, &format!("/schedules/occurrences?{q}"), None).await;
        assert_eq!((c, err(&v)), (400, "invalid range"), "{q}");
    }
    // outcome comes from the latest log entry, disabled rules still listed
    let id = s["id"].as_str().unwrap().to_string();
    _s.store
        .update(|st| {
            for (at, r) in [(10, "firing"), (20, "ok")] {
                st.schedule_log.push(zerowol::model::LogEntry {
                    schedule_id: id.clone(),
                    date: from.to_string(),
                    at,
                    result: serde_json::from_value(json!(r)).unwrap(),
                    detail: Some("d".into()),
                });
            }
            Ok::<_, StoreError>(())
        })
        .unwrap();
    call(
        "PATCH",
        &base,
        &format!("/schedules/{}", s["id"].as_str().unwrap()),
        Some(json!({"enabled":false})),
    )
    .await;
    let (_, o) = call(
        "GET",
        &base,
        &format!("/schedules/occurrences?from={from}&to={from}"),
        None,
    )
    .await;
    assert_eq!(o[0]["enabled"], false);
    assert_eq!(o[0]["outcome"], json!({"result":"ok","detail":"d","at":20}));
}

#[tokio::test]
async fn occurrences_without_timezone_empty() {
    let (base, _s, _d) = spawn(false).await;
    let (c, o) = call(
        "GET",
        &base,
        "/schedules/occurrences?from=2026-01-01&to=2026-01-07",
        None,
    )
    .await;
    assert_eq!((c, o), (200, json!([])));
}

#[tokio::test]
async fn skip_and_unskip() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let s = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    let id = s["id"].as_str().unwrap();
    let tom = today().tomorrow().unwrap().to_string();
    let (c, v) = call(
        "POST",
        &base,
        &format!("/schedules/{id}/skip"),
        Some(json!({"date":tom})),
    )
    .await;
    assert_eq!((c, v["skip"].clone()), (200, json!([tom])));
    let occ = |base: String, d: String| async move {
        let (_, o) = call(
            "GET",
            &base,
            &format!("/schedules/occurrences?from={d}&to={d}"),
            None,
        )
        .await;
        o[0]["skipped"].clone()
    };
    assert_eq!(occ(base.clone(), tom.clone()).await, json!(true));
    let (c, v) = call(
        "DELETE",
        &base,
        &format!("/schedules/{id}/skip/{tom}"),
        None,
    )
    .await;
    assert_eq!((c, v["skip"].clone()), (200, json!([])));
    assert_eq!(occ(base.clone(), tom.clone()).await, json!(false));
    // not an occurrence
    let wk = sched(&base, &mid, "wake", "08:00", json!({"kind":"weekdays"})).await;
    let mut d = today().tomorrow().unwrap();
    while d.weekday() != jiff::civil::Weekday::Saturday {
        d = d.tomorrow().unwrap();
    }
    let (c, v) = call(
        "POST",
        &base,
        &format!("/schedules/{}/skip", wk["id"].as_str().unwrap()),
        Some(json!({"date":d.to_string()})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "invalid date"));
    let (c, v) = call(
        "POST",
        &base,
        &format!("/schedules/{id}/skip"),
        Some(json!({"date":"nope"})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "invalid date"));
    let (c, _) = call(
        "POST",
        &base,
        "/schedules/zzzzzz/skip",
        Some(json!({"date":tom})),
    )
    .await;
    assert_eq!(c, 404);
}

#[tokio::test]
async fn skip_prunes_past_dates() {
    let (base, st, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let s = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    let id = s["id"].as_str().unwrap().to_string();
    st.store
        .update(|x| {
            x.schedules[0].skip = vec!["2020-01-01".into()];
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let tom = today().tomorrow().unwrap().to_string();
    let (_, v) = call(
        "POST",
        &base,
        &format!("/schedules/{id}/skip"),
        Some(json!({"date":tom})),
    )
    .await;
    assert_eq!(v["skip"], json!([tom]));
}

#[tokio::test]
async fn machine_list_counts_enabled_schedules() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let other = machine(&base, "AA:BB:CC:DD:EE:02", false).await;
    sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    sched(&base, &mid, "wake", "09:00", json!({"kind":"daily"})).await;
    let off = sched(&base, &mid, "wake", "10:00", json!({"kind":"daily"})).await;
    call(
        "PATCH",
        &base,
        &format!("/schedules/{}", off["id"].as_str().unwrap()),
        Some(json!({"enabled":false})),
    )
    .await;
    let (_, l) = call("GET", &base, "/machines", None).await;
    for m in l.as_array().unwrap() {
        let want = if m["id"] == mid.as_str() { 2 } else { 0 };
        assert_eq!(m["schedules"], want);
    }
    let (_, p) = call(
        "PATCH",
        &base,
        &format!("/machines/{other}"),
        Some(json!({"name":"X"})),
    )
    .await;
    assert_eq!(p["schedules"], 0);
    let (_, p) = call(
        "PATCH",
        &base,
        &format!("/machines/{mid}"),
        Some(json!({"name":"X"})),
    )
    .await;
    assert_eq!(p["schedules"], 2);
}

#[tokio::test]
async fn agent_removal_disables_power_schedules() {
    let (base, _s, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", true).await;
    let off = sched(&base, &mid, "poweroff", "23:00", json!({"kind":"daily"})).await;
    let wake = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    let (c, _) = call(
        "PATCH",
        &base,
        &format!("/machines/{mid}"),
        Some(json!({"agent":false})),
    )
    .await;
    assert_eq!(c, 200);
    let (_, l) = call("GET", &base, "/schedules", None).await;
    for s in l.as_array().unwrap() {
        let want = s["id"] == wake["id"];
        assert_eq!(s["enabled"], want, "{s}");
    }
    let (c, v) = call(
        "PATCH",
        &base,
        &format!("/schedules/{}", off["id"].as_str().unwrap()),
        Some(json!({"enabled":true})),
    )
    .await;
    assert_eq!((c, err(&v)), (400, "invalid action: needs agent"));
    // disabling is always allowed
    let (c, _) = call(
        "PATCH",
        &base,
        &format!("/schedules/{}", off["id"].as_str().unwrap()),
        Some(json!({"enabled":false})),
    )
    .await;
    assert_eq!(c, 200);
}

#[tokio::test]
async fn machine_delete_cascades() {
    let (base, st, _d) = spawn(false).await;
    set_tz(&base, HCM).await;
    let mid = machine(&base, "AA:BB:CC:DD:EE:01", false).await;
    let keep = machine(&base, "AA:BB:CC:DD:EE:02", false).await;
    let s = sched(&base, &mid, "wake", "08:00", json!({"kind":"daily"})).await;
    let k = sched(&base, &keep, "wake", "08:00", json!({"kind":"daily"})).await;
    let id = s["id"].as_str().unwrap().to_string();
    let kid = k["id"].as_str().unwrap().to_string();
    st.store
        .update(|x| {
            for sid in [&id, &kid] {
                x.schedule_log.push(zerowol::model::LogEntry {
                    schedule_id: sid.clone(),
                    date: "2026-01-01".into(),
                    at: 1,
                    result: zerowol::model::LogResult::Ok,
                    detail: None,
                });
            }
            Ok::<_, StoreError>(())
        })
        .unwrap();
    let (c, _) = call("DELETE", &base, &format!("/machines/{mid}"), None).await;
    assert_eq!(c, 204);
    let (_, l) = call("GET", &base, "/schedules", None).await;
    assert_eq!(l.as_array().unwrap().len(), 1);
    assert_eq!(l[0]["id"], kid.as_str());
    let snap = st.store.snapshot();
    assert_eq!(snap.schedule_log.len(), 1);
    assert_eq!(snap.schedule_log[0].schedule_id, kid);
    let from = today();
    let to = from.checked_add(jiff::Span::new().days(3)).unwrap();
    let (_, o) = call(
        "GET",
        &base,
        &format!("/schedules/occurrences?from={from}&to={to}"),
        None,
    )
    .await;
    assert!(
        o.as_array()
            .unwrap()
            .iter()
            .all(|x| x["schedule_id"] == kid.as_str())
    );
    let (c, v) = call(
        "PATCH",
        &base,
        &format!("/schedules/{id}"),
        Some(json!({"time":"09:00"})),
    )
    .await;
    assert_eq!((c, err(&v)), (404, "not found"));
}

#[tokio::test]
async fn require_token_applies() {
    let (base, _s, _d) = spawn(true).await;
    let (c, _) = req(
        "GET",
        &format!("{base}/api/v1/schedules"),
        &[("Origin", "http://evil.example")],
        None,
    )
    .await;
    assert_eq!(c, 401);
    let (c, _) = req("GET", &format!("{base}/api/v1/settings"), &[], None).await;
    assert_eq!(c, 401);
}
