// Retry configuration with exponential backoff

use std::time::Duration;

/// Configuration for retry logic
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl RetryConfig {
    /// Create a new retry configuration
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            initial_delay_ms: 1000, // 1 second
            max_delay_ms: 30000,    // 30 seconds
        }
    }

    /// Calculate delay for a given attempt using exponential backoff
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        // Exponential backoff: initial_delay * 2^(attempt-1)
        let delay_ms = self.initial_delay_ms * 2u64.pow(attempt - 1);

        // Cap at maximum delay
        let capped_delay = delay_ms.min(self.max_delay_ms);

        Duration::from_millis(capped_delay)
    }
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self::new(3)
    }
}
