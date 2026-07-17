//! Thin RPC client that wraps `stellar contract` CLI calls.
//!
//! All on-chain reads and writes go through this module so the rest of the
//! toolkit stays decoupled from the Stellar RPC wire format.
//!
//! In production this would use the Stellar XDR / Horizon SDK directly.
//! For portability the current implementation shells out to the `stellar`
//! CLI, which is always available in CI and developer environments.

use std::process::Command;
use std::thread;
use std::time::Duration;

use crate::types::{
    LeaderboardEntry, LockedTip, MatchingProgram, Milestone, Subscription,
    TipMetadata, TipRecord, TimeLock,
};

// ─────────────────────────────────────────────────────────────────────────────
// RpcClient
// ─────────────────────────────────────────────────────────────────────────────

pub struct RpcClient {
    rpc_url: String,
    max_retries: u32,
    retry_delay_ms: u64,
}

impl RpcClient {
    pub fn new(rpc_url: String, max_retries: u32, retry_delay_ms: u64) -> Self {
        Self {
            rpc_url,
            max_retries,
            retry_delay_ms,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Generic helpers
    // ─────────────────────────────────────────────────────────────────────────

    /// Runs a `stellar contract invoke` read call with retry logic.
    fn invoke_read(&self, contract_id: &str, function: &str, args: &[(&str, &str)]) -> Result<String, String> {
        let mut attempt = 0u32;
        loop {
            let mut cmd = Command::new("stellar");
            cmd.args(["contract", "invoke", "--id", contract_id, "--rpc-url", &self.rpc_url, "--"]);
            cmd.arg(function);
            for (k, v) in args {
                cmd.arg(format!("--{}", k));
                cmd.arg(v);
            }
            match cmd.output() {
                Ok(out) if out.status.success() => {
                    return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    if attempt >= self.max_retries {
                        return Err(format!("RPC call {}::{} failed: {}", contract_id, function, err));
                    }
                }
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(format!("RPC call {}::{} error: {}", contract_id, function, e));
                    }
                }
            }
            attempt += 1;
            thread::sleep(Duration::from_millis(self.retry_delay_ms));
        }
    }

