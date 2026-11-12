use crate::agent_client::{self, AgentError};
use crate::model::Machine;
use crate::probe;
use serde::Serialize;
use std::time::Duration;
use tokio::task::JoinSet;

const PROBE_TIMEOUT: Duration = Duration::from_millis(1500);
const OVERALL_CAP: Duration = Duration::from_millis(1500);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Status {
    pub online: bool,
    pub agent: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
    /// Why the agent rejected our request (e.g. "clock skew 45s"); not set for
    /// a plain unreachable agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_error: Option<String>,
}

impl Status {
    fn plain(online: bool, agent: &'static str) -> Self {
        Status {
            online,
            agent,
            hostname: None,
            uptime: None,
            agent_error: None,
        }
    }
}

/// Agent health and TCP probe run concurrently, each capped at 1.5 s, so the
/// whole call finishes in ~1.5 s and a probe result is never discarded.
pub async fn machine_status(m: &Machine) -> Status {
    let probe_fut = async {
        tokio::time::timeout(
            OVERALL_CAP,
            probe::is_open(m.ip, m.probe_port, PROBE_TIMEOUT),
        )
        .await
        .unwrap_or(false)
    };
    match &m.agent {
        None => Status::plain(probe_fut.await, "none"),
        Some(a) => {
            let health_fut = async {
                tokio::time::timeout(OVERALL_CAP, agent_client::health(m.ip, a.port, &a.secret))
                    .await
            };
            let (h, up) = tokio::join!(health_fut, probe_fut);
            match h {
                Ok(Ok(h)) => Status {
                    online: true,
                    agent: "paired",
                    hostname: Some(h.hostname),
                    uptime: Some(h.uptime),
                    agent_error: None,
                },
                Ok(Err(e @ AgentError::Rejected(_))) => Status {
                    agent_error: Some(e.message()),
                    ..Status::plain(up, "unreachable")
                },
                _ => Status::plain(up, "unreachable"),
            }
        }
    }
}

pub async fn all_statuses(ms: &[Machine]) -> Vec<Status> {
    let mut set = JoinSet::new();
    for (i, m) in ms.iter().cloned().enumerate() {
        set.spawn(async move { (i, machine_status(&m).await) });
    }
    let mut out: Vec<Option<Status>> = vec![None; ms.len()];
    while let Some(r) = set.join_next().await {
        if let Ok((i, s)) = r {
            out[i] = Some(s);
        }
    }
    out.into_iter()
        .zip(ms)
        .map(|(s, m)| {
            s.unwrap_or_else(|| {
                Status::plain(
                    false,
                    if m.agent.is_some() {
                        "unreachable"
                    } else {
                        "none"
                    },
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_client::testutil::*;
    use crate::model::{AgentCfg, new_secret};
    use std::net::Ipv4Addr;
    use std::time::Instant;
    use tokio::net::TcpListener;

    fn machine(probe_port: u16, agent: Option<(u16, String)>) -> Machine {
        Machine {
            id: "t".into(),
            name: "t".into(),
            mac: "AA:BB:CC:DD:EE:FF".into(),
            ip: Ipv4Addr::LOCALHOST,
            broadcast: Ipv4Addr::new(127, 0, 0, 255),
            wol_port: 9,
            probe_port,
            agent: agent.map(|(port, secret)| AgentCfg { port, secret }),
        }
    }

    #[tokio::test]
    async fn status_paired() {
        let s = new_secret();
        let port = fake_agent(
            &s,
            "/v1/health",
            "200 OK",
            r#"{"hostname":"pc","version":"0.1.0","uptime":7}"#,
        )
        .await;
        let st = machine_status(&machine(closed_port().await, Some((port, s)))).await;
        assert_eq!(
            st,
            Status {
                online: true,
                agent: "paired",
                hostname: Some("pc".into()),
                uptime: Some(7),
                agent_error: None
            }
        );
    }

    #[tokio::test]
    async fn status_agent_down_probe_up() {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let probe_port = l.local_addr().unwrap().port();
        let st = machine_status(&machine(
            probe_port,
            Some((closed_port().await, new_secret())),
        ))
        .await;
        assert_eq!(st, Status::plain(true, "unreachable"));
    }

    #[tokio::test]
    async fn status_agent_silent_probe_up() {
        let l = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let probe_port = l.local_addr().unwrap().port();
        let m = machine(probe_port, Some((silent_agent().await, new_secret())));
        let t = Instant::now();
        let st = machine_status(&m).await;
        assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
        assert_eq!(st, Status::plain(true, "unreachable"));
    }

    #[tokio::test]
    async fn status_agent_rejected_carries_reason() {
        let s = new_secret();
        let port = fake_agent(
            &s,
            "/v1/health",
            "401 Unauthorized",
            r#"{"error":"clock skew 45s"}"#,
        )
        .await;
        let st = machine_status(&machine(closed_port().await, Some((port, s)))).await;
        assert_eq!(st.agent, "unreachable");
        assert_eq!(st.agent_error.as_deref(), Some("clock skew 45s"));
        assert!(!st.online);
        let j = serde_json::to_value(&st).unwrap();
        assert_eq!(j["agent_error"], "clock skew 45s");
    }

    #[tokio::test]
    async fn status_none() {
        let st = machine_status(&machine(closed_port().await, None)).await;
        assert_eq!(st, Status::plain(false, "none"));
    }

    #[tokio::test]
    async fn status_serializes_without_none_fields() {
        let j = serde_json::to_string(&Status::plain(false, "none")).unwrap();
        assert_eq!(j, r#"{"online":false,"agent":"none"}"#);
    }

    #[tokio::test]
    async fn all_statuses_concurrent() {
        let mut ms = Vec::new();
        for _ in 0..3 {
            ms.push(machine(
                closed_port().await,
                Some((silent_agent().await, new_secret())),
            ));
        }
        let t = Instant::now();
        let out = all_statuses(&ms).await;
        assert!(
            t.elapsed() < Duration::from_millis(2500),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(out.len(), 3);
        assert!(
            out.iter()
                .all(|s| *s == Status::plain(false, "unreachable"))
        );
    }
}
