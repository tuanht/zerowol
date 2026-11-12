use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use zerowol::app::{AppState, router};
use zerowol::config::{self, Config};
use zerowol::model::now;
use zerowol::store::Store;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    let cfg = match config::from_env() {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("config: {e}");
            return ExitCode::FAILURE;
        }
    };
    match std::env::args().nth(1).as_deref() {
        None | Some("serve") => serve(cfg).await,
        Some("healthcheck") => healthcheck(&cfg).await,
        Some(other) => {
            eprintln!("unknown command: {other}");
            ExitCode::FAILURE
        }
    }
}

async fn healthcheck(cfg: &Config) -> ExitCode {
    let io = async {
        let mut s = TcpStream::connect(("127.0.0.1", cfg.port)).await?;
        s.write_all(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .await?;
        let mut buf = Vec::new();
        s.read_to_end(&mut buf).await?;
        Ok::<_, std::io::Error>(buf)
    };
    match tokio::time::timeout(Duration::from_secs(3), io).await {
        Ok(Ok(buf)) if buf.starts_with(b"HTTP/1.1 200") => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

async fn serve(cfg: Config) -> ExitCode {
    let store = match Store::open(&cfg.data) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            tracing::error!("{e}");
            return ExitCode::FAILURE;
        }
    };
    seed_fakes(&store);
    let flusher = store.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = flusher.flush_if_due(now()) {
                tracing::error!("flush failed: {e}");
            }
        }
    });
    let addr = (cfg.bind, cfg.port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("cannot bind {}:{}: {e}", cfg.bind, cfg.port);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!("listening on {}:{}", cfg.bind, cfg.port);
    let state = AppState {
        store,
        cfg: Arc::new(cfg),
        limiter: Arc::new(Mutex::new(Default::default())),
    };
    let final_store = state.store.clone();
    let served = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await;
    if let Err(e) = final_store.flush() {
        tracing::error!("final flush failed: {e}");
    }
    if let Err(e) = served {
        tracing::error!("server error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Development only: seed simulated machines when built with `dev-fakes` and
/// ZEROWOL_DEV_FAKES=1. Without the feature the variable is ignored (with a warning).
fn seed_fakes(store: &Store) {
    let wanted = matches!(
        std::env::var("ZEROWOL_DEV_FAKES").as_deref().map(str::trim),
        Ok("1" | "true")
    );
    if !wanted {
        return;
    }
    #[cfg(feature = "dev-fakes")]
    match zerowol::fake::seed(store) {
        Ok(n) => tracing::warn!("DEV: fake machines enabled ({n} seeded); ids start with 'dev-'"),
        Err(e) => tracing::error!("DEV: seeding fake machines failed: {e}"),
    }
    #[cfg(not(feature = "dev-fakes"))]
    {
        let _ = store;
        tracing::warn!("ZEROWOL_DEV_FAKES ignored: built without the dev-fakes feature");
    }
}

async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    tokio::select! {
        _ = term.recv() => {}
        _ = tokio::signal::ctrl_c() => {}
    }
    tracing::info!("shutting down");
}
