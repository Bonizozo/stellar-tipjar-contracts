//! Standalone event-monitor binary.
//!
//! Reads `config/events.toml` (or env-var overrides) and starts the full
//! event-monitoring stack:
//!   - EventListener  – polls Horizon/RPC, retries with exponential back-off
//!   - EventProcessor – parses and persists events to PostgreSQL
//!   - SSE stream     – broadcasts indexed events to connected clients
//!   - HTTP API       – GET /api/events, GET /api/events/stream, POST /api/events/replay
//!
//! All heavy lifting is in the `tipjar-indexer` crate; this binary only wires
//! configuration and starts the service.
//!
//! ## Usage
//!
//! ```bash
//! CONTRACT_ID=C... DATABASE_URL=postgres://... cargo run --bin event-monitor
//! ```
//!
//! Or with the config file:
//!
//! ```bash
//! EVENT_CONFIG=config/events.toml CONTRACT_ID=C... cargo run --bin event-monitor
//! ```

use std::{env, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tracing::{error, info};

// Re-use the indexer crate's types directly.
use tipjar_indexer::{
    api::events as events_api,
    db::schema,
    event_listener::{EventListener, HorizonClient, IndexedEvent},
    AppState,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "event_monitor=info,tipjar_indexer=info".into()),
        )
        .init();

    let cfg = Config::from_env()?;

    info!(
        contract = %cfg.contract_id,
        rpc_url  = %cfg.rpc_url,
        bind     = %cfg.bind_addr,
        "event monitor starting"
    );

    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.database_url)
        .await
        .context("failed to connect to postgres")?;

    schema::ensure_schema(&db_pool)
        .await
        .context("failed to ensure schema")?;

    let (stream_tx, _) = broadcast::channel::<IndexedEvent>(2_000);

    let listener = Arc::new(EventListener::new(
        HorizonClient::new(cfg.rpc_url.clone(), cfg.page_size),
        cfg.contract_id.clone(),
        db_pool.clone(),
        stream_tx.clone(),
        Duration::from_secs(cfg.poll_interval_secs),
        cfg.max_retries,
        cfg.start_ledger,
    ));

    let listener_task = {
        let l = listener.clone();
        tokio::spawn(async move {
            if let Err(err) = l.start().await {
                error!(error = %err, "event listener stopped unexpectedly");
            }
        })
    };

    let app_state = AppState {
        db_pool,
        listener,
        stream_tx,
    };

    let app = Router::new().nest("/api", events_api::routes(app_state));

    let tcp = tokio::net::TcpListener::bind(cfg.bind_addr).await?;
    info!(bind = %cfg.bind_addr, "HTTP/SSE API listening");

    axum::serve(tcp, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            info!("shutdown signal received");
        })
        .await
        .map_err(|e| anyhow!(e))?;

    listener_task.abort();
    Ok(())
}

struct Config {
    database_url: String,
    rpc_url: String,
    contract_id: String,
    bind_addr: SocketAddr,
    poll_interval_secs: u64,
    max_retries: usize,
    start_ledger: Option<u64>,
    page_size: usize,
}

impl Config {
    fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://tipjar:tipjar@localhost:5432/tipjar".to_string()),
            rpc_url: env::var("STELLAR_HORIZON_URL")
                .or_else(|_| env::var("STELLAR_RPC_URL"))
                .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".to_string()),
            contract_id: env::var("CONTRACT_ID")
                .map_err(|_| anyhow!("CONTRACT_ID env var is required"))?,
            bind_addr: SocketAddr::from_str(
                &env::var("MONITOR_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string()),
            )
            .context("invalid MONITOR_BIND")?,
            poll_interval_secs: env::var("MONITOR_POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            max_retries: env::var("MONITOR_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            start_ledger: env::var("MONITOR_START_LEDGER")
                .ok()
                .and_then(|v| v.parse().ok()),
            page_size: env::var("MONITOR_PAGE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100),
        })
    }
}
