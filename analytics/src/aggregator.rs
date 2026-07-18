/// Data aggregation for metrics
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedMetrics {
    pub period: String,
    pub total_tips: i64,
    pub total_volume: i64,
    pub unique_participants: i64,
    pub average_tip_size: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatorStats {
    pub creator: String,
    pub total_received: i64,
    pub tip_count: i64,
    pub average_tip: f64,
    pub last_tip: DateTime<Utc>,
}

pub struct MetricsAggregator {
    db: PgPool,
}

impl MetricsAggregator {
    /// Create new aggregator
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Get aggregated metrics for a time period
    pub async fn get_period_metrics(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<AggregatedMetrics, sqlx::Error> {
        let row = sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT 
                COUNT(*) as tip_count,
                COALESCE(SUM(amount), 0) as total_volume,
                COUNT(DISTINCT sender) + COUNT(DISTINCT creator) as unique_participants
             FROM tips 
             WHERE DATE(timestamp) BETWEEN $1 AND $2",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&self.db)
        .await?;

        let avg_tip = if row.0 > 0 {
            row.1 as f64 / row.0 as f64
        } else {
            0.0
        };

        Ok(AggregatedMetrics {
            period: format!("{} to {}", start_date, end_date),
            total_tips: row.0,
            total_volume: row.1,
            unique_participants: row.2,
            average_tip_size: avg_tip,
        })
    }

    /// Get creator statistics
    pub async fn get_creator_stats(&self, creator: &str) -> Result<CreatorStats, sqlx::Error> {
        let row = sqlx::query_as::<_, (i64, i64, DateTime<Utc>)>(
            "SELECT 
                COALESCE(SUM(amount), 0) as total_received,
                COUNT(*) as tip_count,
                MAX(timestamp) as last_tip
             FROM tips 
             WHERE creator = $1",
        )
        .bind(creator)
        .fetch_one(&self.db)
        .await?;

        let avg_tip = if row.1 > 0 {
            row.0 as f64 / row.1 as f64
        } else {
            0.0
        };

        Ok(CreatorStats {
            creator: creator.to_string(),
            total_received: row.0,
            tip_count: row.1,
            average_tip: avg_tip,
            last_tip: row.2,
        })
    }

    /// Cache frequently accessed metrics
    pub async fn cache_daily_metrics(&self, date: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO metrics_cache (date, data, created_at) 
             SELECT $1, row_to_json(t), NOW() FROM (
                SELECT 
                    COUNT(*) as tip_count,
                    COALESCE(SUM(amount), 0) as total_volume,
                    COUNT(DISTINCT sender) as unique_tippers
                FROM tips 
                WHERE DATE(timestamp) = $1
             ) t
             ON CONFLICT (date) DO UPDATE SET data = EXCLUDED.data, created_at = NOW()",
        )
        .bind(date)
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
