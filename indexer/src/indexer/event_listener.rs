//! High-level event monitoring loop.
//!
//! [`EventIndexer`] wraps the lower-level [`crate::event_listener::EventListener`]
//! and adds webhook delivery via [`super::event_processor::send_webhook`].

use std::time::Duration;

use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use sqlx::PgPool;
use tracing::{error, info};

use super::event_processor::{parse_tip_event, parse_withdrawal_event, send_webhook};
use super::event_storage::store_event;
use crate::event_listener::{EventListener, HorizonClient};
use tokio::sync::broadcast;

/// Polls Horizon for contract events, stores them, and fires webhooks.
pub struct EventIndexer {
    pub listener: std::sync::Arc<EventListener>,
    pub pool: PgPool,
    pub contract_id: String,
}

impl EventIndexer {
    pub fn new(
        rpc_url: impl Into<String>,
        contract_id: impl Into<String>,
        pool: PgPool,
        page_size: usize,
        poll_interval: Duration,
        max_retries: usize,
        start_ledger: Option<u64>,
    ) -> Self {
        let contract_id = contract_id.into();
        let (tx, _) = broadcast::channel(1_000);
        let listener = std::sync::Arc::new(EventListener::new(
            HorizonClient::new(rpc_url, page_size),
            contract_id.clone(),
            pool.clone(),
            tx,
            poll_interval,
            max_retries,
            start_ledger,
        ));
        Self {
            listener,
            pool,
            contract_id,
        }
    }

    /// Starts the monitoring loop.
    ///
    /// # Cursor strategy
    ///
    /// The last processed event cursor is persisted in the `indexer_state`
    /// table (key `cursor:<contract_id>`).  On restart the loop resumes from
    /// that cursor, so no events are reprocessed.  `ON CONFLICT DO NOTHING`
    /// in [`store_event`] provides a second layer of idempotency.
    ///
    /// # Retry behaviour
    ///
    /// Horizon fetch errors are retried with exponential backoff
    /// (2 s → 4 s → 8 s → 16 s → 32 s, max 5 attempts).  After all retries
    /// are exhausted the error is logged and the loop continues — the indexer
    /// never panics.
    ///
    /// # Webhook delivery
    ///
    /// If `WEBHOOK_URL` is set, each successfully stored event is POSTed to
    /// that URL.  Delivery failures are logged but do not interrupt indexing.
    pub async fn start_monitoring(&self) -> Result<()> {
        let webhook_url = std::env::var("WEBHOOK_URL").ok();
        let pool = self.pool.clone();
        let contract_id = self.contract_id.clone();

        // Delegate polling + persistence to the existing EventListener, then
        // layer webhook delivery on top via the broadcast stream.
        let mut rx = self.listener.stream_tx.subscribe();
        let listener = self.listener.clone();

        tokio::spawn(async move {
            if let Err(e) = listener.start().await {
                error!(error = %e, "EventListener stopped");
            }
        });

        info!(contract = %contract_id, "EventIndexer webhook layer started");

        loop {
            match rx.recv().await {
                Ok(indexed) => {
                    // Re-store via event_storage for the contract_events table
                    // defined in 0003_create_events.sql (separate from the
                    // richer table managed by EventListener).
                    let raw: Value = indexed.raw_event.clone();
                    let (event_type, sender, recipient, amount) =
                        classify(&indexed.topic, &indexed.parsed_data);

                    if let Err(e) = store_event(
                        &pool,
                        &event_type,
                        &contract_id,
                        indexed.tx_hash.as_deref().unwrap_or(&indexed.event_id),
                        sender.as_deref(),
                        recipient.as_deref(),
                        amount,
                        &raw,
                    )
                    .await
                    {
                        error!(error = %e, "store_event failed");
                    }

                    if let Some(ref url) = webhook_url {
                        let payload = serde_json::json!({
                            "event_type": event_type,
                            "contract_id": contract_id,
                            "tx_hash": indexed.tx_hash,
                            "data": indexed.parsed_data,
                        });
                        if let Err(e) = send_webhook(url, &payload).await {
                            error!(error = %e, "webhook delivery failed");
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    error!(skipped = n, "webhook receiver lagged; events skipped");
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }

        Ok(())
    }
}

/// Extracts (event_type, sender, recipient, amount) from parsed event data.
fn classify(topic: &str, parsed: &Value) -> (String, Option<String>, Option<String>, Option<i64>) {
    let fields = parsed.get("fields").unwrap_or(parsed);
    let sender = fields
        .get("sender")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let creator = fields
        .get("creator")
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let amount = fields
        .get("amount")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| fields.get("amount").and_then(|v| v.as_i64()));

    let (event_type, recipient) = match topic {
        "tip" => ("tip".to_owned(), creator),
        "withdraw" => ("withdraw".to_owned(), creator),
        other => (other.to_owned(), creator),
    };

    (event_type, sender, recipient, amount)
}
