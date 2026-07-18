/// Metrics collector for TipJar contract events
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipEvent {
    pub id: String,
    pub sender: String,
    pub creator: String,
    pub amount: i64,
    pub token: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalEvent {
    pub id: String,
    pub creator: String,
    pub amount: i64,
    pub token: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMetrics {
    pub date: String,
    pub total_tips: i64,
    pub total_volume: i64,
    pub unique_tippers: i64,
    pub unique_creators: i64,
    pub average_tip: f64,
}

pub struct MetricsCollector {
    db: PgPool,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Record a tip event
    pub async fn record_tip(&self, event: &TipEvent) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO tips (id, sender, creator, amount, token, timestamp) 
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(&event.id)
        .bind(&event.sender)
        .bind(&event.creator)
        .bind(event.amount)
        .bind(&event.token)
        .bind(event.timestamp)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Record a withdrawal event
    pub async fn record_withdrawal(&self, event: &WithdrawalEvent) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO withdrawals (id, creator, amount, token, timestamp) 
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&event.id)
        .bind(&event.creator)
        .bind(event.amount)
        .bind(&event.token)
        .bind(event.timestamp)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Get daily metrics for a specific date
    pub async fn get_daily_metrics(&self, date: &str) -> Result<DailyMetrics, sqlx::Error> {
        let row = sqlx::query_as::<_, (i64, i64, i64, i64)>(
            "SELECT 
                COUNT(*) as tip_count,
                COALESCE(SUM(amount), 0) as total_volume,
                COUNT(DISTINCT sender) as unique_tippers,
                COUNT(DISTINCT creator) as unique_creators
             FROM tips 
             WHERE DATE(timestamp) = $1",
        )
        .bind(date)
        .fetch_one(&self.db)
        .await?;

        let avg_tip = if row.0 > 0 {
            row.1 as f64 / row.0 as f64
        } else {
            0.0
        };

        Ok(DailyMetrics {
            date: date.to_string(),
            total_tips: row.0,
            total_volume: row.1,
            unique_tippers: row.2,
            unique_creators: row.3,
            average_tip: avg_tip,
        })
    }

    /// Get top creators by total tips
    pub async fn get_top_creators(&self, limit: i64) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT creator, SUM(amount) as total 
             FROM tips 
             GROUP BY creator 
             ORDER BY total DESC 
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }

    /// Get top tippers by total amount
    pub async fn get_top_tippers(&self, limit: i64) -> Result<Vec<(String, i64)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT sender, SUM(amount) as total 
             FROM tips 
             GROUP BY sender 
             ORDER BY total DESC 
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await?;

        Ok(rows)
    }
}
