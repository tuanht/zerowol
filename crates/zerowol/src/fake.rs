//! Development-only fake machines with simulated status and power transitions.
//!
//! Compiled only with the `dev-fakes` cargo feature (never enabled for release,
//! `make dist` or Docker builds) and seeded only when `ZEROWOL_DEV_FAKES=1`.
//! Fake machines are ordinary records whose id starts with `dev-`; status
//! probes, Wake and agent power calls for them are answered by the simulator
//! instead of touching the network.

use crate::agent_client::AgentError;
use crate::model::{AgentCfg, Machine, new_secret};
use crate::status::Status;
use crate::store::{Store, StoreError};
use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};
use zerowol_proto::Action;

pub const PREFIX: &str = "dev-";
/// Schedule timezone the dev seed saves when none is set.
const SEED_TZ: &str = "Europe/Berlin";

#[derive(Clone, Copy, PartialEq, Debug)]
enum Agent {
    /// No agent configured: status comes from the (simulated) probe only.
    None,
    /// Agent answers health when the machine is on.
    Paired,
    /// Agent configured but never answers.
    Unreachable,
    /// Agent answers but rejects requests with this reason.
    Rejected(&'static str),
}

struct Spec {
    id: &'static str,
    name: &'static str,
    host: u8,
    agent: Agent,
    online: bool,
    /// Uptime at server start, seconds.
    uptime: u64,
    /// Seconds until a Wake brings it online; None = never comes up.
    wake_after: Option<u64>,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "dev-ws",
        name: "Workstation",
        host: 10,
        agent: Agent::Paired,
        online: true,
        uptime: 3 * 86400 + 4 * 3600,
        wake_after: Some(5),
    },
    Spec {
        id: "dev-media",
        name: "Media PC",
        host: 11,
        agent: Agent::None,
        online: false,
        uptime: 0,
        wake_after: Some(6),
    },
    Spec {
        id: "dev-laptop",
        name: "Laptop",
        host: 12,
        agent: Agent::Unreachable,
        online: true,
        uptime: 0,
        wake_after: Some(4),
    },
    Spec {
        id: "dev-gaming",
        name: "Gaming PC",
        host: 13,
        agent: Agent::Rejected("clock skew 45s"),
        online: true,
        uptime: 0,
        wake_after: Some(4),
    },
    Spec {
        id: "dev-old",
        name: "Old Server",
        host: 14,
        agent: Agent::None,
        online: false,
        uptime: 0,
        wake_after: None,
    },
];

#[derive(Clone, Copy, Debug)]
enum Step {
    Up,
    Down,
}

struct Sim {
    online: bool,
    /// Instant the machine (virtually) booted; uptime = now - booted.
    booted: Instant,
    /// Scheduled transitions, applied lazily when status is read.
    queue: Vec<(Instant, Step)>,
}

static SIM: LazyLock<Mutex<HashMap<&'static str, Sim>>> = LazyLock::new(|| {
    let now = Instant::now();
    Mutex::new(
        SPECS
            .iter()
            .map(|s| {
                let booted = now
                    .checked_sub(Duration::from_secs(s.uptime))
                    .unwrap_or(now);
                (
                    s.id,
                    Sim {
                        online: s.online,
                        booted,
                        queue: Vec::new(),
                    },
                )
            })
            .collect(),
    )
});

fn spec(m: &Machine) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.id == m.id)
}

pub fn is_fake(m: &Machine) -> bool {
    m.id.starts_with(PREFIX)
}

fn with_sim<R>(id: &'static str, f: impl FnOnce(&mut Sim) -> R) -> R {
    let mut map = SIM.lock().unwrap_or_else(|e| e.into_inner());
    let sim = map.get_mut(id).expect("sim seeded for every spec");
    let now = Instant::now();
    sim.queue.sort_by_key(|(t, _)| *t);
    while let Some(&(t, step)) = sim.queue.first() {
        if t > now {
            break;
        }
        sim.queue.remove(0);
        match step {
            Step::Up => {
                if !sim.online {
                    sim.booted = t;
                }
                sim.online = true;
            }
            Step::Down => sim.online = false,
        }
    }
    f(sim)
}

/// Seed any missing fake machines into the store. With no effective schedule
/// timezone (none saved, and no usable `TZ` env), also save one (Europe/Berlin),
/// so the Schedules tab and the UI regression cases start from a usable state
/// rather than the timezone prompt. A developer's own `TZ` is left in charge.
pub fn seed(store: &Store, tz_env: Option<&str>) -> Result<usize, StoreError> {
    store.update(|st| {
        if crate::schedule::effective_tz(&st.settings, tz_env).is_none() {
            st.settings.timezone = Some(SEED_TZ.into());
        }
        let mut added = 0;
        for s in SPECS {
            if st.machines.iter().any(|m| m.id == s.id) {
                continue;
            }
            let ip = Ipv4Addr::new(192, 0, 2, s.host); // TEST-NET-1, never routed
            st.machines.push(Machine {
                id: s.id.into(),
                name: s.name.into(),
                mac: format!("02:00:5E:00:00:{:02X}", s.host),
                ip,
                broadcast: Ipv4Addr::new(192, 0, 2, 255),
                wol_port: 9,
                probe_port: 22,
                agent: (s.agent != Agent::None).then(|| AgentCfg {
                    port: 9009,
                    secret: new_secret(),
                }),
            });
            added += 1;
        }
        Ok::<_, StoreError>(added)
    })
}

