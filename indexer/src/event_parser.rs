use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;

use crate::event_listener::RawContractEvent;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Tip,
    TipWithMessage,
    Withdraw,
    TipLocked,
    WithdrawLocked,
    Refund,
    RefundRequested,
    RefundApproved,
    RoleGranted,
    RoleRevoked,
    MatchCreated,
    MatchApplied,
    MatchCancelled,
    Upgraded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEvent {
    pub kind: EventKind,
    pub topic: String,
    pub fields: Value,
}

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("event has no topic entries")]
    MissingTopic,
    #[error("topic[0] is not a recognizable symbol")]
    InvalidTopic,
    #[error("malformed payload for topic {topic}: {reason}")]
    InvalidPayload { topic: String, reason: String },
}

pub fn parse_event(event: &RawContractEvent) -> Result<ParsedEvent, ParserError> {
    let topic = event.topic.first().ok_or(ParserError::MissingTopic)?;
    let topic_symbol = topic_symbol(topic).ok_or(ParserError::InvalidTopic)?;

    let parsed = match topic_symbol.as_str() {
        "tip" => ParsedEvent {
            kind: EventKind::Tip,
            topic: topic_symbol,
            fields: parse_tip(event)?,
        },
        "tip_msg" => ParsedEvent {
            kind: EventKind::TipWithMessage,
            topic: topic_symbol,
            fields: parse_tip_message(event)?,
        },
        "withdraw" => ParsedEvent {
            kind: EventKind::Withdraw,
            topic: topic_symbol,
            fields: parse_withdraw(event)?,
        },
        "tip_lckd" => ParsedEvent {
            kind: EventKind::TipLocked,
            topic: topic_symbol,
            fields: parse_tip_locked(event)?,
        },
        "lck_wdrw" => ParsedEvent {
            kind: EventKind::WithdrawLocked,
            topic: topic_symbol,
            fields: parse_withdraw_locked(event)?,
        },
        "refund" => ParsedEvent {
            kind: EventKind::Refund,
            topic: topic_symbol,
            fields: parse_refund(event, false)?,
        },
        "ref_req" => ParsedEvent {
            kind: EventKind::RefundRequested,
            topic: topic_symbol,
            fields: parse_refund_request(event)?,
        },
        "ref_appr" => ParsedEvent {
            kind: EventKind::RefundApproved,
            topic: topic_symbol,
            fields: parse_refund(event, true)?,
        },
        "role_grnt" => ParsedEvent {
            kind: EventKind::RoleGranted,
            topic: topic_symbol,
            fields: parse_role_event(event)?,
        },
        "role_rvk" => ParsedEvent {
            kind: EventKind::RoleRevoked,
            topic: topic_symbol,
            fields: parse_role_event(event)?,
        },
        "match_new" => ParsedEvent {
            kind: EventKind::MatchCreated,
            topic: topic_symbol,
            fields: parse_match_new(event)?,
        },
        "tip_match" => ParsedEvent {
            kind: EventKind::MatchApplied,
            topic: topic_symbol,
            fields: parse_tip_match(event)?,
        },
        "match_end" => ParsedEvent {
            kind: EventKind::MatchCancelled,
            topic: topic_symbol,
            fields: parse_match_end(event)?,
        },
        "upgraded" => ParsedEvent {
            kind: EventKind::Upgraded,
            topic: topic_symbol,
            fields: parse_upgraded(event)?,
        },
        _ => ParsedEvent {
            kind: EventKind::Unknown,
            topic: topic_symbol,
            fields: json!({
                "raw_topic": event.topic,
                "raw_value": event.value,
            }),
        },
    };

    Ok(parsed)
}

fn parse_tip(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "tip")?;
    if data.len() < 2 {
        return Err(payload_err("tip", "expected [sender, amount]"));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "sender": value_to_string(&data[0]).unwrap_or_else(|| data[0].to_string()),
        "amount": value_to_i128(&data[1]),
    }))
}

