use chrono::{DateTime, Utc};
/// Rate limiter using an in-memory sliding window per address.
use std::collections::HashMap;
use std::sync::Mutex;

pub struct RateLimiter {
    /// max transactions per window
    max_tx: usize,
    /// window duration in seconds
    window_secs: i64,
    /// address -> list of timestamps within the current window
    windows: Mutex<HashMap<String, Vec<DateTime<Utc>>>>,
    /// blacklisted addresses
    blacklist: Mutex<Vec<String>>,
    /// whitelisted addresses (bypass rate limiting)
    whitelist: Mutex<Vec<String>>,
}

impl RateLimiter {
    pub fn new(max_tx: usize, window_secs: i64) -> Self {
        Self {
            max_tx,
            window_secs,
            windows: Mutex::new(HashMap::new()),
            blacklist: Mutex::new(Vec::new()),
            whitelist: Mutex::new(Vec::new()),
        }
    }

    /// Returns true if the address is allowed to proceed.
    pub fn check(&self, address: &str) -> bool {
        if self.is_blacklisted(address) {
            return false;
        }
        if self.is_whitelisted(address) {
            return true;
        }

        let now = Utc::now();
        let cutoff = now - chrono::Duration::seconds(self.window_secs);
        let mut windows = self.windows.lock().unwrap();
        let timestamps = windows.entry(address.to_string()).or_default();
        timestamps.retain(|t| *t > cutoff);

        if timestamps.len() >= self.max_tx {
            return false;
        }
        timestamps.push(now);
        true
    }

    pub fn add_to_blacklist(&self, address: &str) {
        let mut bl = self.blacklist.lock().unwrap();
        if !bl.contains(&address.to_string()) {
            bl.push(address.to_string());
        }
    }

    pub fn remove_from_blacklist(&self, address: &str) {
        let mut bl = self.blacklist.lock().unwrap();
        bl.retain(|a| a != address);
    }

    pub fn add_to_whitelist(&self, address: &str) {
        let mut wl = self.whitelist.lock().unwrap();
        if !wl.contains(&address.to_string()) {
            wl.push(address.to_string());
        }
    }

    pub fn is_blacklisted(&self, address: &str) -> bool {
        self.blacklist
            .lock()
            .unwrap()
            .contains(&address.to_string())
    }

    pub fn is_whitelisted(&self, address: &str) -> bool {
        self.whitelist
            .lock()
            .unwrap()
            .contains(&address.to_string())
    }
}