    /// Runs a `stellar contract invoke` write call (requires source key).
    pub fn invoke_contract(
        &self,
        contract_id: &str,
        function: &str,
        args: &[(&str, &str)],
    ) -> Result<String, String> {
        // The admin secret key is read from the environment variable
        // MIGRATION_ADMIN_SECRET to avoid it appearing in process args.
        let secret = std::env::var("MIGRATION_ADMIN_SECRET")
            .map_err(|_| "MIGRATION_ADMIN_SECRET env var not set".to_string())?;

        let mut attempt = 0u32;
        loop {
            let mut cmd = Command::new("stellar");
            cmd.args([
                "contract", "invoke",
                "--id", contract_id,
                "--rpc-url", &self.rpc_url,
                "--source", &secret,
                "--",
            ]);
            cmd.arg(function);
            for (k, v) in args {
                cmd.arg(format!("--{}", k));
                cmd.arg(v);
            }
            match cmd.output() {
                Ok(out) if out.status.success() => {
                    return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
                }
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    if attempt >= self.max_retries {
                        return Err(format!("Write call {}::{} failed: {}", contract_id, function, err));
                    }
                }
                Err(e) => {
                    if attempt >= self.max_retries {
                        return Err(format!("Write call {}::{} error: {}", contract_id, function, e));
                    }
                }
            }
            attempt += 1;
            thread::sleep(Duration::from_millis(self.retry_delay_ms));
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Instance storage reads
    // ─────────────────────────────────────────────────────────────────────────

    pub fn get_instance_string(&self, contract_id: &str, key: &str) -> Result<String, String> {
        self.invoke_read(contract_id, &format!("get_{}", to_snake(key)), &[])
    }

    pub fn get_instance_string_opt(&self, contract_id: &str, key: &str) -> Option<String> {
        self.invoke_read(contract_id, &format!("get_{}", to_snake(key)), &[]).ok()
    }

    pub fn get_instance_u32(&self, contract_id: &str, key: &str) -> Option<u32> {
        self.invoke_read(contract_id, &format!("get_{}", to_snake(key)), &[])
            .ok()
            .and_then(|s| s.trim_matches('"').parse().ok())
    }

    pub fn get_instance_u64(&self, contract_id: &str, key: &str) -> Option<u64> {
        self.invoke_read(contract_id, &format!("get_{}", to_snake(key)), &[])
            .ok()
            .and_then(|s| s.trim_matches('"').parse().ok())
    }

    pub fn get_instance_bool(&self, contract_id: &str, key: &str) -> Option<bool> {
        self.invoke_read(contract_id, &format!("get_{}", to_snake(key)), &[])
            .ok()
            .and_then(|s| match s.trim_matches('"') {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            })
    }

    pub fn get_latest_ledger_sequence(&self) -> Result<u32, String> {
        let out = Command::new("stellar")
            .args(["ledger", "latest", "--rpc-url", &self.rpc_url])
            .output()
            .map_err(|e| format!("ledger latest error: {}", e))?;
        let text = String::from_utf8_lossy(&out.stdout);
        // Parse "sequence: 12345" from output.
        for line in text.lines() {
            if line.contains("sequence") {
                if let Some(n) = line.split_whitespace().last().and_then(|s| s.parse().ok()) {
                    return Ok(n);
                }
            }
        }
        Ok(0)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Persistent storage reads
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns all persistent keys matching a prefix as (key, value) pairs.
    /// Uses `stellar contract storage` to enumerate entries.
    pub fn scan_persistent_prefix(
        &self,
        contract_id: &str,
        prefix: &str,
    ) -> Result<Vec<(String, String)>, String> {
        let out = Command::new("stellar")
            .args([
                "contract", "storage",
                "--id", contract_id,
                "--rpc-url", &self.rpc_url,
                "--output", "json",
            ])
            .output()
            .map_err(|e| format!("storage scan error: {}", e))?;

        let text = String::from_utf8_lossy(&out.stdout);
        let entries: Vec<serde_json::Value> = serde_json::from_str(&text)
            .unwrap_or_default();

        let mut result = Vec::new();
        for entry in entries {
            if let (Some(key), Some(val)) = (
                entry.get("key").and_then(|v| v.as_str()),
                entry.get("value").and_then(|v| v.as_str()),
            ) {
                if key.starts_with(prefix) {
                    result.push((key.to_string(), val.to_string()));
                }
            }
        }
        Ok(result)
    }

    pub fn get_persistent_u64(&self, contract_id: &str, key: &str) -> Option<u64> {
        self.invoke_read(contract_id, "get_storage_u64", &[("key", key)])
            .ok()
            .and_then(|s| s.trim_matches('"').parse().ok())
    }

    pub fn get_persistent_string(&self, contract_id: &str, key: &str) -> Option<String> {
        self.invoke_read(contract_id, "get_storage_string", &[("key", key)]).ok()
    }

    pub fn get_persistent_tip_metadata(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<TipMetadata>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let meta: TipMetadata = serde_json::from_str(&raw)
            .map_err(|e| format!("TipMetadata parse error for key {}: {}", key, e))?;
        Ok(Some(meta))
    }

    pub fn get_persistent_leaderboard_entry(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<LeaderboardEntry>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let entry: LeaderboardEntry = serde_json::from_str(&raw)
            .map_err(|e| format!("LeaderboardEntry parse error for key {}: {}", key, e))?;
        Ok(Some(entry))
    }

    pub fn get_persistent_subscription(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<Subscription>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let sub: Subscription = serde_json::from_str(&raw)
            .map_err(|e| format!("Subscription parse error for key {}: {}", key, e))?;
        Ok(Some(sub))
    }

    pub fn get_persistent_time_lock(
        &self,
        contract_id: &str,
        key: &str,
        lock_id: u64,
    ) -> Result<Option<TimeLock>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        // The on-chain TimeLock doesn't store lock_id; we inject it here.
        let mut val: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| format!("TimeLock parse error for key {}: {}", key, e))?;
        if let serde_json::Value::Object(ref mut map) = val {
            map.insert("lock_id".into(), serde_json::Value::Number(lock_id.into()));
        }
        let lock: TimeLock = serde_json::from_value(val)
            .map_err(|e| format!("TimeLock deserialise error: {}", e))?;
        Ok(Some(lock))
    }

