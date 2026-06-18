/// Enhanced Circuit Breaker System
///
/// This module provides sophisticated automated protection against extreme market volatility,
/// anomalous trading patterns, and potential attacks within the Stellar tipjar contracts.
use soroban_sdk::{contracterror, contracttype, symbol_short};

/// Enhanced circuit breaker configuration with comprehensive protection parameters
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedCircuitBreakerConfig {
    // Single tip thresholds
    /// Maximum amount for a single tip before triggering immediate halt
    pub max_single_tip: i128,

    // Volume thresholds for different time windows
    /// Maximum volume allowed in a 1-minute sliding window
    pub one_minute_threshold: i128,
    /// Maximum volume allowed in a 5-minute sliding window
    pub five_minute_threshold: i128,
    /// Maximum volume allowed in a 1-hour sliding window
    pub one_hour_threshold: i128,
    /// Maximum volume allowed in a 24-hour sliding window
    pub twenty_four_hour_threshold: i128,

    // Rate limiting parameters
    /// Maximum number of tips allowed per minute globally
    pub max_tips_per_minute: u32,
    /// Maximum number of tips per creator per minute
    pub max_tips_per_creator_per_min: u32,

    // Anomaly detection parameters
    /// Whether advanced anomaly detection is enabled
    pub anomaly_detection_enabled: bool,
    /// Confidence threshold in basis points (e.g., 7500 = 75%)
    pub anomaly_confidence_threshold: u32,
    /// Whether pattern analysis for coordinated attacks is enabled
    pub pattern_analysis_enabled: bool,

    // Cooldown configuration
    /// Base cooldown duration in seconds for standard triggers
    pub base_cooldown_seconds: u64,
    /// Maximum cooldown duration in seconds to prevent indefinite halts
    pub max_cooldown_seconds: u64,
    /// Whether exponential backoff is enabled for repeated triggers
    pub exponential_backoff_enabled: bool,
    /// Backoff multiplier in basis points (e.g., 15000 = 1.5x multiplier)
    pub backoff_multiplier: u32,

    // System control flags
    /// Whether the circuit breaker system is enabled
    pub enabled: bool,
    /// Emergency mode requiring multiple admin signatures
    pub emergency_mode: bool,
    /// Maintenance mode preventing all operations
    pub maintenance_mode: bool,
}

/// Volume monitoring thresholds across different time windows
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeThresholds {
    /// Maximum volume allowed in a 1-minute sliding window
    pub one_minute_threshold: i128,
    /// Maximum volume allowed in a 5-minute sliding window
    pub five_minute_threshold: i128,
    /// Maximum volume allowed in a 1-hour sliding window
    pub one_hour_threshold: i128,
    /// Maximum volume allowed in a 24-hour sliding window
    pub twenty_four_hour_threshold: i128,
    /// Whether percentage-based thresholds relative to historical averages are enabled
    pub percentage_based_enabled: bool,
    /// Multiplier in basis points above historical average (e.g., 15000 = 150% above average)
    pub historical_multiplier: u32,
}

/// Comprehensive cooldown period management configuration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CooldownConfig {
    /// Base cooldown duration in seconds for standard triggers
    pub base_cooldown_seconds: u64,
    /// Maximum cooldown duration in seconds to prevent indefinite halts
    pub max_cooldown_seconds: u64,
    /// Whether exponential backoff is enabled for repeated triggers
    pub exponential_backoff_enabled: bool,
    /// Backoff multiplier in basis points (e.g., 15000 = 1.5x multiplier)
    pub backoff_multiplier: u32,
}

/// Circuit breaker specific error conditions
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CircuitBreakerError {
    /// Configuration validation failed
    InvalidConfiguration = 200,
    /// Single tip threshold must be positive
    InvalidSingleTipThreshold = 201,
    /// Volume threshold must be positive
    InvalidVolumeThreshold = 202,
    /// Cooldown duration must be positive
    InvalidCooldownDuration = 203,
    /// Rate limit must be positive
    InvalidRateLimit = 204,
    /// Confidence threshold must be between 0 and 10000
    InvalidConfidenceThreshold = 205,
    /// Backoff multiplier must be greater than 10000 (100%)
    InvalidBackoffMultiplier = 206,
}

impl EnhancedCircuitBreakerConfig {
    /// Validates all configuration parameters for correctness and consistency
    ///
    /// Returns Ok(()) if configuration is valid, or Err with specific error code
    pub fn validate(&self) -> Result<(), CircuitBreakerError> {
        // Validate single tip threshold
        if self.max_single_tip <= 0 {
            return Err(CircuitBreakerError::InvalidSingleTipThreshold);
        }

        // Validate volume thresholds
        if self.one_minute_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.five_minute_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.one_hour_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.twenty_four_hour_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }

        // Validate logical ordering (longer windows should have higher thresholds)
        if self.five_minute_threshold < self.one_minute_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.one_hour_threshold < self.five_minute_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.twenty_four_hour_threshold < self.one_hour_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }

        // Validate rate limits
        if self.max_tips_per_minute == 0 {
            return Err(CircuitBreakerError::InvalidRateLimit);
        }
        if self.max_tips_per_creator_per_min == 0 {
            return Err(CircuitBreakerError::InvalidRateLimit);
        }

        // Validate anomaly detection parameters
        if self.anomaly_confidence_threshold > 10000 {
            return Err(CircuitBreakerError::InvalidConfidenceThreshold);
        }

        // Validate cooldown configuration
        if self.base_cooldown_seconds == 0 {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }
        if self.max_cooldown_seconds == 0 {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }
        if self.max_cooldown_seconds < self.base_cooldown_seconds {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }

        if self.exponential_backoff_enabled && self.backoff_multiplier <= 10000 {
            return Err(CircuitBreakerError::InvalidBackoffMultiplier);
        }

        Ok(())
    }

    /// Creates a default configuration with conservative settings
    pub fn default_config() -> Self {
        Self {
            max_single_tip: 1_000_000_000,         // 1 billion stroops (100 XLM)
            one_minute_threshold: 10_000_000_000,  // 1,000 XLM per minute
            five_minute_threshold: 40_000_000_000, // 4,000 XLM per 5 minutes
            one_hour_threshold: 200_000_000_000,   // 20,000 XLM per hour
            twenty_four_hour_threshold: 2_000_000_000_000, // 200,000 XLM per day
            max_tips_per_minute: 1000,
            max_tips_per_creator_per_min: 100,
            anomaly_detection_enabled: true,
            anomaly_confidence_threshold: 7500, // 75%
            pattern_analysis_enabled: true,
            base_cooldown_seconds: 300,  // 5 minutes
            max_cooldown_seconds: 86400, // 24 hours
            exponential_backoff_enabled: true,
            backoff_multiplier: 15000, // 1.5x multiplier
            enabled: true,
            emergency_mode: false,
            maintenance_mode: false,
        }
    }
}

impl VolumeThresholds {
    /// Validates volume threshold configuration
    pub fn validate(&self) -> Result<(), CircuitBreakerError> {
        if self.one_minute_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.five_minute_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.one_hour_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.twenty_four_hour_threshold <= 0 {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }

        // Validate logical ordering (longer windows should have higher thresholds)
        if self.five_minute_threshold < self.one_minute_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.one_hour_threshold < self.five_minute_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }
        if self.twenty_four_hour_threshold < self.one_hour_threshold {
            return Err(CircuitBreakerError::InvalidVolumeThreshold);
        }

        Ok(())
    }

    /// Creates default volume thresholds with conservative limits
    pub fn default_config() -> Self {
        Self {
            one_minute_threshold: 10_000_000_000,  // 1,000 XLM per minute
            five_minute_threshold: 40_000_000_000, // 4,000 XLM per 5 minutes
            one_hour_threshold: 200_000_000_000,   // 20,000 XLM per hour
            twenty_four_hour_threshold: 2_000_000_000_000, // 200,000 XLM per day
            percentage_based_enabled: true,
            historical_multiplier: 20000, // 200% above historical average
        }
    }
}

impl CooldownConfig {
    /// Validates cooldown configuration parameters
    pub fn validate(&self) -> Result<(), CircuitBreakerError> {
        if self.base_cooldown_seconds == 0 {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }
        if self.max_cooldown_seconds == 0 {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }
        if self.max_cooldown_seconds < self.base_cooldown_seconds {
            return Err(CircuitBreakerError::InvalidCooldownDuration);
        }

        if self.exponential_backoff_enabled && self.backoff_multiplier <= 10000 {
            return Err(CircuitBreakerError::InvalidBackoffMultiplier);
        }

        Ok(())
    }

    /// Creates default cooldown configuration
    pub fn default_config() -> Self {
        Self {
            base_cooldown_seconds: 300,  // 5 minutes
            max_cooldown_seconds: 86400, // 24 hours
            exponential_backoff_enabled: true,
            backoff_multiplier: 15000, // 1.5x multiplier
        }
    }
}

/// Time window enumeration for volume tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum TimeWindow {
    OneMinute,
    FiveMinutes,
    OneHour,
    TwentyFourHours,
}

/// Volume tracking data for a specific time window
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VolumeWindow {
    /// Timestamp when this window started
    pub window_start: u64,
    /// Cumulative volume in this window
    pub current_volume: i128,
    /// Number of tips in this window
    pub tip_count: u32,
    /// Number of unique senders in this window
    pub unique_senders: u32,
    /// Maximum single tip amount in this window
    pub max_single_tip: i128,
}

/// Rate limiting state for tracking tip frequency
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RateLimitState {
    /// Start of the current minute window
    pub current_minute_start: u64,
    /// Number of tips in the current minute
    pub tips_this_minute: u32,
}

/// Historical statistics for anomaly detection
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistoricalStats {
    /// Average tip amount over historical period
    pub average_tip_amount: i128,
    /// Standard deviation of tip amounts
    pub standard_deviation: i128,
    /// Average number of tips per minute
    pub average_tips_per_minute: u32,
    /// Average number of unique senders per hour
    pub unique_senders_per_hour: u32,
    /// Timestamp of last statistics update
    pub last_update: u64,
    /// Number of samples used for statistics
    pub sample_count: u32,
}

