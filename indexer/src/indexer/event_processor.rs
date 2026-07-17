//! Typed event parsing and webhook delivery.
//!
//! Event topic strings match the `symbol_short!` values emitted by the
//! contract: `"tip"` and `"withdraw"`.  Topic layout:
//!   - tip:      topics = ["tip", creator, token]  value = [sender, amount]
//!   - withdraw: topics = ["withdraw", creator, token]  value = amount

use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use tracing::{error, warn};

/// Parsed fields from a `("tip", creator)` contract event.
#[derive(Debug, Serialize)]
pub struct TipEventData {
    pub creator: String,
    pub token: String,
    pub sender: String,
    pub amount: String,
}

/// Parsed fields from a `("withdraw", creator)` contract event.
#[derive(Debug, Serialize)]
pub struct WithdrawalEventData {
    pub creator: String,
    pub token: String,
    pub amount: String,
}

/// Parses a raw Horizon/RPC event value into [`TipEventData`].
///
/// Expected shape:
/// ```json
/// { "topic": ["tip", "<creator>", "<token>"], "value": ["<sender>", "<amount>"] }
/// ```
pub fn parse_tip_event(raw: &Value) -> Result<TipEventData> {
    let topic = raw
        .get("topic")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("missing topic array"))?;
    let creator = str_at(topic, 1, "creator")?;
    let token = str_at(topic, 2, "token")?;
    let value = raw
        .get("value")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("tip value is not an array"))?;
    let sender = str_at(value, 0, "sender")?;
    let amount = str_at(value, 1, "amount")?;
    Ok(TipEventData {
        creator,
        token,
        sender,
        amount,
    })
}

/// Parses a raw Horizon/RPC event value into [`WithdrawalEventData`].
///
/// Expected shape:
/// ```json
/// { "topic": ["withdraw", "<creator>", "<token>"], "value": "<amount>" }
/// ```
pub fn parse_withdrawal_event(raw: &Value) -> Result<WithdrawalEventData> {
    let topic = raw
        .get("topic")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow!("missing topic array"))?;
    let creator = str_at(topic, 1, "creator")?;
    let token = str_at(topic, 2, "token")?;
    let amount = val_to_string(raw.get("value").unwrap_or(&Value::Null));
    Ok(WithdrawalEventData {
        creator,
        token,
        amount,
    })
}

/// POSTs `payload` to `url` with up to 3 attempts (backoff: 1 s, 2 s, 4 s).
///
/// Reads the webhook URL from the `WEBHOOK_URL` environment variable.
/// Skips silently if the variable is unset.
/// Logs `tracing::warn!` on each retry and `tracing::error!` on final failure.
pub async fn send_webhook(url: &str, payload: &Value) -> Result<()> {
    let client = reqwest::Client::new();
    let mut delay_secs = 1u64;
    for attempt in 1..=3u32 {
        match client.post(url).json(payload).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                let status = resp.status();
                if attempt < 3 {
                    warn!(attempt, %status, "webhook non-success; retrying in {delay_secs}s");
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    delay_secs *= 2;
                } else {
                    error!(%status, "webhook failed after 3 attempts");
                    return Err(anyhow!("webhook returned {status} after 3 attempts"));
                }
            }
            Err(e) => {
                if attempt < 3 {
                    warn!(attempt, error = %e, "webhook error; retrying in {delay_secs}s");
                    tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
                    delay_secs *= 2;
                } else {
                    error!(error = %e, "webhook failed after 3 attempts");
                    return Err(anyhow!(e));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn str_at(arr: &[Value], idx: usize, field: &str) -> Result<String> {
    arr.get(idx)
        .map(val_to_string)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("missing field '{field}' at index {idx}"))
}

fn val_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Object(m) => m
            .get("address")
            .or_else(|| m.get("symbol"))
            .or_else(|| m.get("string"))
            .or_else(|| m.get("i128"))
            .or_else(|| m.get("u64"))
            .map(val_to_string)
            .unwrap_or_default(),
        _ => String::new(),
    }
}