    pub fn get_persistent_milestone(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<Milestone>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let ms: Milestone = serde_json::from_str(&raw)
            .map_err(|e| format!("Milestone parse error for key {}: {}", key, e))?;
        Ok(Some(ms))
    }

    pub fn get_persistent_matching_program(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<MatchingProgram>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let prog: MatchingProgram = serde_json::from_str(&raw)
            .map_err(|e| format!("MatchingProgram parse error for key {}: {}", key, e))?;
        Ok(Some(prog))
    }

    pub fn get_persistent_tip_record(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Result<Option<TipRecord>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let rec: TipRecord = serde_json::from_str(&raw)
            .map_err(|e| format!("TipRecord parse error for key {}: {}", key, e))?;
        Ok(Some(rec))
    }

    pub fn get_persistent_locked_tip(
        &self,
        contract_id: &str,
        key: &str,
        tip_id: u64,
    ) -> Result<Option<LockedTip>, String> {
        let raw = match self.invoke_read(contract_id, "get_storage_json", &[("key", key)]) {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };
        if raw == "null" || raw.is_empty() {
            return Ok(None);
        }
        let mut val: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|e| format!("LockedTip parse error for key {}: {}", key, e))?;
        if let serde_json::Value::Object(ref mut map) = val {
            map.insert("tip_id".into(), serde_json::Value::Number(tip_id.into()));
        }
        let lt: LockedTip = serde_json::from_value(val)
            .map_err(|e| format!("LockedTip deserialise error: {}", e))?;
        Ok(Some(lt))
    }

    pub fn get_persistent_address_vec(
        &self,
        contract_id: &str,
        key: &str,
    ) -> Option<Vec<String>> {
        let raw = self
            .invoke_read(contract_id, "get_storage_json", &[("key", key)])
            .ok()?;
        serde_json::from_str(&raw).ok()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Balance / total reads (used by verify)
    // ─────────────────────────────────────────────────────────────────────────

    pub fn get_creator_balance(&self, contract_id: &str, creator: &str, token: &str) -> Option<i128> {
        self.invoke_read(
            contract_id,
            "get_withdrawable_balance",
            &[("creator", creator), ("token", token)],
        )
        .ok()
        .and_then(|s| s.trim_matches('"').parse().ok())
    }

    pub fn get_creator_total(&self, contract_id: &str, creator: &str, token: &str) -> Option<i128> {
        self.invoke_read(
            contract_id,
            "get_total_tips",
            &[("creator", creator), ("token", token)],
        )
        .ok()
        .and_then(|s| s.trim_matches('"').parse().ok())
    }

    pub fn get_whitelisted_tokens(&self, contract_id: &str) -> Result<Vec<String>, String> {
        let raw = self.invoke_read(contract_id, "get_whitelisted_tokens", &[])?;
        serde_json::from_str(&raw).map_err(|e| format!("Token list parse error: {}", e))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Write helpers (used by import)
    // ─────────────────────────────────────────────────────────────────────────

    pub fn set_instance_value(&self, contract_id: &str, key: &str, value: &str) -> Result<(), String> {
        self.invoke_contract(
            contract_id,
            "admin_set_instance",
            &[("key", key), ("value", value)],
        )?;
        Ok(())
    }

    pub fn whitelist_token(&self, contract_id: &str, token: &str) -> Result<(), String> {
        let admin = std::env::var("MIGRATION_ADMIN_ADDRESS")
            .map_err(|_| "MIGRATION_ADMIN_ADDRESS env var not set".to_string())?;
        self.invoke_contract(contract_id, "add_token", &[("admin", &admin), ("token", token)])?;
        Ok(())
    }

    pub fn set_creator_balance(
        &self,
        contract_id: &str,
        creator: &str,
        token: &str,
        balance: i128,
    ) -> Result<(), String> {
        self.invoke_contract(
            contract_id,
            "admin_set_creator_balance",
            &[
                ("creator", creator),
                ("token", token),
                ("balance", &balance.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn set_creator_total(
        &self,
        contract_id: &str,
        creator: &str,
        token: &str,
        total: i128,
    ) -> Result<(), String> {
        self.invoke_contract(
            contract_id,
            "admin_set_creator_total",
            &[
                ("creator", creator),
                ("token", token),
                ("total", &total.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn set_tip_count(&self, contract_id: &str, creator: &str, count: u64) -> Result<(), String> {
        self.invoke_contract(
            contract_id,
            "admin_set_tip_count",
            &[("creator", creator), ("count", &count.to_string())],
        )?;
        Ok(())
    }

    pub fn set_tip_history_entry(
        &self,
        contract_id: &str,
        creator: &str,
        idx: u64,
        meta: &TipMetadata,
    ) -> Result<(), String> {
        let json = serde_json::to_string(meta)
            .map_err(|e| format!("TipMetadata serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_tip_history",
            &[
                ("creator", creator),
                ("index", &idx.to_string()),
                ("metadata", &json),
            ],
        )?;
        Ok(())
    }

    pub fn set_tipper_aggregate(
        &self,
        contract_id: &str,
        address: &str,
        bucket: u32,
        entry: &LeaderboardEntry,
    ) -> Result<(), String> {
        let json = serde_json::to_string(entry)
            .map_err(|e| format!("LeaderboardEntry serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_tipper_aggregate",
            &[
                ("address", address),
                ("bucket", &bucket.to_string()),
                ("entry", &json),
            ],
        )?;
        Ok(())
    }

    pub fn set_creator_aggregate(
        &self,
        contract_id: &str,
        address: &str,
        bucket: u32,
        entry: &LeaderboardEntry,
    ) -> Result<(), String> {
        let json = serde_json::to_string(entry)
            .map_err(|e| format!("LeaderboardEntry serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_creator_aggregate",
            &[
                ("address", address),
                ("bucket", &bucket.to_string()),
                ("entry", &json),
            ],
        )?;
        Ok(())
    }

    pub fn set_subscription(&self, contract_id: &str, sub: &Subscription) -> Result<(), String> {
        let json = serde_json::to_string(sub)
            .map_err(|e| format!("Subscription serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_subscription",
            &[("subscription", &json)],
        )?;
        Ok(())
    }

    pub fn set_time_lock(&self, contract_id: &str, lock: &TimeLock) -> Result<(), String> {
        let json = serde_json::to_string(lock)
            .map_err(|e| format!("TimeLock serialise error: {}", e))?;
        self.invoke_contract(contract_id, "admin_set_time_lock", &[("lock", &json)])?;
        Ok(())
    }

    pub fn set_milestone(&self, contract_id: &str, creator: &str, ms: &Milestone) -> Result<(), String> {
        let json = serde_json::to_string(ms)
            .map_err(|e| format!("Milestone serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_milestone",
            &[("creator", creator), ("milestone", &json)],
        )?;
        Ok(())
    }

    pub fn set_matching_program(&self, contract_id: &str, prog: &MatchingProgram) -> Result<(), String> {
        let json = serde_json::to_string(prog)
            .map_err(|e| format!("MatchingProgram serialise error: {}", e))?;
        self.invoke_contract(
            contract_id,
            "admin_set_matching_program",
            &[("program", &json)],
        )?;
        Ok(())
    }

    pub fn set_tip_record(&self, contract_id: &str, rec: &TipRecord) -> Result<(), String> {
        let json = serde_json::to_string(rec)
            .map_err(|e| format!("TipRecord serialise error: {}", e))?;
        self.invoke_contract(contract_id, "admin_set_tip_record", &[("record", &json)])?;
        Ok(())
    }

    pub fn set_locked_tip(&self, contract_id: &str, lt: &LockedTip) -> Result<(), String> {
        let json = serde_json::to_string(lt)
            .map_err(|e| format!("LockedTip serialise error: {}", e))?;
        self.invoke_contract(contract_id, "admin_set_locked_tip", &[("locked_tip", &json)])?;
        Ok(())
    }

    pub fn set_user_role(&self, contract_id: &str, address: &str, role: &str) -> Result<(), String> {
        let admin = std::env::var("MIGRATION_ADMIN_ADDRESS")
            .map_err(|_| "MIGRATION_ADMIN_ADDRESS env var not set".to_string())?;
        self.invoke_contract(
            contract_id,
            "grant_role",
            &[("admin", &admin), ("user", address), ("role", role)],
        )?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Converts a PascalCase key like "FeeBasisPoints" to "fee_basis_points".
fn to_snake(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_lowercase().next().unwrap());
    }
    out
}