/// Anomaly detection state for pattern recognition
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnomalyDetectionState {
    /// Historical statistics for comparison
    pub historical_stats: HistoricalStats,
    /// Sender clustering anomaly score (basis points)
    pub sender_clustering_score: u32,
    /// Velocity anomaly score (basis points)
    pub velocity_anomaly_score: u32,
    /// Amount deviation anomaly score (basis points)
    pub amount_deviation_score: u32,
    /// Overall confidence score (basis points)
    pub overall_confidence: u32,
    /// Timestamp of last anomaly analysis
    pub last_analysis_time: u64,
}

/// Trigger type enumeration
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TriggerType {
    None,
    SingleTipSpike,
    VolumeSpike(TimeWindow),
    RateLimit,
    AnomalyDetection,
    PatternAnalysis,
    PriceVolatility,
    Manual,
}

/// Trigger severity levels
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq, Copy)]
pub enum TriggerSeverity {
    None,
    Low,
    Medium,
    High,
    Critical,
}

/// Optional wrapper for AnomalyDetectionState (contracttype-compatible replacement for Option<AnomalyDetectionState>)
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptAnomalyDetectionState {
    Absent,
    Present(AnomalyDetectionState),
}

/// Record of a circuit breaker trigger event
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerEvent {
    /// Unique trigger identifier
    pub trigger_id: u64,
    /// Type of trigger that occurred
    pub trigger_type: TriggerType,
    /// Severity level of the trigger
    pub severity: TriggerSeverity,
    /// Timestamp when trigger occurred
    pub timestamp: u64,
    /// Cooldown duration applied (seconds)
    pub cooldown_duration: u64,
}

/// Enhanced circuit breaker state with comprehensive tracking
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnhancedCircuitBreakerState {
    /// Timestamp until which operations are halted (0 if not halted)
    pub halted_until: u64,
    /// Reason for current halt (if any)
    pub halt_reason: TriggerType,
    /// Severity of current halt (if any)
    pub halt_severity: TriggerSeverity,

    /// Volume tracking for 1-minute window
    pub one_minute_window: VolumeWindow,
    /// Volume tracking for 5-minute window
    pub five_minute_window: VolumeWindow,
    /// Volume tracking for 1-hour window
    pub one_hour_window: VolumeWindow,
    /// Volume tracking for 24-hour window
    pub twenty_four_hour_window: VolumeWindow,

    /// Rate limiting state
    pub rate_limit_state: RateLimitState,

    /// Total number of times circuit breaker has been triggered
    pub trigger_count: u32,
    /// Timestamp of last trigger
    pub last_trigger_time: u64,

    /// Anomaly detection state (optional, only if enabled)
    pub anomaly_state: OptAnomalyDetectionState,

    /// Total number of halts
    pub total_halts: u32,
    /// Total duration of all halts (seconds)
    pub total_halt_duration: u64,
}

impl EnhancedCircuitBreakerState {
    /// Creates a new state with default values
    pub fn new(timestamp: u64) -> Self {
        Self {
            halted_until: 0,
            halt_reason: TriggerType::None,
            halt_severity: TriggerSeverity::None,
            one_minute_window: VolumeWindow {
                window_start: timestamp,
                current_volume: 0,
                tip_count: 0,
                unique_senders: 0,
                max_single_tip: 0,
            },
            five_minute_window: VolumeWindow {
                window_start: timestamp,
                current_volume: 0,
                tip_count: 0,
                unique_senders: 0,
                max_single_tip: 0,
            },
            one_hour_window: VolumeWindow {
                window_start: timestamp,
                current_volume: 0,
                tip_count: 0,
                unique_senders: 0,
                max_single_tip: 0,
            },
            twenty_four_hour_window: VolumeWindow {
                window_start: timestamp,
                current_volume: 0,
                tip_count: 0,
                unique_senders: 0,
                max_single_tip: 0,
            },
            rate_limit_state: RateLimitState {
                current_minute_start: timestamp,
                tips_this_minute: 0,
            },
            trigger_count: 0,
            last_trigger_time: 0,
            anomaly_state: OptAnomalyDetectionState::Absent,
            total_halts: 0,
            total_halt_duration: 0,
        }
    }

    /// Checks if the circuit breaker is currently halted
    pub fn is_halted(&self, current_time: u64) -> bool {
        self.halted_until > current_time
    }

    /// Gets remaining cooldown time in seconds
    pub fn remaining_cooldown(&self, current_time: u64) -> u64 {
        self.halted_until.saturating_sub(current_time)
    }
}

/// Circuit breaker trigger engine for detecting anomalies and enforcing halts
pub mod trigger_engine {
    use super::*;
    use soroban_sdk::{Address, Env};

    /// Checks if a single tip amount exceeds configured thresholds
    ///
    /// Returns Some(TriggerSeverity) if threshold is exceeded, None otherwise
    pub fn check_single_tip_spike(
        config: &EnhancedCircuitBreakerConfig,
        amount: i128,
        _creator: &Address,
        _token: &Address,
    ) -> Option<TriggerSeverity> {
        if !config.enabled {
            return None;
        }

        if amount > config.max_single_tip {
            // Single tip spike is always considered High severity
            Some(TriggerSeverity::High)
        } else {
            None
        }
    }

    /// Checks creator-specific limits if configured
    ///
    /// Returns Some(TriggerSeverity) if creator-specific limit is exceeded
    pub fn check_creator_specific_limit(
        _env: &Env,
        _creator: &Address,
        amount: i128,
        creator_limit: Option<i128>,
    ) -> Option<TriggerSeverity> {
        if let Some(limit) = creator_limit {
            if amount > limit {
                return Some(TriggerSeverity::Medium);
            }
        }
        None
    }

    /// Checks token-specific limits if configured
    ///
    /// Returns Some(TriggerSeverity) if token-specific limit is exceeded
    pub fn check_token_specific_limit(
        _env: &Env,
        _token: &Address,
        amount: i128,
        token_limit: Option<i128>,
    ) -> Option<TriggerSeverity> {
        if let Some(limit) = token_limit {
            if amount > limit {
                return Some(TriggerSeverity::Medium);
            }
        }
        None
    }

    /// Triggers an immediate halt for spike conditions
    ///
    /// Updates state with halt information and calculates cooldown period
    pub fn trigger_immediate_halt(
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        trigger_type: TriggerType,
        severity: TriggerSeverity,
        current_time: u64,
    ) {
        let cooldown = calculate_cooldown(config, severity, state.trigger_count);

        state.halted_until = current_time.saturating_add(cooldown);
        state.halt_reason = trigger_type;
        state.halt_severity = severity;
        state.trigger_count = state.trigger_count.saturating_add(1);
        state.last_trigger_time = current_time;
        state.total_halts = state.total_halts.saturating_add(1);
        state.total_halt_duration = state.total_halt_duration.saturating_add(cooldown);
    }

    /// Calculates cooldown period based on severity and trigger history
    pub fn calculate_cooldown(
        config: &EnhancedCircuitBreakerConfig,
        severity: TriggerSeverity,
        trigger_count: u32,
    ) -> u64 {
        let base_cooldown = match severity {
            TriggerSeverity::None | TriggerSeverity::Low => config.base_cooldown_seconds / 2,
            TriggerSeverity::Medium => config.base_cooldown_seconds,
            TriggerSeverity::High => config.base_cooldown_seconds * 2,
            TriggerSeverity::Critical => config.base_cooldown_seconds * 4,
        };

        let cooldown = if config.exponential_backoff_enabled && trigger_count > 0 {
            // Apply exponential backoff: cooldown * (multiplier ^ trigger_count)
            let multiplier = config.backoff_multiplier as u64;
            let mut adjusted = base_cooldown;

            for _ in 0..trigger_count.min(5) {
                // Cap at 5 iterations to prevent overflow
                adjusted = adjusted.saturating_mul(multiplier) / 10000;
            }

            adjusted
        } else {
            base_cooldown
        };

        // Ensure cooldown is within configured bounds
        cooldown
            .min(config.max_cooldown_seconds)
            .max(config.base_cooldown_seconds)
    }
}

/// Volume tracking and monitoring module
pub mod volume_tracker {
    use super::*;

    /// Updates volume window with new tip data
    pub fn update_volume_window(
        window: &mut VolumeWindow,
        amount: i128,
        current_time: u64,
        window_duration: u64,
    ) {
        // Check if we need to start a new window
        if current_time >= window.window_start.saturating_add(window_duration) {
            // Start new window
            window.window_start = current_time;
            window.current_volume = amount;
            window.tip_count = 1;
            window.unique_senders = 1;
            window.max_single_tip = amount;
        } else {
            // Update existing window
            window.current_volume = window.current_volume.saturating_add(amount);
            window.tip_count = window.tip_count.saturating_add(1);
            if amount > window.max_single_tip {
                window.max_single_tip = amount;
            }
        }
    }

    /// Checks if volume in a window exceeds threshold
    pub fn check_volume_threshold(
        window: &VolumeWindow,
        threshold: i128,
        current_time: u64,
        window_duration: u64,
    ) -> bool {
        // Only check if we're still in the current window
        if current_time < window.window_start.saturating_add(window_duration) {
            window.current_volume > threshold
        } else {
            false
        }
    }

    /// Updates all volume windows with new tip
    pub fn update_all_windows(
        state: &mut EnhancedCircuitBreakerState,
        amount: i128,
        current_time: u64,
    ) {
        // Update 1-minute window
        update_volume_window(&mut state.one_minute_window, amount, current_time, 60);

        // Update 5-minute window
        update_volume_window(&mut state.five_minute_window, amount, current_time, 300);

        // Update 1-hour window
        update_volume_window(&mut state.one_hour_window, amount, current_time, 3600);

        // Update 24-hour window
        update_volume_window(
            &mut state.twenty_four_hour_window,
            amount,
            current_time,
            86400,
        );
    }