fn parse_tip_message(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let data = value_array(event, "tip_msg")?;
    if data.len() < 4 {
        return Err(payload_err(
            "tip_msg",
            "expected [sender, amount, message, metadata]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "sender": value_to_string(&data[0]).unwrap_or_else(|| data[0].to_string()),
        "amount": value_to_i128(&data[1]),
        "message": value_to_string(&data[2]).unwrap_or_default(),
        "metadata": data[3],
    }))
}

fn parse_withdraw(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;

    Ok(json!({
        "creator": creator,
        "token": token,
        "amount": value_to_i128(&event.value),
    }))
}

fn parse_tip_locked(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "tip_lckd")?;
    if data.len() < 4 {
        return Err(payload_err(
            "tip_lckd",
            "expected [tip_id, sender, amount, unlock_timestamp]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "tip_id": value_to_u64(&data[0]),
        "sender": value_to_string(&data[1]).unwrap_or_else(|| data[1].to_string()),
        "amount": value_to_i128(&data[2]),
        "unlock_timestamp": value_to_u64(&data[3]),
    }))
}

fn parse_withdraw_locked(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "lck_wdrw")?;
    if data.len() < 2 {
        return Err(payload_err("lck_wdrw", "expected [tip_id, amount]"));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "tip_id": value_to_u64(&data[0]),
        "amount": value_to_i128(&data[1]),
    }))
}

fn parse_refund(event: &RawContractEvent, approved: bool) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let data = value_array(event, if approved { "ref_appr" } else { "refund" })?;
    if data.len() < 3 {
        return Err(payload_err(
            if approved { "ref_appr" } else { "refund" },
            "expected [tip_id, sender, amount]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "tip_id": value_to_u64(&data[0]),
        "sender": value_to_string(&data[1]).unwrap_or_else(|| data[1].to_string()),
        "amount": value_to_i128(&data[2]),
    }))
}

fn parse_refund_request(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let data = value_array(event, "ref_req")?;
    if data.len() < 2 {
        return Err(payload_err("ref_req", "expected [tip_id, sender]"));
    }

    Ok(json!({
        "creator": creator,
        "tip_id": value_to_u64(&data[0]),
        "sender": value_to_string(&data[1]).unwrap_or_else(|| data[1].to_string()),
    }))
}

fn parse_role_event(event: &RawContractEvent) -> Result<Value, ParserError> {
    let target = topic_value(event, 1, "target")?;
    let role = topic_value(event, 2, "role")?;

    Ok(json!({
        "target": target,
        "role": role,
        "caller": value_to_string(&event.value).unwrap_or_else(|| event.value.to_string()),
    }))
}

fn parse_match_new(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "match_new")?;
    if data.len() < 4 {
        return Err(payload_err(
            "match_new",
            "expected [sponsor, program_id, match_ratio, max_match_amount]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "sponsor": value_to_string(&data[0]).unwrap_or_else(|| data[0].to_string()),
        "program_id": value_to_u64(&data[1]),
        "match_ratio": value_to_u64(&data[2]),
        "max_match_amount": value_to_i128(&data[3]),
    }))
}

fn parse_tip_match(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "tip_match")?;
    if data.len() < 3 {
        return Err(payload_err(
            "tip_match",
            "expected [sender, amount, matched_amount]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "sender": value_to_string(&data[0]).unwrap_or_else(|| data[0].to_string()),
        "amount": value_to_i128(&data[1]),
        "matched_amount": value_to_i128(&data[2]),
    }))
}

fn parse_match_end(event: &RawContractEvent) -> Result<Value, ParserError> {
    let creator = topic_value(event, 1, "creator")?;
    let token = topic_value(event, 2, "token")?;
    let data = value_array(event, "match_end")?;
    if data.len() < 3 {
        return Err(payload_err(
            "match_end",
            "expected [sponsor, program_id, unspent]",
        ));
    }

    Ok(json!({
        "creator": creator,
        "token": token,
        "sponsor": value_to_string(&data[0]).unwrap_or_else(|| data[0].to_string()),
        "program_id": value_to_u64(&data[1]),
        "unspent": value_to_i128(&data[2]),
    }))
}

