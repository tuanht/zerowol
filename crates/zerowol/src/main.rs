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
    if let Err(e) = axum::serve(listener, router(state)).await {
        tracing::error!("server error: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