    /// Checks all volume windows against configured thresholds
    ///
    /// Returns Some((TriggerType, TriggerSeverity)) if any threshold is exceeded
    pub fn check_all_volume_thresholds(
        state: &EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        current_time: u64,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        // Check 1-minute window (highest severity)
        if check_volume_threshold(
            &state.one_minute_window,
            config.one_minute_threshold,
            current_time,
            60,
        ) {
            return Some((
                TriggerType::VolumeSpike(TimeWindow::OneMinute),
                TriggerSeverity::Critical,
            ));
        }

        // Check 5-minute window
        if check_volume_threshold(
            &state.five_minute_window,
            config.five_minute_threshold,
            current_time,
            300,
        ) {
            return Some((
                TriggerType::VolumeSpike(TimeWindow::FiveMinutes),
                TriggerSeverity::High,
            ));
        }

        // Check 1-hour window
        if check_volume_threshold(
            &state.one_hour_window,
            config.one_hour_threshold,
            current_time,
            3600,
        ) {
            return Some((
                TriggerType::VolumeSpike(TimeWindow::OneHour),
                TriggerSeverity::Medium,
            ));
        }

        // Check 24-hour window
        if check_volume_threshold(
            &state.twenty_four_hour_window,
            config.twenty_four_hour_threshold,
            current_time,
            86400,
        ) {
            return Some((
                TriggerType::VolumeSpike(TimeWindow::TwentyFourHours),
                TriggerSeverity::Low,
            ));
        }

        None
    }

    /// Calculates percentage-based threshold relative to historical average
    pub fn calculate_percentage_threshold(historical_average: i128, multiplier_bps: u32) -> i128 {
        // multiplier_bps is in basis points (e.g., 20000 = 200% above average)
        historical_average.saturating_mul(multiplier_bps as i128) / 10000
    }

    /// Resets all volume counters (called after recovery)
    pub fn reset_volume_counters(state: &mut EnhancedCircuitBreakerState, current_time: u64) {
        state.one_minute_window = VolumeWindow {
            window_start: current_time,
            current_volume: 0,
            tip_count: 0,
            unique_senders: 0,
            max_single_tip: 0,
        };
        state.five_minute_window = VolumeWindow {
            window_start: current_time,
            current_volume: 0,
            tip_count: 0,
            unique_senders: 0,
            max_single_tip: 0,
        };
        state.one_hour_window = VolumeWindow {
            window_start: current_time,
            current_volume: 0,
            tip_count: 0,
            unique_senders: 0,
            max_single_tip: 0,
        };
        state.twenty_four_hour_window = VolumeWindow {
            window_start: current_time,
            current_volume: 0,
            tip_count: 0,
            unique_senders: 0,
            max_single_tip: 0,
        };
    }
}

/// Rate limiting module for tip frequency control
pub mod rate_limiter {
    use super::*;

    /// Updates rate limit state with new tip
    pub fn update_rate_limit_state(state: &mut RateLimitState, current_time: u64) {
        // Check if we need to start a new minute window
        if current_time >= state.current_minute_start.saturating_add(60) {
            // Start new minute
            state.current_minute_start = current_time;
            state.tips_this_minute = 1;
        } else {
            // Increment counter in current minute
            state.tips_this_minute = state.tips_this_minute.saturating_add(1);
        }
    }

    /// Checks if global rate limit is exceeded
    pub fn check_global_rate_limit(
        state: &RateLimitState,
        config: &EnhancedCircuitBreakerConfig,
        current_time: u64,
    ) -> bool {
        // Only check if we're still in the current minute
        if current_time < state.current_minute_start.saturating_add(60) {
            state.tips_this_minute > config.max_tips_per_minute
        } else {
            false
        }
    }

    /// Checks rate limit and returns trigger info if exceeded
    pub fn check_rate_limit_trigger(
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        current_time: u64,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        // Update rate limit state
        update_rate_limit_state(&mut state.rate_limit_state, current_time);

        // Check if global rate limit is exceeded
        if check_global_rate_limit(&state.rate_limit_state, config, current_time) {
            return Some((TriggerType::RateLimit, TriggerSeverity::Medium));
        }

        None
    }

    /// Resets rate limit counters (called after recovery)
    pub fn reset_rate_limit_state(state: &mut EnhancedCircuitBreakerState, current_time: u64) {
        state.rate_limit_state = RateLimitState {
            current_minute_start: current_time,
            tips_this_minute: 0,
        };
    }
}

/// Advanced anomaly detection module
pub mod anomaly_detector {
    use super::*;
    use soroban_sdk::Address;

    /// Calculates sender clustering score
    ///
    /// Detects if tips are coming from a small set of related addresses
    /// Returns score in basis points (0-10000)
    pub fn calculate_sender_clustering_score(
        recent_senders: &[Address],
        unique_threshold: u32,
    ) -> u32 {
        if recent_senders.is_empty() {
            return 0;
        }

        // Count unique senders
        let mut unique_count = 0;
        for i in 0..recent_senders.len() {
            let mut is_unique = true;
            for j in 0..i {
                if recent_senders[i] == recent_senders[j] {
                    is_unique = false;
                    break;
                }
            }
            if is_unique {
                unique_count += 1;
            }
        }

        // Calculate clustering score
        // High score if unique senders are below threshold
        if unique_count < unique_threshold {
            let ratio = (unique_threshold - unique_count) * 10000 / unique_threshold;
            ratio.min(10000)
        } else {
            0
        }
    }

