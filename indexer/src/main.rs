mod api {
    pub mod events;
}
mod db {
    pub mod schema;
}
mod event_listener;
mod event_parser;

use std::{env, net::SocketAddr, str::FromStr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::event_listener::{EventListener, HorizonClient, IndexedEvent};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: sqlx::PgPool,
    pub listener: Arc<EventListener>,
    pub stream_tx: broadcast::Sender<IndexedEvent>,
}

#[derive(Debug, Clone)]
struct Config {
    database_url: String,
    events_api_url: String,
    contract_id: String,
    bind_addr: SocketAddr,
    poll_interval_secs: u64,
    max_retries: usize,
    start_ledger: Option<u64>,
    page_size: usize,
}

impl Config {
    fn from_env() -> Result<Self> {
        let database_url = required_env("DATABASE_URL")?;
        let events_api_url = env::var("STELLAR_HORIZON_URL")
            .or_else(|_| env::var("STELLAR_RPC_URL"))
            .unwrap_or_else(|_| "https://horizon-testnet.stellar.org".to_string());
        let contract_id = required_env("CONTRACT_ID")?;

        let bind_addr = env::var("INDEXER_BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let bind_addr = SocketAddr::from_str(&bind_addr)
            .with_context(|| format!("invalid INDEXER_BIND value: {bind_addr}"))?;

        let poll_interval_secs = env::var("INDEXER_POLL_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5);

        let max_retries = env::var("INDEXER_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(5);

        let start_ledger = env::var("INDEXER_START_LEDGER")
            .ok()
            .and_then(|v| v.parse::<u64>().ok());

        let page_size = env::var("INDEXER_PAGE_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);

        Ok(Self {
            database_url,
            events_api_url,
            contract_id,
            bind_addr,
            poll_interval_secs,
            max_retries,
            start_ledger,
            page_size,
        })
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tipjar_indexer=info,axum=info".into()),
        )
        .init();

    let cfg = Config::from_env()?;

    let db_pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cfg.database_url)
        .await
        .context("failed to connect postgres")?;

    db::schema::ensure_schema(&db_pool)
        .await
        .context("failed creating schema")?;

    let (stream_tx, _) = broadcast::channel::<IndexedEvent>(2_000);

    let listener = Arc::new(EventListener::new(
        HorizonClient::new(cfg.events_api_url.clone(), cfg.page_size),
        cfg.contract_id.clone(),
        db_pool.clone(),
        stream_tx.clone(),
        Duration::from_secs(cfg.poll_interval_secs),
        cfg.max_retries,
        cfg.start_ledger,
    ));

    let listener_task = {
        let listener = listener.clone();
        tokio::spawn(async move {
            if let Err(err) = listener.start().await {
                error!(error = %err, "listener stopped unexpectedly");
            }
        })
    };

    let app_state = AppState {
        db_pool: db_pool.clone(),
        listener,
        stream_tx,
    };

    let app = Router::new().nest("/api", api::events::routes(app_state));

    info!(bind = %cfg.bind_addr, "tipjar indexer API starting");
    let tcp = tokio::net::TcpListener::bind(cfg.bind_addr).await?;

    axum::serve(tcp, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|err| anyhow!(err))?;

    listener_task.abort();

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("shutdown signal received");
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| anyhow!("missing required environment variable {name}"))
}
