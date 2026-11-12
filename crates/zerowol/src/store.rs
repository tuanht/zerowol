use crate::model::State;
use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

const FLUSH_INTERVAL: u64 = 60;

#[derive(Debug)]
pub enum StoreError {
    Corrupt(String),
    Save(String),
    Io(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Corrupt(m) => write!(f, "state file corrupt: {m}"),
            StoreError::Save(m) => write!(f, "could not save: {m}"),
            StoreError::Io(m) => write!(f, "io error: {m}"),
        }
    }
}

impl std::error::Error for StoreError {}

pub struct Store {
    dir: PathBuf,
    state: RwLock<State>,
    /// (dirty, last_flush)
    meta: Mutex<(bool, u64)>,
}

fn persist(dir: &Path, st: &State) -> Result<(), String> {
    let path = dir.join("state.json");
    let tmp = dir.join("state.json.tmp");
    let bak = dir.join("state.json.bak");
    let json = serde_json::to_vec_pretty(st).map_err(|e| e.to_string())?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)
        .map_err(|e| e.to_string())?;
    f.write_all(&json).map_err(|e| e.to_string())?;
    f.sync_all().map_err(|e| e.to_string())?;
    drop(f);
    if path.exists() {
        fs::remove_file(&bak).ok();
        let mut src = File::open(&path).map_err(|e| e.to_string())?;
        let mut dst = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&bak)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut src, &mut dst).map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