    /// Calculates velocity anomaly score
    ///
    /// Detects unusual tip frequency patterns
    /// Returns score in basis points (0-10000)
    pub fn calculate_velocity_anomaly_score(
        current_rate: u32,
        historical_average: u32,
        deviation_threshold: u32,
    ) -> u32 {
        if historical_average == 0 {
            return 0;
        }

        // Calculate how much current rate exceeds historical average
        if current_rate > historical_average {
            let excess = current_rate - historical_average;
            let ratio = excess * 10000 / historical_average;

            // Score increases as we exceed the deviation threshold
            if ratio > deviation_threshold {
                ((ratio - deviation_threshold) * 10000 / deviation_threshold).min(10000)
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Calculates amount deviation score
    ///
    /// Detects tips with amounts that deviate significantly from historical patterns
    /// Returns score in basis points (0-10000)
    pub fn calculate_amount_deviation_score(
        amount: i128,
        historical_average: i128,
        standard_deviation: i128,
    ) -> u32 {
        if standard_deviation == 0 || historical_average == 0 {
            return 0;
        }

        // Calculate z-score (number of standard deviations from mean)
        let deviation = if amount > historical_average {
            amount - historical_average
        } else {
            historical_average - amount
        };

        let z_score = deviation * 100 / standard_deviation;

        // Convert z-score to anomaly score
        // z > 2 (2 std devs) starts to be anomalous
        // z > 3 (3 std devs) is highly anomalous
        if z_score > 300 {
            10000 // Maximum anomaly
        } else if z_score > 200 {
            (((z_score - 200) * 10000 / 100).min(10000)) as u32
        } else {
            0
        }
    }

    /// Updates anomaly detection state with new tip data
    pub fn update_anomaly_state(
        state: &mut OptAnomalyDetectionState,
        amount: i128,
        current_time: u64,
    ) {
        if let OptAnomalyDetectionState::Present(anomaly_state) = state {
            // Update historical statistics
            let stats = &mut anomaly_state.historical_stats;

            // Simple moving average update
            let new_sample_count = stats.sample_count.saturating_add(1);
            let total = stats
                .average_tip_amount
                .saturating_mul(stats.sample_count as i128);
            stats.average_tip_amount = total.saturating_add(amount) / new_sample_count as i128;
            stats.sample_count = new_sample_count;
            stats.last_update = current_time;

            // Calculate amount deviation score
            anomaly_state.amount_deviation_score = calculate_amount_deviation_score(
                amount,
                stats.average_tip_amount,
                stats.standard_deviation,
            );

            anomaly_state.last_analysis_time = current_time;
        }
    }

    /// Calculates overall confidence score from individual anomaly scores
    pub fn calculate_overall_confidence(
        sender_clustering: u32,
        velocity_anomaly: u32,
        amount_deviation: u32,
    ) -> u32 {
        // Weighted average of anomaly scores
        // Sender clustering: 40%, Velocity: 30%, Amount: 30%
        let weighted_sum = sender_clustering
            .saturating_mul(40)
            .saturating_add(velocity_anomaly.saturating_mul(30))
            .saturating_add(amount_deviation.saturating_mul(30));

        weighted_sum / 100
    }

    /// Checks if anomaly detection should trigger circuit breaker
    pub fn check_anomaly_trigger(
        state: &OptAnomalyDetectionState,
        config: &EnhancedCircuitBreakerConfig,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        if !config.anomaly_detection_enabled {
            return None;
        }

        if let OptAnomalyDetectionState::Present(anomaly_state) = state {
            if anomaly_state.overall_confidence > config.anomaly_confidence_threshold {
                // Determine severity based on confidence level
                let severity = if anomaly_state.overall_confidence > 9000 {
                    TriggerSeverity::Critical
                } else if anomaly_state.overall_confidence > 8000 {
                    TriggerSeverity::High
                } else {
                    TriggerSeverity::Medium
                };

                return Some((TriggerType::AnomalyDetection, severity));
            }
        }

        None
    }

    /// Initializes anomaly detection state
    pub fn initialize_anomaly_state(current_time: u64) -> AnomalyDetectionState {
        AnomalyDetectionState {
            historical_stats: HistoricalStats {
                average_tip_amount: 0,
                standard_deviation: 0,
                average_tips_per_minute: 0,
                unique_senders_per_hour: 0,
                last_update: current_time,
                sample_count: 0,
            },
            sender_clustering_score: 0,
            velocity_anomaly_score: 0,
            amount_deviation_score: 0,
            overall_confidence: 0,
            last_analysis_time: current_time,
        }
    }
}

/// Rolling statistics module for historical pattern tracking
pub mod statistics {
    use super::*;

    /// Updates rolling statistics with new tip data
    pub fn update_rolling_statistics(stats: &mut HistoricalStats, amount: i128, current_time: u64) {
        let old_count = stats.sample_count;
        let new_count = old_count.saturating_add(1);

        // Update average using incremental formula
        let old_avg = stats.average_tip_amount;
        let delta = amount - old_avg;
        stats.average_tip_amount = old_avg + (delta / new_count as i128);

        // Update standard deviation using Welford's online algorithm
        if old_count > 0 {
            let delta2 = amount - stats.average_tip_amount;
            let variance_sum = stats
                .standard_deviation
                .saturating_mul(stats.standard_deviation)
                .saturating_mul(old_count as i128);
            let new_variance_sum = variance_sum.saturating_add(delta.saturating_mul(delta2));
            let new_variance = new_variance_sum / new_count as i128;

            // Calculate square root approximation for standard deviation
            stats.standard_deviation = sqrt_approx(new_variance);
        }

        stats.sample_count = new_count;
        stats.last_update = current_time;
    }

    /// Approximates square root using Newton's method
    fn sqrt_approx(n: i128) -> i128 {
        if n == 0 {
            return 0;
        }
        if n < 0 {
            return 0;
        }

        let mut x = n;
        let mut y = (x + 1) / 2;

        // Iterate until convergence (max 10 iterations)
        for _ in 0..10 {
            if y >= x {
                return x;
            }
            x = y;
            y = (x + n / x) / 2;
        }

        x
    }

    /// Updates tip frequency statistics
    pub fn update_frequency_stats(
        stats: &mut HistoricalStats,
        tips_in_window: u32,
        window_duration_minutes: u32,
    ) {
        if window_duration_minutes == 0 {
            return;
        }

        let rate = tips_in_window / window_duration_minutes;

        // Exponential moving average for tip rate
        if stats.average_tips_per_minute == 0 {
            stats.average_tips_per_minute = rate;
        } else {
            // EMA with alpha = 0.2
            stats.average_tips_per_minute = (stats.average_tips_per_minute * 4 + rate) / 5;
        }
    }

    /// Updates unique sender statistics
    pub fn update_sender_stats(stats: &mut HistoricalStats, unique_senders: u32) {
        // Exponential moving average for unique senders
        if stats.unique_senders_per_hour == 0 {
            stats.unique_senders_per_hour = unique_senders;
        } else {
            // EMA with alpha = 0.2
            stats.unique_senders_per_hour =
                (stats.unique_senders_per_hour * 4 + unique_senders) / 5;
        }
    }

    /// Calculates confidence score for pattern comparison
    pub fn calculate_pattern_confidence(
        current_value: u32,
        historical_average: u32,
        deviation_threshold: u32,
    ) -> u32 {
        if historical_average == 0 {
            return 0;
        }

        let ratio = if current_value > historical_average {
            (current_value - historical_average) * 10000 / historical_average
        } else {
            0
        };

        if ratio > deviation_threshold {
            ((ratio - deviation_threshold) * 10000 / deviation_threshold).min(10000)
        } else {
            0
        }
    }

    /// Resets statistics (used for testing or after major system changes)
    pub fn reset_statistics(stats: &mut HistoricalStats, current_time: u64) {
        stats.average_tip_amount = 0;
        stats.standard_deviation = 0;
        stats.average_tips_per_minute = 0;
        stats.unique_senders_per_hour = 0;
        stats.last_update = current_time;
        stats.sample_count = 0;
    }
}

/// Pattern analysis module for detecting coordinated attacks
pub mod pattern_analyzer {
    use super::*;
    use soroban_sdk::Address;

    /// Analyzes tip velocity for a specific creator
    ///
    /// Returns anomaly score in basis points (0-10000)
    pub fn analyze_creator_velocity(
        creator_tip_count: u32,
        time_window_seconds: u64,
        historical_rate: u32,
    ) -> u32 {
        if time_window_seconds == 0 {
            return 0;
        }

        // Calculate current rate (tips per minute)
        let current_rate = (creator_tip_count * 60) / (time_window_seconds as u32);

        // Compare to historical rate
        if historical_rate == 0 {
            // No historical data, use absolute threshold
            if current_rate > 100 {
                // More than 100 tips/min is suspicious
                ((current_rate - 100) * 100).min(10000)
            } else {
                0
            }
        } else {
            // Compare to historical average
            if current_rate > historical_rate * 3 {
                // More than 3x historical rate is suspicious
                let excess_ratio = (current_rate * 10000) / historical_rate;
                (excess_ratio - 30000).min(10000)
            } else {
                0
            }
        }
    }

    /// Detects sequential tips from potentially related addresses
    ///
    /// Returns true if suspicious pattern detected
    pub fn detect_sequential_pattern(
        recent_senders: &[Address],
        recent_timestamps: &[u64],
        max_time_gap: u64,
    ) -> bool {
        if recent_senders.len() < 3 || recent_timestamps.len() < 3 {
            return false;
        }

        let mut sequential_count = 0;

        // Check for rapid sequential tips
        for i in 1..recent_timestamps.len().min(10) {
            let time_gap = recent_timestamps[i] - recent_timestamps[i - 1];

            if time_gap < max_time_gap {
                sequential_count += 1;
            }
        }

        // If more than 5 tips within max_time_gap, it's suspicious
        sequential_count > 5
    }

    /// Calculates pattern analysis score
    pub fn calculate_pattern_score(
        velocity_score: u32,
        sequential_detected: bool,
        clustering_score: u32,
    ) -> u32 {
        let mut total_score = velocity_score;

        if sequential_detected {
            total_score = total_score.saturating_add(3000); // Add 30% for sequential pattern
        }

        total_score = total_score.saturating_add(clustering_score / 2); // Add 50% of clustering score

        total_score.min(10000)
    }

    /// Checks if pattern analysis should trigger circuit breaker
    pub fn check_pattern_trigger(
        config: &EnhancedCircuitBreakerConfig,
        pattern_score: u32,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        if !config.pattern_analysis_enabled {
            return None;
        }

        if pattern_score > 7500 {
            Some((TriggerType::PatternAnalysis, TriggerSeverity::High))
        } else if pattern_score > 5000 {
            Some((TriggerType::PatternAnalysis, TriggerSeverity::Medium))
        } else {
            None
        }
    }

    /// Analyzes correlation with external market events
    ///
    /// This is a placeholder for future integration with oracle data
    pub fn analyze_market_correlation(_tip_volume: i128, _time_window: u64) -> u32 {
        // TODO: Integrate with price oracle to detect correlation
        // with market volatility or price movements
        0
    }
}

/// Halt state management module
pub mod halt_manager {
    use super::*;

    /// Activates halt with specified severity and trigger type
    pub fn activate_halt(
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        trigger_type: TriggerType,
        severity: TriggerSeverity,
        current_time: u64,
    ) {
        let cooldown =
            super::trigger_engine::calculate_cooldown(config, severity, state.trigger_count);

        state.halted_until = current_time.saturating_add(cooldown);
        state.halt_reason = trigger_type;
        state.halt_severity = severity;
        state.trigger_count = state.trigger_count.saturating_add(1);
        state.last_trigger_time = current_time;
        state.total_halts = state.total_halts.saturating_add(1);
        state.total_halt_duration = state.total_halt_duration.saturating_add(cooldown);
    }

    /// Checks if automatic recovery should occur
    pub fn check_automatic_recovery(
        state: &EnhancedCircuitBreakerState,
        current_time: u64,
    ) -> bool {
        state.halted_until > 0 && current_time >= state.halted_until
    }

    /// Performs automatic recovery after cooldown expires
    pub fn perform_automatic_recovery(state: &mut EnhancedCircuitBreakerState, current_time: u64) {
        state.halted_until = 0;
        state.halt_reason = TriggerType::None;
        state.halt_severity = TriggerSeverity::None;

        // Reset volume counters
        super::volume_tracker::reset_volume_counters(state, current_time);

        // Reset rate limit state
        super::rate_limiter::reset_rate_limit_state(state, current_time);
    }

    /// Checks if operations should be blocked
    pub fn should_block_operations(
        state: &EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        current_time: u64,
    ) -> bool {
        if !config.enabled {
            return false;
        }

        if config.maintenance_mode {
            return true;
        }

        state.is_halted(current_time)
    }

    /// Gets halt status information
    pub fn get_halt_status(
        state: &EnhancedCircuitBreakerState,
        current_time: u64,
    ) -> (bool, TriggerType, TriggerSeverity, u64) {
        let is_halted = state.is_halted(current_time);
        let remaining = state.remaining_cooldown(current_time);

        (
            is_halted,
            state.halt_reason.clone(),
            state.halt_severity,
            remaining,
        )
    }

    /// Extends existing halt period
    pub fn extend_halt(
        state: &mut EnhancedCircuitBreakerState,
        additional_seconds: u64,
        current_time: u64,
    ) -> Result<(), CircuitBreakerError> {
        if !state.is_halted(current_time) {
            return Err(CircuitBreakerError::InvalidConfiguration);
        }

        state.halted_until = state.halted_until.saturating_add(additional_seconds);
        state.total_halt_duration = state.total_halt_duration.saturating_add(additional_seconds);

        Ok(())
    }
}

/// Cooldown period management module
pub mod cooldown_manager {
    use super::*;

    /// Calculates cooldown period based on severity, trigger history, and configuration
    pub fn calculate_cooldown_period(
        config: &EnhancedCircuitBreakerConfig,
        severity: TriggerSeverity,
        trigger_count: u32,
        last_trigger_time: u64,
        current_time: u64,
    ) -> u64 {
        // Base cooldown based on severity
        let base_cooldown = match severity {
            TriggerSeverity::None | TriggerSeverity::Low => config.base_cooldown_seconds / 2,
            TriggerSeverity::Medium => config.base_cooldown_seconds,
            TriggerSeverity::High => config.base_cooldown_seconds * 2,
            TriggerSeverity::Critical => config.base_cooldown_seconds * 4,
        };

        // Apply exponential backoff if enabled and there are recent triggers
        let cooldown = if config.exponential_backoff_enabled && trigger_count > 0 {
            apply_exponential_backoff(
                base_cooldown,
                trigger_count,
                config.backoff_multiplier,
                last_trigger_time,
                current_time,
            )
        } else {
            base_cooldown
        };

        // Ensure cooldown is within configured bounds
        enforce_cooldown_bounds(cooldown, config)
    }

    /// Applies exponential backoff for repeated triggers
    fn apply_exponential_backoff(
        base_cooldown: u64,
        trigger_count: u32,
        multiplier_bps: u32,
        last_trigger_time: u64,
        current_time: u64,
    ) -> u64 {
        // Only apply backoff if triggers are within a short timeframe (1 hour)
        let time_since_last = current_time.saturating_sub(last_trigger_time);
        if time_since_last > 3600 {
            return base_cooldown;
        }

        let multiplier = multiplier_bps as u64;
        let mut adjusted = base_cooldown;

        // Apply multiplier for each recent trigger (cap at 5 to prevent overflow)
        for _ in 0..trigger_count.min(5) {
            adjusted = adjusted.saturating_mul(multiplier) / 10000;
        }

        adjusted
    }

    /// Enforces minimum and maximum cooldown bounds
    fn enforce_cooldown_bounds(cooldown: u64, config: &EnhancedCircuitBreakerConfig) -> u64 {
        cooldown
            .max(config.base_cooldown_seconds)
            .min(config.max_cooldown_seconds)
    }

    /// Calculates cooldown for creator-specific overrides
    pub fn calculate_creator_cooldown(
        base_cooldown: u64,
        creator_override_multiplier: Option<u32>,
    ) -> u64 {
        if let Some(multiplier) = creator_override_multiplier {
            // Multiplier in basis points (e.g., 5000 = 50% of base cooldown)
            base_cooldown.saturating_mul(multiplier as u64) / 10000
        } else {
            base_cooldown
        }
    }

    /// Determines longest cooldown when multiple triggers occur simultaneously
    pub fn select_longest_cooldown(cooldowns: &[u64]) -> u64 {
        cooldowns.iter().copied().max().unwrap_or(0)
    }

    /// Calculates graduated recovery cooldown (partial operation restoration)
    pub fn calculate_graduated_recovery(
        total_cooldown: u64,
        elapsed_time: u64,
        recovery_stages: u32,
    ) -> u32 {
        if recovery_stages == 0 || total_cooldown == 0 {
            return 0;
        }

        let stage_duration = total_cooldown / recovery_stages as u64;
        let current_stage = elapsed_time / stage_duration;

        current_stage.min(recovery_stages as u64) as u32
    }
}

/// Error handling and state preservation module
pub mod error_handler {
    use super::*;

    /// Circuit breaker specific error codes for different halt conditions
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    pub enum HaltErrorCode {
        /// Operations halted due to single tip spike
        SingleTipSpike = 1,
        /// Operations halted due to volume spike in 1-minute window
        VolumeSpike1Min = 2,
        /// Operations halted due to volume spike in 5-minute window
        VolumeSpike5Min = 3,
        /// Operations halted due to volume spike in 1-hour window
        VolumeSpike1Hour = 4,
        /// Operations halted due to volume spike in 24-hour window
        VolumeSpike24Hour = 5,
        /// Operations halted due to rate limiting
        RateLimitExceeded = 6,
        /// Operations halted due to anomaly detection
        AnomalyDetected = 7,
        /// Operations halted due to pattern analysis
        SuspiciousPattern = 8,
        /// Operations halted due to price volatility
        PriceVolatility = 9,
        /// Operations halted manually by admin
        ManualHalt = 10,
        /// Operations halted due to emergency mode
        EmergencyMode = 11,
        /// Operations halted due to maintenance mode
        MaintenanceMode = 12,
    }

    /// Gets error code for current halt condition
    pub fn get_halt_error_code(
        state: &EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
    ) -> HaltErrorCode {
        if config.maintenance_mode {
            return HaltErrorCode::MaintenanceMode;
        }

        if config.emergency_mode {
            return HaltErrorCode::EmergencyMode;
        }

        match &state.halt_reason {
            TriggerType::None => HaltErrorCode::ManualHalt,
            TriggerType::SingleTipSpike => HaltErrorCode::SingleTipSpike,
            TriggerType::VolumeSpike(TimeWindow::OneMinute) => HaltErrorCode::VolumeSpike1Min,
            TriggerType::VolumeSpike(TimeWindow::FiveMinutes) => HaltErrorCode::VolumeSpike5Min,
            TriggerType::VolumeSpike(TimeWindow::OneHour) => HaltErrorCode::VolumeSpike1Hour,
            TriggerType::VolumeSpike(TimeWindow::TwentyFourHours) => {
                HaltErrorCode::VolumeSpike24Hour
            }
            TriggerType::RateLimit => HaltErrorCode::RateLimitExceeded,
            TriggerType::AnomalyDetection => HaltErrorCode::AnomalyDetected,
            TriggerType::PatternAnalysis => HaltErrorCode::SuspiciousPattern,
            TriggerType::PriceVolatility => HaltErrorCode::PriceVolatility,
            TriggerType::Manual => HaltErrorCode::ManualHalt,
        }
    }

    /// Validates state integrity during initialization
    pub fn validate_state_integrity(
        state: &EnhancedCircuitBreakerState,
    ) -> Result<(), CircuitBreakerError> {
        // Check for logical consistency
        if state.halted_until > 0 && state.halt_reason == TriggerType::None {
            return Err(CircuitBreakerError::InvalidConfiguration);
        }

        // Validate volume windows
        if state.one_minute_window.current_volume < 0 {
            return Err(CircuitBreakerError::InvalidConfiguration);
        }

        // Validate trigger count
        if state.trigger_count > 1000000 {
            // Unreasonably high trigger count suggests corruption
            return Err(CircuitBreakerError::InvalidConfiguration);
        }

        Ok(())
    }

    /// Creates safe default state when corruption is detected
    pub fn create_safe_default_state(current_time: u64) -> EnhancedCircuitBreakerState {
        EnhancedCircuitBreakerState::new(current_time)
    }

    /// Preserves balances and state during halt periods
    pub fn ensure_state_preservation(_state: &EnhancedCircuitBreakerState) -> bool {
        // State preservation is guaranteed by not modifying any balance or
        // user state during halt periods. This function serves as a validation point.
        true
    }

    /// Handles corrupted state gracefully
    pub fn handle_corrupted_state(
        state: &EnhancedCircuitBreakerState,
        current_time: u64,
    ) -> EnhancedCircuitBreakerState {
        match validate_state_integrity(state) {
            Ok(_) => state.clone(),
            Err(_) => create_safe_default_state(current_time),
        }
    }
}

/// Administrative interface for circuit breaker management
pub mod admin {
    use super::*;
    use soroban_sdk::{Address, Env};

    /// Sets enhanced circuit breaker configuration
    ///
    /// Validates configuration before applying
    pub fn set_enhanced_config(
        env: &Env,
        admin: &Address,
        config: &EnhancedCircuitBreakerConfig,
    ) -> Result<(), CircuitBreakerError> {
        // Validate admin authorization (caller must verify this)
        admin.require_auth();

        // Validate configuration
        config.validate()?;

        // Store configuration
        env.storage()
            .instance()
            .set(&crate::CircuitBreakerKey::EnhancedConfig, config);

        // Emit configuration update event
        env.events().publish(
            (soroban_sdk::symbol_short!("cb_cfg"), admin.clone()),
            config.clone(),
        );

        Ok(())
    }

    /// Gets current enhanced configuration
    pub fn get_enhanced_config(env: &Env) -> Option<EnhancedCircuitBreakerConfig> {
        env.storage()
            .instance()
            .get(&crate::CircuitBreakerKey::EnhancedConfig)
    }

    /// Sets creator-specific limit override
    pub fn set_creator_limit(
        env: &Env,
        admin: &Address,
        creator: &Address,
        limit: i128,
    ) -> Result<(), CircuitBreakerError> {
        admin.require_auth();

        if limit <= 0 {
            return Err(CircuitBreakerError::InvalidSingleTipThreshold);
        }

        env.storage().persistent().set(
            &crate::CircuitBreakerKey::CreatorOverrides(creator.clone()),
            &limit,
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("cb_crlmt"), creator.clone()),
            limit,
        );

        Ok(())
    }

    /// Removes creator-specific limit override
    pub fn remove_creator_limit(env: &Env, admin: &Address, creator: &Address) {
        admin.require_auth();

        env.storage()
            .persistent()
            .remove(&crate::CircuitBreakerKey::CreatorOverrides(creator.clone()));

        env.events()
            .publish((soroban_sdk::symbol_short!("cb_crrm"), creator.clone()), ());
    }

    /// Gets creator-specific limit
    pub fn get_creator_limit(env: &Env, creator: &Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&crate::CircuitBreakerKey::CreatorOverrides(creator.clone()))
    }

    /// Sets token-specific limit
    pub fn set_token_limit(
        env: &Env,
        admin: &Address,
        token: &Address,
        limit: i128,
    ) -> Result<(), CircuitBreakerError> {
        admin.require_auth();

        if limit <= 0 {
            return Err(CircuitBreakerError::InvalidSingleTipThreshold);
        }

        env.storage().persistent().set(
            &crate::CircuitBreakerKey::TokenLimits(token.clone()),
            &limit,
        );

        env.events().publish(
            (soroban_sdk::symbol_short!("cb_tklmt"), token.clone()),
            limit,
        );

        Ok(())
    }

    /// Gets token-specific limit
    pub fn get_token_limit(env: &Env, token: &Address) -> Option<i128> {
        env.storage()
            .persistent()
            .get(&crate::CircuitBreakerKey::TokenLimits(token.clone()))
    }
}

/// Manual override capabilities for administrative control
pub mod manual_override {
    use super::*;
    use soroban_sdk::{Address, Env, String};

