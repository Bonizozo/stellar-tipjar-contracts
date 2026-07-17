use std::{future::Future, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::event_parser::{parse_event, ParsedEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawContractEvent {
    pub id: String,
    #[serde(default, alias = "pagingToken")]
    pub paging_token: Option<String>,
    #[serde(default, alias = "contractId")]
    pub contract_id: Option<String>,
    pub ledger: i64,
    #[serde(default, alias = "ledgerClosedAt", alias = "closed_at")]
    pub ledger_closed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub topic: Vec<Value>,
    #[serde(default)]
    pub value: Value,
    #[serde(default, alias = "txHash", alias = "transaction_hash")]
    pub tx_hash: Option<String>,
    #[serde(default, alias = "inSuccessfulContractCall")]
    pub in_successful_contract_call: Option<bool>,
}

impl RawContractEvent {
    pub fn cursor(&self) -> String {
        self.paging_token.clone().unwrap_or_else(|| self.id.clone())
    }
}

#[derive(Debug, Deserialize)]
struct RpcResponse<T> {
    result: Option<T>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventsResult {
    #[serde(default)]
    events: Vec<RawContractEvent>,
    #[serde(default)]
    latest_ledger: Option<u64>,
    #[serde(default)]
    cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EventsPage {
    pub events: Vec<RawContractEvent>,
    pub latest_ledger: Option<u64>,
    pub next_cursor: Option<String>,
}

#[derive(Clone)]
pub struct HorizonClient {
    http: Client,
    rpc_url: String,
    page_size: usize,
}

impl HorizonClient {
    pub fn new(rpc_url: impl Into<String>, page_size: usize) -> Self {
        Self {
            http: Client::new(),
            rpc_url: rpc_url.into(),
            page_size,
        }
    }

    pub async fn get_events(
        &self,
        contract_id: &str,
        cursor: Option<&str>,
        start_ledger: Option<u64>,
    ) -> Result<EventsPage> {
        match self
            .get_events_horizon_rest(contract_id, cursor, start_ledger)
            .await
        {
            Ok(page) => Ok(page),
            Err(rest_err) => {
                warn!(error = %rest_err, "horizon REST /events failed; falling back to RPC getEvents");
                self.get_events_rpc(contract_id, cursor, start_ledger).await
            }
        }
    }

    async fn get_events_horizon_rest(
        &self,
        contract_id: &str,
        cursor: Option<&str>,
        start_ledger: Option<u64>,
    ) -> Result<EventsPage> {
        let events_url = format!("{}/events", self.rpc_url.trim_end_matches('/'));
        let limit = self.page_size.to_string();
        let mut query: Vec<(&str, String)> = vec![
            ("order", "asc".to_string()),
            ("limit", limit),
            ("contract_ids", contract_id.to_string()),
        ];
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor.to_string()));
        } else if let Some(start_ledger) = start_ledger {
            query.push(("start_ledger", start_ledger.to_string()));
        }

        let response = self
            .http
            .get(&events_url)
            .query(&query)
            .send()
            .await
            .with_context(|| format!("requesting Horizon events from {events_url}"))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "horizon /events returned status {}: {}",
                status,
                body
            ));
        }

        let payload: Value = response
            .json()
            .await
            .with_context(|| format!("decoding Horizon /events response status={status}"))?;

        let events = payload
            .get("_embedded")
            .and_then(|v| v.get("records"))
            .and_then(|v| v.as_array())
            .or_else(|| payload.get("events").and_then(|v| v.as_array()))
            .ok_or_else(|| anyhow!("horizon /events payload missing records"))?
            .iter()
            .map(|v| serde_json::from_value::<RawContractEvent>(v.clone()))
            .collect::<std::result::Result<Vec<_>, _>>()
            .with_context(|| "parsing Horizon /events records")?;

        let latest_ledger = payload
            .get("latest_ledger")
            .or_else(|| payload.get("latestLedger"))
            .and_then(|v| v.as_u64());

        let next_cursor = events.last().map(RawContractEvent::cursor);
        Ok(EventsPage {
            events,
            latest_ledger,
            next_cursor,
        })
    }

    async fn get_events_rpc(
        &self,
        contract_id: &str,
        cursor: Option<&str>,
        start_ledger: Option<u64>,
    ) -> Result<EventsPage> {
        let mut params = json!({
            "filters": [{
                "type": "contract",
                "contractIds": [contract_id]
            }],
            "pagination": {
                "limit": self.page_size
            }
        });

        if let Some(cursor) = cursor {
            params["pagination"]["cursor"] = Value::String(cursor.to_string());
        } else if let Some(start_ledger) = start_ledger {
            params["startLedger"] = json!(start_ledger);
        }

        let body = json!({
            "jsonrpc": "2.0",
            "id": "tipjar-indexer",
            "method": "getEvents",
            "params": params
        });

        let response = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("requesting RPC getEvents from {}", self.rpc_url))?;

        let status = response.status();
        let payload: RpcResponse<EventsResult> = response
            .json()
            .await
            .with_context(|| format!("decoding RPC getEvents response status={status}"))?;

        if let Some(err) = payload.error {
            return Err(anyhow!("rpc error {}: {}", err.code, err.message));
        }

        let result = payload
            .result
            .ok_or_else(|| anyhow!("missing rpc result"))?;
        let next_cursor = result
            .cursor
            .or_else(|| result.events.last().map(RawContractEvent::cursor));

        Ok(EventsPage {
            events: result.events,
            latest_ledger: result.latest_ledger,
            next_cursor,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexedEvent {
    pub event_id: String,
    pub contract_id: String,
    pub cursor: String,
    pub ledger: i64,
    pub ledger_closed_at: Option<DateTime<Utc>>,
    pub topic: String,
    pub topic_values: Value,
    pub event_data: Value,
    pub parsed_data: Value,
    pub raw_event: Value,
    pub tx_hash: Option<String>,
    pub successful_call: Option<bool>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayRequest {
    pub from_cursor: Option<String>,
    pub from_ledger: Option<u64>,
    pub to_ledger: Option<u64>,
    pub persist_cursor: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReplaySummary {
    pub indexed_events: usize,
    pub failed_events: usize,
    pub last_cursor: Option<String>,
    pub latest_ledger_seen: Option<u64>,
}

#[derive(Clone)]
pub struct EventListener {
    pub horizon_client: HorizonClient,
    pub contract_id: String,
    pub db_pool: PgPool,
    pub stream_tx: broadcast::Sender<IndexedEvent>,
    pub poll_interval: Duration,
    pub max_retries: usize,
    pub boot_start_ledger: Option<u64>,
}

impl EventListener {
    pub fn new(
        horizon_client: HorizonClient,
        contract_id: impl Into<String>,
        db_pool: PgPool,
        stream_tx: broadcast::Sender<IndexedEvent>,
        poll_interval: Duration,
        max_retries: usize,
        boot_start_ledger: Option<u64>,
    ) -> Self {
        Self {
            horizon_client,
            contract_id: contract_id.into(),
            db_pool,
            stream_tx,
            poll_interval,
            max_retries,
            boot_start_ledger,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let mut cursor = self.load_cursor().await?;
        info!(contract = %self.contract_id, cursor = ?cursor, "indexer listener started");

        loop {
            let request_cursor = cursor.clone();
            let boot_start_ledger = self.boot_start_ledger;
            let page = retry_with_backoff(self.max_retries, || {
                let client = self.horizon_client.clone();
                let contract_id = self.contract_id.clone();
                let request_cursor = request_cursor.clone();
                async move {
                    client
                        .get_events(
                            &contract_id,
                            request_cursor.as_deref(),
                            if request_cursor.is_none() {
                                boot_start_ledger
                            } else {
                                None
                            },
                        )
                        .await
                }
            })
            .await;

            let page = match page {
                Ok(page) => page,
                Err(err) => {
                    error!(error = %err, "event fetch failed after retries; continuing");
                    tokio::time::sleep(self.poll_interval).await;
                    continue;
                }
            };
            if let Some(latest) = page.latest_ledger {
                info!(
                    latest_ledger = latest,
                    "latest ledger observed from event source"
                );
            }

            if page.events.is_empty() {
                tokio::time::sleep(self.poll_interval).await;
                continue;
            }

            for event in &page.events {
                if let Err(err) = self.process_event(event).await {
                    error!(event_id = %event.id, error = %err, "failed to process event");
                }
            }

            if let Some(next_cursor) = page.next_cursor {
                cursor = Some(next_cursor.clone());
                if let Err(err) = self.save_cursor(&next_cursor).await {
                    error!(error = %err, "failed to save cursor state");
                }
            }

            tokio::time::sleep(self.poll_interval).await;
        }
    }

    pub async fn replay(&self, request: ReplayRequest) -> Result<ReplaySummary> {
        let mut cursor = if let Some(cursor) = request.from_cursor.clone() {
            Some(cursor)
        } else {
            self.load_cursor().await?
        };

        if cursor.as_deref() == Some("") {
            cursor = None;
        }

        let mut indexed = 0usize;
        let mut failed = 0usize;
        let mut latest_ledger_seen = None;

        loop {
            let page = self
                .horizon_client
                .get_events(
                    &self.contract_id,
                    cursor.as_deref(),
                    if cursor.is_none() {
                        request.from_ledger
                    } else {
                        None
                    },
                )
                .await?;
            if let Some(latest) = page.latest_ledger {
                info!(
                    latest_ledger = latest,
                    "latest ledger observed during replay"
                );
            }

            if page.events.is_empty() {
                break;
            }

            for event in &page.events {
                if let Some(max_ledger) = request.to_ledger {
                    if event.ledger as u64 > max_ledger {
                        return Ok(ReplaySummary {
                            indexed_events: indexed,
                            failed_events: failed,
                            last_cursor: cursor,
                            latest_ledger_seen,
                        });
                    }
                }

                latest_ledger_seen = Some(event.ledger as u64);
                match self.process_event(event).await {
                    Ok(()) => indexed += 1,
                    Err(err) => {
                        failed += 1;
                        warn!(event_id = %event.id, error = %err, "event replay processing failed");
                    }
                }
            }

            cursor = page.next_cursor;

            if request.persist_cursor.unwrap_or(false) {
                if let Some(ref c) = cursor {
                    self.save_cursor(c).await?;
                }
            }
        }

        Ok(ReplaySummary {
            indexed_events: indexed,
            failed_events: failed,
            last_cursor: cursor,
            latest_ledger_seen,
        })
    }

    async fn process_event(&self, event: &RawContractEvent) -> Result<()> {
        let parsed = match parse_event(event) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.record_failure(event, &err.to_string()).await?;
                return Err(anyhow!(err));
            }
        };

        self.persist_event(event, &parsed).await
    }

    async fn persist_event(&self, event: &RawContractEvent, parsed: &ParsedEvent) -> Result<()> {
        let raw_event = serde_json::to_value(event)?;
        let topic_values = serde_json::to_value(&event.topic)?;
        let cursor = event.cursor();
        let parsed_data = serde_json::to_value(parsed)?;

        retry_with_backoff(self.max_retries, || {
            let db = self.db_pool.clone();
            let contract_id = self.contract_id.clone();
            let event_id = event.id.clone();
            let cursor = cursor.clone();
            let topic = parsed.topic.clone();
            let topic_values = topic_values.clone();
            let event_data = event.value.clone();
            let parsed_data = parsed_data.clone();
            let raw_event = raw_event.clone();
            let tx_hash = event.tx_hash.clone();
            let successful_call = event.in_successful_contract_call;
            let ledger_closed_at = event.ledger_closed_at;
            async move {
                sqlx::query(
                    r#"
                    INSERT INTO contract_events (
                        contract_id, event_id, cursor, ledger, ledger_closed_at,
                        topic, topic_values, event_data, parsed_data, raw_event, tx_hash, successful_call
                    )
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                    ON CONFLICT (event_id) DO NOTHING
                    "#,
                )
                .bind(&contract_id)
                .bind(&event_id)
                .bind(&cursor)
                .bind(event.ledger)
                .bind(ledger_closed_at)
                .bind(&topic)
                .bind(topic_values)
                .bind(event_data)
                .bind(parsed_data)
                .bind(raw_event)
                .bind(tx_hash)
                .bind(successful_call)
                .execute(&db)
                .await?;

                Ok::<(), anyhow::Error>(())
            }
        })
        .await
        .map_err(|err| {
            anyhow!(
                "failed to persist event {} after retries: {}",
                event.id,
                err
            )
        })?;

        let indexed = IndexedEvent {
            event_id: event.id.clone(),
            contract_id: event
                .contract_id
                .clone()
                .unwrap_or_else(|| self.contract_id.clone()),
            cursor,
            ledger: event.ledger,
            ledger_closed_at: event.ledger_closed_at,
            topic: parsed.topic.clone(),
            topic_values,
            event_data: event.value.clone(),
            parsed_data: serde_json::to_value(parsed)?,
            raw_event,
            tx_hash: event.tx_hash.clone(),
            successful_call: event.in_successful_contract_call,
            indexed_at: Utc::now(),
        };

        if let Err(err) = self.stream_tx.send(indexed) {
            warn!(error = %err, "failed to push event to realtime stream");
        }

        Ok(())
    }

    pub async fn record_failure(&self, event: &RawContractEvent, reason: &str) -> Result<()> {
        let raw_event = serde_json::to_value(event)?;

        sqlx::query(
            r#"
            INSERT INTO event_failures (event_id, contract_id, reason, retry_count, raw_event)
            VALUES ($1, $2, $3, 1, $4)
            ON CONFLICT (event_id)
            DO UPDATE SET
                reason = EXCLUDED.reason,
                retry_count = event_failures.retry_count + 1,
                raw_event = EXCLUDED.raw_event,
                last_error_at = NOW()
            "#,
        )
        .bind(&event.id)
        .bind(&self.contract_id)
        .bind(reason)
        .bind(raw_event)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }

    pub async fn load_cursor(&self) -> Result<Option<String>> {
        let key = format!("cursor:{}", self.contract_id);
        let row = sqlx::query("SELECT value FROM indexer_state WHERE key = $1")
            .bind(key)
            .fetch_optional(&self.db_pool)
            .await?;

        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    pub async fn save_cursor(&self, cursor: &str) -> Result<()> {
        let key = format!("cursor:{}", self.contract_id);
        sqlx::query(
            r#"
            INSERT INTO indexer_state (key, value)
            VALUES ($1, $2)
            ON CONFLICT (key)
            DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()
            "#,
        )
        .bind(key)
        .bind(cursor)
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}

pub async fn retry_with_backoff<T, F, Fut>(max_retries: usize, mut op: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let mut attempt = 0usize;
    let mut delay_ms = 400u64;

    loop {
        match op().await {
            Ok(output) => return Ok(output),
            Err(err) => {
                attempt += 1;
                if attempt > max_retries {
                    return Err(err);
                }

                warn!(
                    attempt,
                    max_retries,
                    delay_ms,
                    error = %err,
                    "operation failed; retrying"
                );

                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                delay_ms = (delay_ms.saturating_mul(2)).min(10_000);
            }
        }
    }
}