fn parse_upgraded(event: &RawContractEvent) -> Result<Value, ParserError> {
    let admin = topic_value(event, 1, "admin")?;

    Ok(json!({
        "admin": admin,
        "version": value_to_u64(&event.value),
    }))
}

fn topic_value(event: &RawContractEvent, idx: usize, field: &str) -> Result<String, ParserError> {
    event
        .topic
        .get(idx)
        .and_then(value_to_string)
        .ok_or_else(|| payload_err(topic_name(event), &format!("missing topic field {field}")))
}

fn topic_name(event: &RawContractEvent) -> String {
    event
        .topic
        .first()
        .and_then(topic_symbol)
        .unwrap_or_else(|| "<unknown>".to_string())
}

fn payload_err(topic: impl Into<String>, reason: impl Into<String>) -> ParserError {
    ParserError::InvalidPayload {
        topic: topic.into(),
        reason: reason.into(),
    }
}

fn value_array(event: &RawContractEvent, topic: &str) -> Result<Vec<Value>, ParserError> {
    match &event.value {
        Value::Array(v) => Ok(v.clone()),
        _ => Err(payload_err(topic, "value is not an array")),
    }
}

fn topic_symbol(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Object(map) => map
            .get("symbol")
            .and_then(value_to_string)
            .or_else(|| map.get("string").and_then(value_to_string)),
        _ => None,
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Object(map) => map
            .get("address")
            .and_then(value_to_string)
            .or_else(|| map.get("symbol").and_then(value_to_string))
            .or_else(|| map.get("string").and_then(value_to_string))
            .or_else(|| map.get("bytes").and_then(value_to_string))
            .or_else(|| map.get("u64").and_then(value_to_string))
            .or_else(|| map.get("i128").and_then(value_to_string)),
        _ => None,
    }
}

fn value_to_u64(value: &Value) -> u64 {
    match value {
        Value::Number(n) => n.as_u64().unwrap_or(0),
        Value::String(s) => s.parse::<u64>().unwrap_or(0),
        Value::Object(map) => map
            .get("u64")
            .and_then(value_to_string)
            .and_then(|v| v.parse::<u64>().ok())
            .or_else(|| {
                map.get("i128")
                    .and_then(value_to_string)
                    .and_then(|v| v.parse::<u64>().ok())
            })
            .unwrap_or(0),
        _ => 0,
    }
}

fn value_to_i128(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Object(map) => map
            .get("i128")
            .and_then(value_to_string)
            .or_else(|| map.get("u64").and_then(value_to_string))
            .unwrap_or_else(|| value.to_string()),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn event(topic: Vec<Value>, value: Value) -> RawContractEvent {
        RawContractEvent {
            id: "evt-1".to_string(),
            paging_token: Some("evt-1".to_string()),
            contract_id: Some("abc".to_string()),
            ledger: 100,
            ledger_closed_at: None,
            topic,
            value,
            tx_hash: Some("tx".to_string()),
            in_successful_contract_call: Some(true),
        }
    }

    #[test]
    fn parses_tip_event() {
        let e = event(
            vec![json!("tip"), json!("CREATOR"), json!("TOKEN")],
            json!(["SENDER", "1000"]),
        );

        let parsed = parse_event(&e).expect("tip parses");
        assert_eq!(parsed.kind, EventKind::Tip);
        assert_eq!(parsed.fields["creator"], "CREATOR");
        assert_eq!(parsed.fields["amount"], "1000");
    }

    #[test]
    fn rejects_malformed_tip_message() {
        let e = event(
            vec![json!("tip_msg"), json!("CREATOR")],
            json!(["sender", "1"]),
        );
        let err = parse_event(&e).expect_err("should fail");
        matches!(err, ParserError::InvalidPayload { .. });
    }

    #[test]
    fn unknown_topic_is_still_indexable() {
        let e = event(vec![json!("custom")], json!({"hello": "world"}));
        let parsed = parse_event(&e).expect("unknown topic should parse");
        assert_eq!(parsed.kind, EventKind::Unknown);
        assert_eq!(parsed.topic, "custom");
    }
}