    /// Manually triggers circuit breaker halt
    pub fn manual_trigger(
        env: &Env,
        admin: &Address,
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        reason: String,
        severity: TriggerSeverity,
    ) {
        admin.require_auth();

        let current_time = env.ledger().timestamp();

        super::halt_manager::activate_halt(
            state,
            config,
            TriggerType::Manual,
            severity,
            current_time,
        );

        // Emit manual trigger event
        env.events().publish(
            (soroban_sdk::symbol_short!("cb_man"), admin.clone()),
            (reason, severity, state.halted_until),
        );
    }

    /// Manually resets circuit breaker state
    pub fn manual_reset(
        env: &Env,
        admin: &Address,
        state: &mut EnhancedCircuitBreakerState,
        force: bool,
    ) -> Result<(), CircuitBreakerError> {
        admin.require_auth();

        let current_time = env.ledger().timestamp();

        // Validate timing unless force is true
        if !force {
            // Require at least 50% of cooldown to have elapsed
            let elapsed = current_time.saturating_sub(state.last_trigger_time);
            let min_elapsed = super::cooldown_manager::calculate_cooldown_period(
                &EnhancedCircuitBreakerConfig::default_config(),
                state.halt_severity,
                0,
                0,
                0,
            ) / 2;

            if elapsed < min_elapsed {
                return Err(CircuitBreakerError::InvalidConfiguration);
            }
        }

        // Perform recovery
        super::halt_manager::perform_automatic_recovery(state, current_time);

        // Emit reset event
        env.events().publish(
            (soroban_sdk::symbol_short!("cb_rst"), admin.clone()),
            (force, current_time),
        );

        Ok(())
    }

