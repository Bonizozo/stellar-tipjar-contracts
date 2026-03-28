use std::convert::Infallible;

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::{
    event_listener::{IndexedEvent, ReplayRequest, ReplaySummary},
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct EventQuery {
    pub topic: Option<String>,
    pub creator: Option<String>,
    pub sender: Option<String>,
    pub tx_hash: Option<String>,
    pub min_ledger: Option<i64>,
    pub max_ledger: Option<i64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ApiEvent {
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
    pub created_at: DateTime<Utc>,
}

pub fn routes(state: crate::AppState) -> Router {
    Router::new()
        .route("/events", get(list_events))
        .route("/events/:event_id", get(get_event))
        .route("/events/stream", get(stream_events))
        .route("/events/replay", post(replay_events))
        .with_state(state)
}

async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<EventQuery>,
) -> Result<Json<Vec<ApiEvent>>, (axum::http::StatusCode, String)> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500) as i64;
    let offset = query.offset.unwrap_or(0) as i64;

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"SELECT event_id, contract_id, cursor, ledger, ledger_closed_at, topic, topic_values,
                  event_data, parsed_data, raw_event, tx_hash, successful_call, created_at
           FROM contract_events WHERE 1 = 1"#,
    );

    if let Some(topic) = query.topic {
        qb.push(" AND topic = ").push_bind(topic);
    }

    if let Some(creator) = query.creator {
        qb.push(" AND parsed_data->'fields'->>'creator' = ")
            .push_bind(creator);
    }

    if let Some(sender) = query.sender {
        qb.push(" AND parsed_data->'fields'->>'sender' = ")
            .push_bind(sender);
    }

    if let Some(tx_hash) = query.tx_hash {
        qb.push(" AND tx_hash = ").push_bind(tx_hash);
    }

    if let Some(min_ledger) = query.min_ledger {
        qb.push(" AND ledger >= ").push_bind(min_ledger);
    }

    if let Some(max_ledger) = query.max_ledger {
        qb.push(" AND ledger <= ").push_bind(max_ledger);
    }

    qb.push(" ORDER BY ledger DESC, created_at DESC");
    qb.push(" LIMIT ").push_bind(limit);
    qb.push(" OFFSET ").push_bind(offset);

    let rows = qb
        .build()
        .fetch_all(&state.db_pool)
        .await
        .map_err(internal_error)?;

    let events = rows.into_iter().map(row_to_event).collect();
    Ok(Json(events))
}

async fn get_event(
    State(state): State<AppState>,
    Path(event_id): Path<String>,
) -> Result<Json<Option<ApiEvent>>, (axum::http::StatusCode, String)> {
    let row = sqlx::query(
        r#"SELECT event_id, contract_id, cursor, ledger, ledger_closed_at, topic, topic_values,
                  event_data, parsed_data, raw_event, tx_hash, successful_call, created_at
           FROM contract_events WHERE event_id = $1"#,
    )
    .bind(event_id)
    .fetch_optional(&state.db_pool)
    .await
    .map_err(internal_error)?;

    Ok(Json(row.map(row_to_event)))
}

async fn replay_events(
    State(state): State<AppState>,
    Json(request): Json<ReplayRequest>,
) -> Result<Json<ReplaySummary>, (axum::http::StatusCode, String)> {
    let summary = state
        .listener
        .replay(request)
        .await
        .map_err(internal_error)?;
    Ok(Json(summary))
}

async fn stream_events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.stream_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|evt| match evt {
        Ok(event) => {
            let payload = serde_json::to_string(&event).ok()?;
            Some(Ok(Event::default().event("contract_event").data(payload)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keep-alive"),
    )
}

#[allow(clippy::too_many_lines)]
fn row_to_event(row: PgRow) -> ApiEvent {
    ApiEvent {
        event_id: row.get("event_id"),
        contract_id: row.get("contract_id"),
        cursor: row.get("cursor"),
        ledger: row.get("ledger"),
        ledger_closed_at: row.get("ledger_closed_at"),
        topic: row.get("topic"),
        topic_values: row.get("topic_values"),
        event_data: row.get("event_data"),
        parsed_data: row.get("parsed_data"),
        raw_event: row.get("raw_event"),
        tx_hash: row.get("tx_hash"),
        successful_call: row.get("successful_call"),
        created_at: row.get("created_at"),
    }
}

fn internal_error<E: std::fmt::Display>(error: E) -> (axum::http::StatusCode, String) {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("internal error: {error}"),
    )
}

#[allow(dead_code)]
fn _as_realtime_event(event: IndexedEvent) -> Value {
    serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({ "error": "serialization" }))
}
