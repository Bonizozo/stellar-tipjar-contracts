/// Alert system — logs security events and (optionally) sends webhook notifications.
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum Alert {
    RateLimited {
        address: String,
    },
    AnomalyDetected {
        tx_hash: String,
        sender: String,
        score: f64,
    },
    Blacklisted {
        address: String,
    },
    CircuitBreakerTripped {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityEvent {
    pub id: String,
    pub timestamp: String,
    pub alert: Alert,
}

pub struct AlertingService {
    /// Optional webhook URL for external notifications
    webhook_url: Option<String>,
}

impl AlertingService {
    pub fn new(webhook_url: Option<String>) -> Self {
        Self { webhook_url }
    }

    /// Record and dispatch an alert.
    pub async fn send_alert(&self, alert: Alert) {
        let event = SecurityEvent {
            id: Uuid::new_v4().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            alert: alert.clone(),
        };

        // Always log to structured tracing output
        match &alert {
            Alert::CircuitBreakerTripped { .. } => error!(event = ?event, "SECURITY ALERT"),
            _ => warn!(event = ?event, "SECURITY ALERT"),
        }

        // Fire-and-forget webhook if configured
        if let Some(url) = &self.webhook_url {
            let url = url.clone();
            let payload = serde_json::to_string(&event).unwrap_or_default();
            tokio::spawn(async move {
                if let Err(e) = post_json(&url, &payload).await {
                    warn!("Webhook delivery failed: {e}");
                }
            });
        }
    }
}

/// Minimal HTTP POST using raw tokio TCP (avoids extra HTTP client dependency).
async fn post_json(url: &str, body: &str) -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    // Parse "http://host:port/path" manually
    let without_scheme = url
        .strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    let (host_port, path) = without_scheme
        .split_once('/')
        .map(|(h, p)| (h, format!("/{p}")))
        .unwrap_or((without_scheme, "/".to_string()));
    let (host, port) = host_port
        .split_once(':')
        .map(|(h, p)| (h, p.parse::<u16>().unwrap_or(80)))
        .unwrap_or((host_port, 80));

    let mut stream = TcpStream::connect(format!("{host}:{port}")).await?;
    let request = format!(
        "POST {path} HTTP/1.0\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await?;
    Ok(())
}