    /// Extends existing halt period
    pub fn extend_halt(
        env: &Env,
        admin: &Address,
        state: &mut EnhancedCircuitBreakerState,
        additional_seconds: u64,
    ) -> Result<(), CircuitBreakerError> {
        admin.require_auth();

        let current_time = env.ledger().timestamp();

        super::halt_manager::extend_halt(state, additional_seconds, current_time)?;

        // Emit extension event
        env.events().publish(
            (soroban_sdk::symbol_short!("cb_ext"), admin.clone()),
            (additional_seconds, state.halted_until),
        );

        Ok(())
    }
}

/// Emergency and maintenance mode management
pub mod emergency_mode {
    use super::*;
    use soroban_sdk::{Address, Env, String};

    /// Enables emergency mode
    pub fn enable_emergency_mode(
        env: &Env,
        admin: &Address,
        config: &mut EnhancedCircuitBreakerConfig,
        reason: String,
    ) {
        admin.require_auth();

        config.emergency_mode = true;

        env.storage()
            .instance()
            .set(&crate::CircuitBreakerKey::EnhancedConfig, config);

        env.events().publish(
            (soroban_sdk::symbol_short!("cb_emerg"), admin.clone()),
            reason,
        );
    }

    /// Disables emergency mode
    pub fn disable_emergency_mode(
        env: &Env,
        admin: &Address,
        config: &mut EnhancedCircuitBreakerConfig,
    ) {
        admin.require_auth();

        config.emergency_mode = false;

        env.storage()
            .instance()
            .set(&crate::CircuitBreakerKey::EnhancedConfig, config);

        env.events()
            .publish((soroban_sdk::symbol_short!("cb_emoff"), admin.clone()), ());
    }

    /// Enables maintenance mode
    pub fn enable_maintenance_mode(
        env: &Env,
        admin: &Address,
        config: &mut EnhancedCircuitBreakerConfig,
    ) {
        admin.require_auth();

        config.maintenance_mode = true;

        env.storage()
            .instance()
            .set(&crate::CircuitBreakerKey::EnhancedConfig, config);

        env.events()
            .publish((soroban_sdk::symbol_short!("cb_maint"), admin.clone()), ());
    }

    /// Disables maintenance mode
    pub fn disable_maintenance_mode(
        env: &Env,
        admin: &Address,
        config: &mut EnhancedCircuitBreakerConfig,
    ) {
        admin.require_auth();

        config.maintenance_mode = false;

        env.storage()
            .instance()
            .set(&crate::CircuitBreakerKey::EnhancedConfig, config);

        env.events()
            .publish((soroban_sdk::symbol_short!("cb_mtoff"), admin.clone()), ());
    }

    /// Checks if system is in emergency or maintenance mode
    pub fn is_system_locked(config: &EnhancedCircuitBreakerConfig) -> bool {
        config.emergency_mode || config.maintenance_mode
    }
}

/// Query interface for external systems
pub mod query {
    use super::*;
    use soroban_sdk::Env;

    /// Query result for halt status
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct HaltStatusResult {
        pub is_halted: bool,
        pub halt_reason: TriggerType,
        pub halt_severity: TriggerSeverity,
        pub remaining_cooldown: u64,
        pub halted_until: u64,
    }

    /// Query result for volume statistics
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct VolumeStatsResult {
        pub one_minute_volume: i128,
        pub five_minute_volume: i128,
        pub one_hour_volume: i128,
        pub twenty_four_hour_volume: i128,
        pub one_minute_tip_count: u32,
        pub five_minute_tip_count: u32,
    }

    /// Checks if circuit breaker is currently halted
    pub fn is_halted(env: &Env, state: &EnhancedCircuitBreakerState) -> bool {
        let current_time = env.ledger().timestamp();
        state.is_halted(current_time)
    }

    /// Gets comprehensive halt status
    pub fn get_halt_status(env: &Env, state: &EnhancedCircuitBreakerState) -> HaltStatusResult {
        let current_time = env.ledger().timestamp();
        let (is_halted, reason, severity, remaining) =
            super::halt_manager::get_halt_status(state, current_time);

        HaltStatusResult {
            is_halted,
            halt_reason: reason,
            halt_severity: severity,
            remaining_cooldown: remaining,
            halted_until: state.halted_until,
        }
    }

    /// Gets remaining cooldown time in seconds
    pub fn get_remaining_cooldown(env: &Env, state: &EnhancedCircuitBreakerState) -> u64 {
        let current_time = env.ledger().timestamp();
        state.remaining_cooldown(current_time)
    }

    /// Gets current volume statistics across all time windows
    pub fn get_volume_stats(state: &EnhancedCircuitBreakerState) -> VolumeStatsResult {
        VolumeStatsResult {
            one_minute_volume: state.one_minute_window.current_volume,
            five_minute_volume: state.five_minute_window.current_volume,
            one_hour_volume: state.one_hour_window.current_volume,
            twenty_four_hour_volume: state.twenty_four_hour_window.current_volume,
            one_minute_tip_count: state.one_minute_window.tip_count,
            five_minute_tip_count: state.five_minute_window.tip_count,
        }
    }

    /// Gets anomaly scores if anomaly detection is enabled
    pub fn get_anomaly_scores(state: &EnhancedCircuitBreakerState) -> Option<(u32, u32, u32, u32)> {
        if let OptAnomalyDetectionState::Present(anomaly) = &state.anomaly_state {
            Some((
                anomaly.sender_clustering_score,
                anomaly.velocity_anomaly_score,
                anomaly.amount_deviation_score,
                anomaly.overall_confidence,
            ))
        } else {
            None
        }
    }

    /// Gets trigger history statistics
    pub fn get_trigger_stats(state: &EnhancedCircuitBreakerState) -> (u32, u64, u32, u64) {
        (
            state.trigger_count,
            state.last_trigger_time,
            state.total_halts,
            state.total_halt_duration,
        )
    }
}

/// Estimation and planning functions for integration
pub mod estimation {
    use super::*;
    use soroban_sdk::Env;

    /// Estimates recovery time based on current state
    pub fn estimate_recovery_time(env: &Env, state: &EnhancedCircuitBreakerState) -> u64 {
        let current_time = env.ledger().timestamp();

        if !state.is_halted(current_time) {
            return 0;
        }

        state.halted_until.saturating_sub(current_time)
    }

    /// Estimates gas cost for circuit breaker check operation
    pub fn estimate_check_gas_cost(config: &EnhancedCircuitBreakerConfig) -> u64 {
        let mut base_cost = 1000u64; // Base cost for simple checks

        // Add cost for volume tracking
        base_cost += 500;

        // Add cost for rate limiting
        base_cost += 300;

        // Add cost for anomaly detection if enabled
        if config.anomaly_detection_enabled {
            base_cost += 2000;
        }

        // Add cost for pattern analysis if enabled
        if config.pattern_analysis_enabled {
            base_cost += 1500;
        }

        base_cost
    }

    /// Provides performance metrics and recommendations
    pub fn get_performance_metrics(state: &EnhancedCircuitBreakerState) -> (u32, u64, f64) {
        let total_halts = state.total_halts;
        let total_duration = state.total_halt_duration;

        // Calculate average halt duration
        let avg_duration = if total_halts > 0 {
            total_duration as f64 / total_halts as f64
        } else {
            0.0
        };

        (total_halts, total_duration, avg_duration)
    }

    /// Estimates time until next potential trigger based on current trends
    pub fn estimate_next_trigger_risk(
        state: &EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
    ) -> u32 {
        let mut risk_score = 0u32;

        // Check volume proximity to thresholds
        let one_min_ratio = if config.one_minute_threshold > 0 {
            (state.one_minute_window.current_volume * 100) / config.one_minute_threshold
        } else {
            0
        };

        if one_min_ratio > 80 {
            risk_score += 5000; // 50% risk
        } else if one_min_ratio > 60 {
            risk_score += 3000; // 30% risk
        }

        // Check rate limit proximity
        let rate_ratio = if config.max_tips_per_minute > 0 {
            (state.rate_limit_state.tips_this_minute * 100) / config.max_tips_per_minute
        } else {
            0
        };

        if rate_ratio > 80 {
            risk_score += 3000;
        }

        // Check anomaly confidence
        if let OptAnomalyDetectionState::Present(anomaly) = &state.anomaly_state {
            if anomaly.overall_confidence > 5000 {
                risk_score += 2000;
            }
        }

        risk_score.min(10000) // Cap at 100%
    }
}

/// Batch query capabilities for efficient multi-entity queries
pub mod batch_query {
    use super::*;
    use soroban_sdk::{Address, Env, Map, Vec};

    /// Status code enumeration for integration compatibility
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq, Copy)]
    pub enum StatusCode {
        Normal = 0,
        Warning = 1,
        Halted = 2,
        Emergency = 3,
        Maintenance = 4,
    }

    /// Creator status result
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct CreatorStatus {
        pub creator: Address,
        pub has_override: bool,
        pub override_limit: Option<i128>,
        pub status_code: StatusCode,
    }

    /// Token status result
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct TokenStatus {
        pub token: Address,
        pub has_limit: bool,
        pub token_limit: Option<i128>,
        pub status_code: StatusCode,
    }

    /// Gets status for multiple creators in a single call
    pub fn get_creator_status_batch(
        env: &Env,
        creators: &Vec<Address>,
        config: &EnhancedCircuitBreakerConfig,
    ) -> Map<Address, CreatorStatus> {
        let mut results = Map::new(env);

        for creator in creators.iter() {
            let override_limit = super::admin::get_creator_limit(env, &creator);
            let has_override = override_limit.is_some();

            let status_code = get_system_status_code(config);

            results.set(
                creator.clone(),
                CreatorStatus {
                    creator: creator.clone(),
                    has_override,
                    override_limit,
                    status_code,
                },
            );
        }

        results
    }

    /// Gets status for multiple tokens in a single call
    pub fn get_token_status_batch(
        env: &Env,
        tokens: &Vec<Address>,
        config: &EnhancedCircuitBreakerConfig,
    ) -> Map<Address, TokenStatus> {
        let mut results = Map::new(env);

        for token in tokens.iter() {
            let token_limit = super::admin::get_token_limit(env, &token);
            let has_limit = token_limit.is_some();

            let status_code = get_system_status_code(config);

            results.set(
                token.clone(),
                TokenStatus {
                    token: token.clone(),
                    has_limit,
                    token_limit,
                    status_code,
                },
            );
        }

        results
    }

    /// Gets standardized status code for current system state
    fn get_system_status_code(config: &EnhancedCircuitBreakerConfig) -> StatusCode {
        if config.maintenance_mode {
            StatusCode::Maintenance
        } else if config.emergency_mode {
            StatusCode::Emergency
        } else if !config.enabled {
            StatusCode::Normal
        } else {
            StatusCode::Normal
        }
    }

    /// Gets status code for current halt state
    pub fn get_halt_status_code(
        state: &EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        current_time: u64,
    ) -> StatusCode {
        if config.maintenance_mode {
            return StatusCode::Maintenance;
        }

        if config.emergency_mode {
            return StatusCode::Emergency;
        }

        if state.is_halted(current_time) {
            return StatusCode::Halted;
        }

        // Check if we're approaching thresholds (warning state)
        let risk = super::estimation::estimate_next_trigger_risk(state, config);
        if risk > 7000 {
            StatusCode::Warning
        } else {
            StatusCode::Normal
        }
    }
}

