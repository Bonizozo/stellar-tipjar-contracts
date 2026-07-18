use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Summary metrics for a time window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractMetrics {
    pub period: String,
    pub total_tips: i64,
    pub total_volume: i64,
    pub unique_tippers: i64,
    pub unique_creators: i64,
    pub avg_tip_amount: f64,
}

/// Per-creator performance snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorMetrics {
    pub creator: String,
    pub total_received: i64,
    pub tip_count: i64,
    pub avg_tip: f64,
    pub last_tip_at: Option<DateTime<Utc>>,
}

/// Per-tipper activity snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipperMetrics {
    pub tipper: String,
    pub total_sent: i64,
    pub tip_count: i64,
    pub avg_tip: f64,
}

/// Query parameters shared by several endpoints.
#[derive(Debug, Deserialize)]
pub struct DateRangeQuery {
    /// Inclusive start date, e.g. `2026-01-01`.
    pub start_date: Option<String>,
    /// Inclusive end date, e.g. `2026-12-31`.
    pub end_date: Option<String>,
    pub limit: Option<i64>,
}