/// Simulated status, or None when `m` is not a fake machine.
pub fn status(m: &Machine) -> Option<Status> {
    let s = spec(m)?;
    Some(with_sim(s.id, |sim| {
        let plain = |online, agent| Status {
            online,
            agent,
            hostname: None,
            uptime: None,
            agent_error: None,
        };
        // The machine record may have been edited; trust its agent setting.
        match (m.agent.is_some(), s.agent) {
            (false, _) | (true, Agent::None) => plain(
                sim.online,
                if m.agent.is_some() {
                    "unreachable"
                } else {
                    "none"
                },
            ),
            (true, Agent::Paired) if sim.online => Status {
                online: true,
                agent: "paired",
                hostname: Some(s.name.to_ascii_lowercase().replace(' ', "-")),
                uptime: Some(sim.booted.elapsed().as_secs()),
                agent_error: None,
            },
            (true, Agent::Rejected(why)) if sim.online => Status {
                agent_error: Some(why.into()),
                ..plain(true, "unreachable")
            },
            (true, _) => plain(sim.online, "unreachable"),
        }
    }))
}

/// Simulated Wake: Some(()) when handled (the caller already checked "already online").
pub fn wake(m: &Machine) -> Option<()> {
    let s = spec(m)?;
    with_sim(s.id, |sim| {
        if let Some(secs) = s.wake_after {
            sim.queue
                .push((Instant::now() + Duration::from_secs(secs), Step::Up));
        }
    });
    Some(())
}

/// Simulated agent power call, or None when `m` is not a fake machine.
pub fn power(m: &Machine, action: Action) -> Option<Result<(), AgentError>> {
    let s = spec(m)?;
    Some(with_sim(s.id, |sim| match s.agent {
        Agent::Rejected(why) if sim.online => Err(AgentError::Rejected(why.into())),
        Agent::Paired if sim.online => {
            let now = Instant::now();
            match action {
                Action::Shutdown => sim.queue.push((now + Duration::from_secs(4), Step::Down)),
                Action::Reboot => {
                    sim.queue.push((now + Duration::from_secs(3), Step::Down));
                    sim.queue.push((now + Duration::from_secs(10), Step::Up));
                }
                Action::Health => {}
            }
            Ok(())
        }
        _ => Err(AgentError::Unreachable),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(id: &str, agent: bool) -> Machine {
        Machine {
            id: id.into(),
            name: "x".into(),
            mac: "02:00:5E:00:00:01".into(),
            ip: Ipv4Addr::new(192, 0, 2, 1),
            broadcast: Ipv4Addr::new(192, 0, 2, 255),
            wol_port: 9,
            probe_port: 22,
            agent: agent.then(|| AgentCfg {
                port: 9009,
                secret: new_secret(),
            }),
        }
    }

    #[test]
    fn real_machines_untouched() {
        let m = machine("abc123", true);
        assert!(status(&m).is_none());
        assert!(wake(&m).is_none());
        assert!(power(&m, Action::Shutdown).is_none());
    }

    #[test]
    fn scenarios() {
        let ws = status(&machine("dev-ws", true)).unwrap();
        assert_eq!((ws.online, ws.agent), (true, "paired"));
        assert!(ws.uptime.unwrap() >= 3 * 86400);
        let lap = status(&machine("dev-laptop", true)).unwrap();
        assert_eq!((lap.online, lap.agent), (true, "unreachable"));
        let g = status(&machine("dev-gaming", true)).unwrap();
        assert_eq!(g.agent_error.as_deref(), Some("clock skew 45s"));
        assert!(matches!(
            power(&machine("dev-gaming", true), Action::Reboot),
            Some(Err(AgentError::Rejected(_)))
        ));
        let media = status(&machine("dev-media", false)).unwrap();
        assert_eq!((media.online, media.agent), (false, "none"));
    }

    #[test]
    fn seed_sets_timezone_only_when_unset() {
        let d = tempfile::TempDir::new().unwrap();
        let store = Store::open(d.path()).unwrap();
        assert_eq!(seed(&store, None).unwrap(), SPECS.len());
        assert_eq!(store.snapshot().settings.timezone.as_deref(), Some(SEED_TZ));
        store
            .update(|st| {
                st.settings.timezone = Some("Asia/Tokyo".into());
                Ok::<_, StoreError>(())
            })
            .unwrap();
        assert_eq!(seed(&store, None).unwrap(), 0);
        assert_eq!(
            store.snapshot().settings.timezone.as_deref(),
            Some("Asia/Tokyo")
        );
    }

    #[test]
    fn seed_respects_tz_env() {
        let d = tempfile::TempDir::new().unwrap();
        let store = Store::open(d.path()).unwrap();
        // A real TZ is the effective timezone: nothing is saved.
        seed(&store, Some("America/New_York")).unwrap();
        assert_eq!(store.snapshot().settings.timezone, None);
        // UTC counts as no timezone, so the seed saves its own.
        seed(&store, Some("UTC")).unwrap();
        assert_eq!(store.snapshot().settings.timezone.as_deref(), Some(SEED_TZ));
    }

    #[test]
    fn transitions_apply_lazily() {
        with_sim("dev-old", |sim| {
            sim.queue
                .push((Instant::now() - Duration::from_secs(1), Step::Up))
        });
        assert!(status(&machine("dev-old", false)).unwrap().online);
        with_sim("dev-old", |sim| {
            sim.queue
                .push((Instant::now() - Duration::from_secs(1), Step::Down))
        });
        assert!(!status(&machine("dev-old", false)).unwrap().online);
    }
}
