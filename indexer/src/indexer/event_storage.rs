//! Persistent storage helpers for indexed contract events.
//!
//! All functions target the `contract_events` table created by
//! `migrations/0003_create_events.sql`.

use anyhow::Result;
use serde_json::Value;
use sqlx::{PgPool, Row};

/// Inserts one event row.
///
/// Uses `ON CONFLICT (tx_hash) DO NOTHING` so replaying the same event is
/// idempotent.  Relies on the unique index on `tx_hash`.
pub async fn store_event(
    pool: &PgPool,
    event_type: &str,
    contract_id: &str,
    tx_hash: &str,
    sender: Option<&str>,
    recipient: Option<&str>,
    amount: Option<i64>,
    raw_data: &Value,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO contract_events
            (event_type, contract_id, tx_hash, sender, recipient, amount, raw_data)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (tx_hash) DO NOTHING
        "#,
    )
    .bind(event_type)
    .bind(contract_id)
    .bind(tx_hash)
    .bind(sender)
    .bind(recipient)
    .bind(amount)
    .bind(raw_data)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns a paginated list of events for a given recipient address.
///
/// Uses `idx_contract_events_recipient` for the filter and
/// `idx_contract_events_processed` for the ORDER BY.
pub async fn get_events_by_recipient(
    pool: &PgPool,
    recipient: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<serde_json::Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, event_type, contract_id, tx_hash, sender, recipient, amount, raw_data, processed_at
        FROM contract_events
        WHERE recipient = $1
        ORDER BY processed_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(recipient)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let events = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "id":           r.get::<i64, _>("id"),
                "event_type":   r.get::<String, _>("event_type"),
                "contract_id":  r.get::<String, _>("contract_id"),
                "tx_hash":      r.get::<String, _>("tx_hash"),
                "sender":       r.get::<Option<String>, _>("sender"),
                "recipient":    r.get::<Option<String>, _>("recipient"),
                "amount":       r.get::<Option<i64>, _>("amount"),
                "raw_data":     r.get::<Value, _>("raw_data"),
                "processed_at": r.get::<chrono::DateTime<chrono::Utc>, _>("processed_at").to_rfc3339(),
            })
        })
        .collect();

    Ok(events)
}

/// Aggregate stats for a contract: tip count, total tip volume, withdrawal count.
///
/// Scans `contract_events` filtered by `contract_id`; uses
/// `idx_contract_events_type` for the conditional aggregation.
pub async fn get_event_stats(pool: &PgPool, contract_id: &str) -> Result<serde_json::Value> {
    let row = sqlx::query(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE event_type = 'tip')      AS tip_count,
            COALESCE(SUM(amount) FILTER (WHERE event_type = 'tip'), 0) AS total_volume,
            COUNT(*) FILTER (WHERE event_type = 'withdraw') AS withdrawal_count
        FROM contract_events
        WHERE contract_id = $1
        "#,
    )
    .bind(contract_id)
    .fetch_one(pool)
    .await?;

    Ok(serde_json::json!({
        "tip_count":        row.get::<i64, _>("tip_count"),
        "total_volume":     row.get::<i64, _>("total_volume"),
        "withdrawal_count": row.get::<i64, _>("withdrawal_count"),
    }))
}