/// Comprehensive event emission system
pub mod events {
    use super::*;
    use soroban_sdk::{symbol_short, Address, Env, Symbol};

    /// Recovery type enumeration
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub enum RecoveryType {
        Automatic,
        ManualReset,
        ManualForce,
        EmergencyOverride,
    }

    /// Emits trigger event when circuit breaker is activated
    pub fn emit_trigger_event(
        env: &Env,
        trigger_id: u64,
        trigger_type: &TriggerType,
        severity: TriggerSeverity,
        cooldown_duration: u64,
        volume_stats: Option<&super::query::VolumeStatsResult>,
    ) {
        env.events().publish(
            (symbol_short!("cb_trig"), trigger_id),
            (
                trigger_type.clone(),
                severity,
                cooldown_duration,
                volume_stats.cloned(),
            ),
        );
    }

    /// Emits recovery event when circuit breaker is reset
    pub fn emit_recovery_event(
        env: &Env,
        recovery_id: u64,
        recovery_type: RecoveryType,
        admin: Option<&Address>,
        halt_duration: u64,
    ) {
        env.events().publish(
            (symbol_short!("cb_rcvr"), recovery_id),
            (recovery_type, admin.cloned(), halt_duration),
        );
    }

    /// Emits configuration update event
    pub fn emit_config_update_event(
        env: &Env,
        admin: &Address,
        config: &EnhancedCircuitBreakerConfig,
    ) {
        env.events()
            .publish((symbol_short!("cb_cfg"), admin.clone()), config.clone());
    }

    /// Emits periodic status event during extended halts
    pub fn emit_status_event(
        env: &Env,
        state: &EnhancedCircuitBreakerState,
        remaining_cooldown: u64,
    ) {
        env.events().publish(
            (symbol_short!("cb_stat"), env.ledger().timestamp()),
            (
                state.halt_reason.clone(),
                state.halt_severity,
                remaining_cooldown,
            ),
        );
    }

    /// Emits audit trail event for state changes
    pub fn emit_audit_event(env: &Env, action: Symbol, admin: Option<&Address>, details: Symbol) {
        env.events().publish(
            (symbol_short!("cb_audit"), action),
            (admin.cloned(), details, env.ledger().timestamp()),
        );
    }

    /// Emits volume threshold warning event
    pub fn emit_threshold_warning(
        env: &Env,
        window: TimeWindow,
        current_volume: i128,
        threshold: i128,
        percentage: u32,
    ) {
        env.events().publish(
            (symbol_short!("cb_warn"), window),
            (current_volume, threshold, percentage),
        );
    }

    /// Emits anomaly detection event
    pub fn emit_anomaly_event(
        env: &Env,
        confidence_score: u32,
        clustering_score: u32,
        velocity_score: u32,
        amount_score: u32,
    ) {
        env.events().publish(
            (symbol_short!("cb_anom"), env.ledger().timestamp()),
            (
                confidence_score,
                clustering_score,
                velocity_score,
                amount_score,
            ),
        );
    }
}

/// Audit trail and history tracking
pub mod audit {
    use super::*;
    use soroban_sdk::{Address, Env, Symbol, Vec};

    /// Audit trail entry
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct AuditEntry {
        pub entry_id: u64,
        pub timestamp: u64,
        pub action: Symbol,
        pub admin: Option<Address>,
        pub details: Symbol,
    }

    /// Records audit trail entry for state change
    pub fn record_audit_entry(
        env: &Env,
        action: Symbol,
        admin: Option<&Address>,
        details: Symbol,
    ) -> u64 {
        let entry_id = get_next_audit_id(env);
        let timestamp = env.ledger().timestamp();

        let entry = AuditEntry {
            entry_id,
            timestamp,
            action: action.clone(),
            admin: admin.cloned(),
            details: details.clone(),
        };

        // Store audit entry (using temporary storage for recent entries)
        env.storage()
            .temporary()
            .set(&(symbol_short!("audit"), entry_id), &entry);

        // Emit audit event
        super::events::emit_audit_event(env, action.clone(), admin, details.clone());

        entry_id
    }

    /// Gets next audit entry ID
    fn get_next_audit_id(env: &Env) -> u64 {
        let current: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("aud_ctr"))
            .unwrap_or(0);

        let next = current.saturating_add(1);
        env.storage()
            .instance()
            .set(&symbol_short!("aud_ctr"), &next);

        next
    }

    /// Retrieves recent audit entries
    pub fn get_recent_audit_entries(env: &Env, limit: u32) -> Vec<AuditEntry> {
        let mut entries = Vec::new(env);
        let current_id = get_current_audit_id(env);

        let start_id = current_id.saturating_sub(limit as u64);

        for id in start_id..=current_id {
            if let Some(entry) = env
                .storage()
                .temporary()
                .get::<_, AuditEntry>(&(symbol_short!("audit"), id))
            {
                entries.push_back(entry);
            }
        }

        entries
    }

    /// Gets current audit ID without incrementing
    pub fn get_current_audit_id(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&symbol_short!("aud_ctr"))
            .unwrap_or(0)
    }

    /// Validates audit trail integrity
    pub fn validate_audit_integrity(env: &Env) -> bool {
        // Check if audit counter is reasonable
        let current_id = get_current_audit_id(env);
        current_id < 1_000_000 // Sanity check
    }
}

/// Periodic status event emission for extended halts
pub mod periodic_status {
    use super::*;
    use soroban_sdk::Env;

    /// Configuration for periodic status events
    pub const DEFAULT_STATUS_INTERVAL: u64 = 300; // 5 minutes

    /// Checks if periodic status event should be emitted
    pub fn should_emit_status(
        env: &Env,
        state: &EnhancedCircuitBreakerState,
        last_status_time: u64,
        interval: u64,
    ) -> bool {
        let current_time = env.ledger().timestamp();

        // Only emit if halted
        if !state.is_halted(current_time) {
            return false;
        }

        // Check if enough time has passed since last status
        current_time >= last_status_time.saturating_add(interval)
    }

    /// Emits periodic status event and returns new last status time
    pub fn emit_periodic_status(env: &Env, state: &EnhancedCircuitBreakerState) -> u64 {
        let current_time = env.ledger().timestamp();
        let remaining = state.remaining_cooldown(current_time);

        super::events::emit_status_event(env, state, remaining);

        current_time
    }

    /// Gets last status emission time from storage
    pub fn get_last_status_time(env: &Env) -> u64 {
        env.storage()
            .instance()
            .get(&soroban_sdk::symbol_short!("lst_stat"))
            .unwrap_or(0)
    }

    /// Updates last status emission time in storage
    pub fn update_last_status_time(env: &Env, time: u64) {
        env.storage()
            .instance()
            .set(&soroban_sdk::symbol_short!("lst_stat"), &time);
    }

    /// Checks and emits periodic status if needed
    pub fn check_and_emit_status(
        env: &Env,
        state: &EnhancedCircuitBreakerState,
        interval: Option<u64>,
    ) {
        let interval = interval.unwrap_or(DEFAULT_STATUS_INTERVAL);
        let last_time = get_last_status_time(env);

        if should_emit_status(env, state, last_time, interval) {
            let new_time = emit_periodic_status(env, state);
            update_last_status_time(env, new_time);
        }
    }
}

/// Performance optimization and caching layer
pub mod cache {
    use super::*;
    use soroban_sdk::Env;

    /// Cache keys for frequently accessed data
    const CACHE_CONFIG: soroban_sdk::Symbol = soroban_sdk::symbol_short!("c_cfg");
    const CACHE_STATE: soroban_sdk::Symbol = soroban_sdk::symbol_short!("c_state");
    const CACHE_VALID: soroban_sdk::Symbol = soroban_sdk::symbol_short!("c_valid");

    /// Gets cached configuration or loads from storage
    pub fn get_cached_config(env: &Env) -> Option<EnhancedCircuitBreakerConfig> {
        // Check if cache is valid
        if is_cache_valid(env) {
            // Try to get from instance storage (hot cache)
            if let Some(config) = env.storage().instance().get(&CACHE_CONFIG) {
                return Some(config);
            }
        }

        // Load from persistent storage
        if let Some(config) = super::admin::get_enhanced_config(env) {
            // Update cache
            update_config_cache(env, &config);
            Some(config)
        } else {
            None
        }
    }

    /// Updates configuration cache
    pub fn update_config_cache(env: &Env, config: &EnhancedCircuitBreakerConfig) {
        env.storage().instance().set(&CACHE_CONFIG, config);
        mark_cache_valid(env);
    }

    /// Gets cached state or loads from storage
    pub fn get_cached_state(env: &Env) -> Option<EnhancedCircuitBreakerState> {
        // State is always read from instance storage for consistency
        env.storage().instance().get(&CACHE_STATE)
    }

    /// Updates state cache
    pub fn update_state_cache(env: &Env, state: &EnhancedCircuitBreakerState) {
        env.storage().instance().set(&CACHE_STATE, state);
    }

    /// Checks if cache is valid
    fn is_cache_valid(env: &Env) -> bool {
        env.storage().instance().get(&CACHE_VALID).unwrap_or(false)
    }