impl Store {
    pub fn open(dir: &Path) -> Result<Store, StoreError> {
        let path = dir.join("state.json");
        let state = match fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str::<State>(&s)
                .map_err(|e| StoreError::Corrupt(format!("{}: {e}", path.display())))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let st = State::default();
                persist(dir, &st).map_err(StoreError::Io)?;
                st
            }
            Err(e) => return Err(StoreError::Io(format!("{}: {e}", path.display()))),
        };
        Ok(Store {
            dir: dir.to_path_buf(),
            state: RwLock::new(state),
            meta: Mutex::new((false, 0)),
        })
    }

    pub fn snapshot(&self) -> State {
        self.state.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn update<R, E: From<StoreError>>(
        &self,
        f: impl FnOnce(&mut State) -> Result<R, E>,
    ) -> Result<R, E> {
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        let mut next = guard.clone();
        let out = f(&mut next)?;
        persist(&self.dir, &next).map_err(StoreError::Save)?;
        *guard = next;
        self.meta.lock().unwrap_or_else(|e| e.into_inner()).0 = false;
        Ok(out)
    }

    pub fn touch_token(&self, id: &str, at: u64) {
        let mut guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        if let Some(t) = guard.tokens.iter_mut().find(|t| t.id == id) {
            t.last_used = Some(at);
            self.meta.lock().unwrap_or_else(|e| e.into_inner()).0 = true;
        }
    }

    pub fn flush_if_due(&self, now: u64) -> Result<(), StoreError> {
        self.flush_inner(now, false)
    }

    /// Writes pending `last_used` changes now, regardless of the interval.
    pub fn flush(&self) -> Result<(), StoreError> {
        self.flush_inner(crate::model::now(), true)
    }

    fn flush_inner(&self, now: u64, force: bool) -> Result<(), StoreError> {
        let guard = self.state.write().unwrap_or_else(|e| e.into_inner());
        let mut meta = self.meta.lock().unwrap_or_else(|e| e.into_inner());
        if !meta.0 || (!force && now < meta.1.saturating_add(FLUSH_INTERVAL)) {
            return Ok(());
        }
        persist(&self.dir, &guard).map_err(StoreError::Save)?;
        *meta = (false, now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Machine, Token};
    use std::net::Ipv4Addr;
    use tempfile::TempDir;

    fn machine(id: &str) -> Machine {
        Machine {
            id: id.into(),
            name: id.into(),
            mac: "AA:BB:CC:DD:EE:FF".into(),
            ip: Ipv4Addr::new(10, 0, 0, 2),
            broadcast: Ipv4Addr::new(10, 0, 0, 255),
            wol_port: 9,
            probe_port: 22,
            agent: None,
        }
    }

    fn add(s: &Store, id: &str) -> Result<(), StoreError> {
        s.update(|st| {
            st.machines.push(machine(id));
            Ok::<_, StoreError>(())
        })
    }

    #[test]
    fn open_creates_empty() {
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        assert!(d.path().join("state.json").exists());
        assert_eq!(s.snapshot().machines.len(), 0);
    }

    #[test]
    fn update_persists() {
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        add(&s, "a").unwrap();
        let s2 = Store::open(d.path()).unwrap();
        assert_eq!(s2.snapshot().machines.len(), 1);
    }

    #[test]
    fn bak_written() {
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        add(&s, "a").unwrap();
        add(&s, "b").unwrap();
        let bak = std::fs::read_to_string(d.path().join("state.json.bak")).unwrap();
        let st: State = serde_json::from_str(&bak).unwrap();
        assert_eq!(st.machines.len(), 1);
        assert_eq!(s.snapshot().machines.len(), 2);
    }

    #[test]
    fn corrupt_refused() {
        let d = TempDir::new().unwrap();
        std::fs::write(d.path().join("state.json"), "{nope").unwrap();
        assert!(matches!(Store::open(d.path()), Err(StoreError::Corrupt(_))));
        assert_eq!(
            std::fs::read_to_string(d.path().join("state.json")).unwrap(),
            "{nope"
        );
    }

    #[test]
    fn save_failure_rolls_back() {
        use std::os::unix::fs::PermissionsExt;
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::File::create(d.path().join("probe")).is_ok() {
            eprintln!("running as root; skipping");
            std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        let r = add(&s, "a");
        std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(r, Err(StoreError::Save(_))));
        assert_eq!(s.snapshot().machines.len(), 0);
    }

    #[test]
    fn missing_optional_fields_default() {
        let d = TempDir::new().unwrap();
        let j = r#"{"version":1,"machines":[{"id":"a","name":"n","mac":"AA:BB:CC:DD:EE:FF","ip":"10.0.0.2","broadcast":"10.0.0.255"}],"tokens":[{"id":"t","name":"x","hash":"h","created":5}]}"#;
        std::fs::write(d.path().join("state.json"), j).unwrap();
        let st = Store::open(d.path()).unwrap().snapshot();
        assert_eq!(st.machines[0].wol_port, 9);
        assert_eq!(st.machines[0].probe_port, 22);
        assert!(st.machines[0].agent.is_none());
        assert!(st.tokens[0].last_used.is_none());
    }

    fn on_disk_last_used(d: &TempDir) -> Option<u64> {
        let st: State =
            serde_json::from_str(&std::fs::read_to_string(d.path().join("state.json")).unwrap())
                .unwrap();
        st.tokens[0].last_used
    }

    #[test]
    fn touch_flush() {
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        s.update(|st| {
            st.tokens.push(Token {
                id: "t".into(),
                name: "x".into(),
                hash: "h".into(),
                created: 1,
                last_used: None,
            });
            Ok::<_, StoreError>(())
        })
        .unwrap();
        s.touch_token("t", 1000);
        assert_eq!(s.snapshot().tokens[0].last_used, Some(1000));
        s.flush_if_due(1000).unwrap();
        assert_eq!(on_disk_last_used(&d), Some(1000));
        s.touch_token("t", 1030);
        s.flush_if_due(1030).unwrap();
        assert_eq!(on_disk_last_used(&d), Some(1000));
        s.flush_if_due(1060).unwrap();
        assert_eq!(on_disk_last_used(&d), Some(1030));
    }

    #[test]
    fn flush_writes_immediately() {
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        s.update(|st| {
            st.tokens.push(Token {
                id: "t".into(),
                name: "x".into(),
                hash: "h".into(),
                created: 1,
                last_used: None,
            });
            Ok::<_, StoreError>(())
        })
        .unwrap();
        s.touch_token("t", 1000);
        s.flush_if_due(1000).unwrap();
        s.touch_token("t", 1010);
        s.flush_if_due(1010).unwrap();
        assert_eq!(on_disk_last_used(&d), Some(1000));
        s.flush().unwrap();
        assert_eq!(on_disk_last_used(&d), Some(1010));
    }

    #[test]
    fn state_files_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let d = TempDir::new().unwrap();
        let s = Store::open(d.path()).unwrap();
        add(&s, "a").unwrap();
        add(&s, "b").unwrap();
        for f in ["state.json", "state.json.bak"] {
            let m = std::fs::metadata(d.path().join(f)).unwrap();
            assert_eq!(m.permissions().mode() & 0o777, 0o600, "{f}");
        }
    }

    #[test]
    fn concurrent_updates_not_lost() {
        let d = TempDir::new().unwrap();
        let s = std::sync::Arc::new(Store::open(d.path()).unwrap());
        let hs: Vec<_> = (0..8)
            .map(|i| {
                let s = s.clone();
                std::thread::spawn(move || add(&s, &format!("m{i}")).unwrap())
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        assert_eq!(s.snapshot().machines.len(), 8);
    }
}
