use std::{env, net::SocketAddr, str::FromStr};

use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use tipjar_analytics::api::{routes, AnalyticsState};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tipjar_analytics=info".into()),
        )
        .init();

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://tipjar:tipjar@localhost:5432/tipjar".to_string());

    let bind = env::var("ANALYTICS_BIND").unwrap_or_else(|_| "0.0.0.0:8081".to_string());
    let addr = SocketAddr::from_str(&bind).context("invalid ANALYTICS_BIND")?;

    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let state = AnalyticsState { db };
    let app = axum::Router::new().nest("/analytics", routes(state));

    info!(bind = %addr, "analytics API starting");
    let tcp = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(tcp, app)
        .with_graceful_shutdown(async { let _ = tokio::signal::ctrl_c().await; })
        .await
        .context("server error")
}