    /// Marks cache as valid
    fn mark_cache_valid(env: &Env) {
        env.storage().instance().set(&CACHE_VALID, &true);
    }

    /// Invalidates cache (call when configuration changes)
    pub fn invalidate_cache(env: &Env) {
        env.storage().instance().set(&CACHE_VALID, &false);
    }

    /// Efficient data structure for volume tracking with minimal storage ops
    pub fn batch_update_volumes(env: &Env, state: &mut EnhancedCircuitBreakerState, amount: i128) {
        let current_time = env.ledger().timestamp();

        // Update all windows in a single operation
        super::volume_tracker::update_all_windows(state, amount, current_time);

        // Single storage write for all updates
        update_state_cache(env, state);
    }
}

/// State update batching and optimization
pub mod optimization {
    use super::*;
    use soroban_sdk::Env;

    /// Batches multiple state updates into a single storage operation
    pub struct StateBatch {
        updates_pending: bool,
    }

    impl Default for StateBatch {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StateBatch {
        pub fn new() -> Self {
            Self {
                updates_pending: false,
            }
        }

        pub fn mark_pending(&mut self) {
            self.updates_pending = true;
        }

        pub fn has_pending(&self) -> bool {
            self.updates_pending
        }

        pub fn flush(&mut self, env: &Env, state: &EnhancedCircuitBreakerState) {
            if self.updates_pending {
                super::cache::update_state_cache(env, state);
                self.updates_pending = false;
            }
        }
    }

    /// Performs conditional check optimization for disabled circuit breakers
    pub fn should_skip_checks(config: &EnhancedCircuitBreakerConfig) -> bool {
        !config.enabled
    }

    /// Lazy evaluation wrapper for complex anomaly detection
    pub fn lazy_anomaly_check(
        env: &Env,
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        amount: i128,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        // Skip if anomaly detection is disabled
        if !config.anomaly_detection_enabled {
            return None;
        }

        // Only perform expensive calculations if we're close to thresholds
        let current_time = env.ledger().timestamp();

        // Update anomaly state
        super::anomaly_detector::update_anomaly_state(
            &mut state.anomaly_state,
            amount,
            current_time,
        );

        // Check if anomaly should trigger
        super::anomaly_detector::check_anomaly_trigger(&state.anomaly_state, config)
    }

    /// Optimized check sequence that short-circuits on first trigger
    pub fn optimized_check_sequence(
        env: &Env,
        state: &mut EnhancedCircuitBreakerState,
        config: &EnhancedCircuitBreakerConfig,
        amount: i128,
        creator: &soroban_sdk::Address,
        token: &soroban_sdk::Address,
    ) -> Option<(TriggerType, TriggerSeverity)> {
        let current_time = env.ledger().timestamp();

        // Fast path: check if already halted
        if state.is_halted(current_time) {
            return Some((state.halt_reason.clone(), state.halt_severity));
        }

        // Fast path: skip if disabled
        if should_skip_checks(config) {
            return None;
        }

        // Check 1: Single tip spike (fastest check)
        if let Some(severity) =
            super::trigger_engine::check_single_tip_spike(config, amount, creator, token)
        {
            return Some((TriggerType::SingleTipSpike, severity));
        }

        // Check 2: Rate limiting (fast check)
        if let Some(trigger) =
            super::rate_limiter::check_rate_limit_trigger(state, config, current_time)
        {
            return Some(trigger);
        }

        // Check 3: Volume thresholds (medium cost)
        if let Some(trigger) =
            super::volume_tracker::check_all_volume_thresholds(state, config, current_time)
        {
            return Some(trigger);
        }

        // Check 4: Anomaly detection (expensive, lazy evaluation)
        if let Some(trigger) = lazy_anomaly_check(env, state, config, amount) {
            return Some(trigger);
        }

        None
    }

    /// Minimizes storage operations by batching related updates
    pub fn batch_state_updates(env: &Env, state: &mut EnhancedCircuitBreakerState, amount: i128) {
        let current_time = env.ledger().timestamp();

        // Batch all volume window updates
        super::volume_tracker::update_all_windows(state, amount, current_time);

        // Batch rate limit update
        super::rate_limiter::update_rate_limit_state(&mut state.rate_limit_state, current_time);

        // Single storage write for all updates
        super::cache::update_state_cache(env, state);
    }
}

/// Gas optimization and monitoring
pub mod gas_optimization {
    use super::*;

    /// Gas cost estimates for different operations
    pub struct GasCosts {
        pub simple_check: u64,
        pub volume_check: u64,
        pub rate_limit_check: u64,
        pub anomaly_check: u64,
        pub full_check: u64,
    }

    impl GasCosts {
        pub fn estimate() -> Self {
            Self {
                simple_check: 500,
                volume_check: 1000,
                rate_limit_check: 800,
                anomaly_check: 3000,
                full_check: 5500,
            }
        }
    }

    /// Performance benchmarks for regression testing
    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct PerformanceBenchmark {
        pub operation: soroban_sdk::Symbol,
        pub avg_gas_cost: u64,
        pub max_gas_cost: u64,
        pub sample_count: u32,
    }

    /// Monitors gas consumption and provides optimization recommendations
    pub fn get_optimization_recommendations(
        config: &EnhancedCircuitBreakerConfig,
    ) -> soroban_sdk::Vec<soroban_sdk::Symbol> {
        let mut recommendations = soroban_sdk::Vec::new(&soroban_sdk::Env::default());

        // Recommend disabling anomaly detection if not needed
        if config.anomaly_detection_enabled && !config.pattern_analysis_enabled {
            recommendations.push_back(soroban_sdk::symbol_short!("dis_anom"));
        }

        // Recommend increasing thresholds if too sensitive
        if config.one_minute_threshold < 1000 {
            recommendations.push_back(soroban_sdk::symbol_short!("inc_thrs"));
        }

        recommendations
    }

    /// Validates gas estimation accuracy
    pub fn validate_gas_estimates(estimated: u64, actual: u64, tolerance_percent: u32) -> bool {
        let tolerance = (estimated * tolerance_percent as u64) / 100;
        let diff = actual.abs_diff(estimated);

        diff <= tolerance
    }

    /// Calibrates gas estimates based on actual measurements
    pub fn calibrate_estimates(
        current_estimate: u64,
        actual_cost: u64,
        alpha: u32, // EMA smoothing factor in basis points
    ) -> u64 {
        // Exponential moving average: new = alpha * actual + (1 - alpha) * current
        let alpha_scaled = alpha as u64;
        let weighted_actual = actual_cost.saturating_mul(alpha_scaled) / 10000;
        let weighted_current = current_estimate.saturating_mul(10000 - alpha_scaled) / 10000;

        weighted_actual.saturating_add(weighted_current)
    }
}

/// Main circuit breaker guard interface - primary entry point
pub mod guard {
    use super::*;
    use soroban_sdk::{Address, Env};

    /// Main circuit breaker check function
    ///
    /// This is the primary entry point that should be called before tip operations
    pub fn check_circuit_breaker(
        env: &Env,
        amount: i128,
        creator: &Address,
        token: &Address,
    ) -> Result<(), CircuitBreakerError> {
        // Get cached configuration
        let config = match cache::get_cached_config(env) {
            Some(c) => c,
            None => {
                // No configuration, allow operation
                return Ok(());
            }
        };

        // Fast path: skip if disabled
        if optimization::should_skip_checks(&config) {
            return Ok(());
        }

        // Get or initialize state
        let mut state = cache::get_cached_state(env)
            .unwrap_or_else(|| EnhancedCircuitBreakerState::new(env.ledger().timestamp()));

        // Check for automatic recovery
        let current_time = env.ledger().timestamp();
        if halt_manager::check_automatic_recovery(&state, current_time) {
            halt_manager::perform_automatic_recovery(&mut state, current_time);

            // Emit recovery event
            let recovery_id = audit::get_current_audit_id(env);
            events::emit_recovery_event(
                env,
                recovery_id,
                events::RecoveryType::Automatic,
                None,
                state.total_halt_duration,
            );
        }

        // Check if operations should be blocked
        if halt_manager::should_block_operations(&state, &config, current_time) {
            let _error_code = error_handler::get_halt_error_code(&state, &config);

            // Emit periodic status if needed
            periodic_status::check_and_emit_status(env, &state, None);

            return Err(CircuitBreakerError::InvalidConfiguration); // Map to appropriate error
        }

        // Perform optimized check sequence
        if let Some((trigger_type, severity)) =
            optimization::optimized_check_sequence(env, &mut state, &config, amount, creator, token)
        {
            // Trigger detected - activate halt
            halt_manager::activate_halt(
                &mut state,
                &config,
                trigger_type.clone(),
                severity,
                current_time,
            );

            // Emit trigger event
            let trigger_id = state.trigger_count as u64;
            let volume_stats = query::get_volume_stats(&state);
            events::emit_trigger_event(
                env,
                trigger_id,
                &trigger_type,
                severity,
                state.halted_until - current_time,
                Some(&volume_stats),
            );

            // Record audit entry
            audit::record_audit_entry(
                env,
                soroban_sdk::symbol_short!("trigger"),
                None,
                soroban_sdk::symbol_short!("auto"),
            );

            // Update state cache
            cache::update_state_cache(env, &state);

            return Err(CircuitBreakerError::InvalidConfiguration); // Map to appropriate error
        }

        // Update state with new tip data
        optimization::batch_state_updates(env, &mut state, amount);

        Ok(())
    }

    /// Initializes circuit breaker with default configuration
    pub fn initialize_circuit_breaker(env: &Env, admin: &Address) {
        admin.require_auth();

        let config = EnhancedCircuitBreakerConfig::default_config();
        let state = EnhancedCircuitBreakerState::new(env.ledger().timestamp());

        // Store configuration and state
        cache::update_config_cache(env, &config);
        cache::update_state_cache(env, &state);

        // Emit initialization event
        events::emit_config_update_event(env, admin, &config);
    }

    /// Gets current circuit breaker status
    pub fn get_status(env: &Env) -> query::HaltStatusResult {
        let state = cache::get_cached_state(env)
            .unwrap_or_else(|| EnhancedCircuitBreakerState::new(env.ledger().timestamp()));

        query::get_halt_status(env, &state)
    }
}
