/// Export functionality for metrics
use csv::Writer;
use serde_json::json;
use sqlx::PgPool;
use std::io::Write;

pub struct MetricsExporter {
    db: PgPool,
}

impl MetricsExporter {
    /// Create new exporter
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    /// Export metrics to JSON
    pub async fn export_json(&self, start_date: &str, end_date: &str) -> Result<String, Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, (String, String, i64, String)>(
            "SELECT DATE(timestamp) as date, creator, SUM(amount) as total, token 
             FROM tips 
             WHERE DATE(timestamp) BETWEEN $1 AND $2
             GROUP BY DATE(timestamp), creator, token
             ORDER BY date DESC",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.db)
        .await?;

        let data: Vec<_> = rows
            .iter()
            .map(|(date, creator, total, token)| {
                json!({
                    "date": date,
                    "creator": creator,
                    "total": total,
                    "token": token
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&data)?)
    }

    /// Export metrics to CSV
    pub async fn export_csv(
        &self,
        start_date: &str,
        end_date: &str,
        writer: &mut dyn Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, (String, String, String, i64, String)>(
            "SELECT DATE(timestamp) as date, sender, creator, amount, token 
             FROM tips 
             WHERE DATE(timestamp) BETWEEN $1 AND $2
             ORDER BY timestamp DESC",
        )
        .bind(start_date)
        .bind(end_date)
        .fetch_all(&self.db)
        .await?;

        let mut csv_writer = Writer::from_writer(writer);

        csv_writer.write_record(&["date", "sender", "creator", "amount", "token"])?;

        for (date, sender, creator, amount, token) in rows {
            csv_writer.write_record(&[date, sender, creator, amount.to_string(), token])?;
        }

        csv_writer.flush()?;
        Ok(())
    }

    /// Export top creators
    pub async fn export_top_creators(
        &self,
        limit: i64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let rows = sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT creator, SUM(amount) as total, COUNT(*) as tip_count 
             FROM tips 
             GROUP BY creator 
             ORDER BY total DESC 
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.db)
        .await?;

        let data: Vec<_> = rows
            .iter()
            .map(|(creator, total, count)| {
                json!({
                    "creator": creator,
                    "total_received": total,
                    "tip_count": count
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&data)?)
    }
}
