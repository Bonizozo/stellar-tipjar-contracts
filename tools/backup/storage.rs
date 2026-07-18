//! Backup storage backends: local filesystem, S3, and IPFS.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Upload `data` to the configured backend and return a storage URI.
pub async fn upload(data: &[u8], backup_id: &str, backend: &StorageBackend) -> Result<String> {
    match backend {
        StorageBackend::Local(dir) => upload_local(data, backup_id, dir),
        StorageBackend::S3(cfg) => upload_s3(data, backup_id, cfg).await,
        StorageBackend::Ipfs(cfg) => upload_ipfs(data, backup_id, cfg).await,
    }
}

/// Download a backup by its storage URI and return the raw bytes.
pub async fn download(uri: &str, backend: &StorageBackend) -> Result<Vec<u8>> {
    match backend {
        StorageBackend::Local(_) => download_local(uri),
        StorageBackend::S3(cfg) => download_s3(uri, cfg).await,
        StorageBackend::Ipfs(cfg) => download_ipfs(uri, cfg).await,
    }
}

pub enum StorageBackend {
    Local(PathBuf),
    S3(S3Config),
    Ipfs(IpfsConfig),
}

pub struct S3Config {
    pub bucket: String,
    pub region: String,
    pub prefix: String,
    /// AWS credentials sourced from environment (AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY).
}

pub struct IpfsConfig {
    /// IPFS HTTP API endpoint, e.g. "http://localhost:5001" or an Infura gateway.
    pub api_url: String,
}

// ── Local ────────────────────────────────────────────────────────────────────

fn upload_local(data: &[u8], backup_id: &str, dir: &Path) -> Result<String> {
    std::fs::create_dir_all(dir).context("Failed to create backup directory")?;
    let path = dir.join(format!("{}.json", backup_id));
    std::fs::write(&path, data).context("Failed to write local backup")?;
    Ok(path.display().to_string())
}

fn download_local(uri: &str) -> Result<Vec<u8>> {
    std::fs::read(uri).with_context(|| format!("Failed to read local backup: {}", uri))
}

// ── S3 ───────────────────────────────────────────────────────────────────────

async fn upload_s3(data: &[u8], backup_id: &str, cfg: &S3Config) -> Result<String> {
    // Uses the AWS SDK via HTTP PUT with pre-signed URL or direct API.
    // For simplicity we use reqwest with AWS Signature V4 headers.
    // In production, replace with the `aws-sdk-s3` crate.
    let key = format!("{}/{}.json", cfg.prefix, backup_id);
    let url = format!(
        "https://{}.s3.{}.amazonaws.com/{}",
        cfg.bucket, cfg.region, key
    );

    let client = reqwest::Client::new();
    let access_key = std::env::var("AWS_ACCESS_KEY_ID").context("AWS_ACCESS_KEY_ID not set")?;
    let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY").context("AWS_SECRET_ACCESS_KEY not set")?;

    // NOTE: This is a simplified PUT without full SigV4 signing.
    // Replace with aws-sdk-s3 for production use.
    let resp = client
        .put(&url)
        .header("x-amz-access-key", &access_key)
        .header("x-amz-secret-key", &secret_key)
        .body(data.to_vec())
        .send()
        .await
        .context("S3 upload request failed")?;

    anyhow::ensure!(resp.status().is_success(), "S3 upload failed: {}", resp.status());
    Ok(format!("s3://{}/{}", cfg.bucket, key))
}

async fn download_s3(uri: &str, _cfg: &S3Config) -> Result<Vec<u8>> {
    // Strip s3:// prefix and fetch via HTTPS.
    let https_url = uri.replace("s3://", "https://s3.amazonaws.com/");
    let resp = reqwest::get(&https_url)
        .await
        .context("S3 download request failed")?;
    anyhow::ensure!(resp.status().is_success(), "S3 download failed: {}", resp.status());
    Ok(resp.bytes().await?.to_vec())
}

// ── IPFS ─────────────────────────────────────────────────────────────────────

async fn upload_ipfs(data: &[u8], _backup_id: &str, cfg: &IpfsConfig) -> Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/v0/add", cfg.api_url);

    let part = reqwest::multipart::Part::bytes(data.to_vec()).file_name("backup.json");
    let form = reqwest::multipart::Form::new().part("file", part);

    let resp: serde_json::Value = client
        .post(&url)
        .multipart(form)
        .send()
        .await
        .context("IPFS upload request failed")?
        .json()
        .await
        .context("Failed to parse IPFS response")?;

    let cid = resp["Hash"]
        .as_str()
        .context("IPFS response missing Hash field")?
        .to_string();

    Ok(format!("ipfs://{}", cid))
}

async fn download_ipfs(uri: &str, cfg: &IpfsConfig) -> Result<Vec<u8>> {
    let cid = uri.trim_start_matches("ipfs://");
    let url = format!("{}/api/v0/cat?arg={}", cfg.api_url, cid);
    let resp = reqwest::get(&url)
        .await
        .context("IPFS download request failed")?;
    anyhow::ensure!(resp.status().is_success(), "IPFS download failed: {}", resp.status());
    Ok(resp.bytes().await?.to_vec())
}
