// Download management module with retry logic

use crate::error::{CrashError, Result};
use crate::{log_debug, log_error, log_info, log_warn};
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

pub fn new_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(600))
            .connect_timeout(Duration::from_secs(30))
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .expect("Failed to create HTTP client")
    })
}

const INITIAL_DELAY_MS: u64 = 1000; // 1 second
const MAX_DELAY_MS: u64 = 30000; // 30 seconds
const MAX_RETRIES: u32 = 3;

/// Calculate delay for a given attempt using exponential backoff
fn calculate_delay(attempt: u32) -> Duration {
    if attempt == 0 {
        return Duration::from_millis(0);
    }

    // Exponential backoff: initial_delay * 2^(attempt-1)
    let delay_ms = INITIAL_DELAY_MS * 2u64.pow(attempt - 1);

    // Cap at maximum delay
    let capped_delay = delay_ms.min(MAX_DELAY_MS);

    Duration::from_millis(capped_delay)
}

/// Download text content from URL with retry logic
pub async fn download_text(url: &str) -> Result<String> {
    log_info!("Starting text download from {}", url);

    let mut attempt = 0;
    let mut last_error = None;

    while attempt <= MAX_RETRIES {
        if attempt > 0 {
            let delay = calculate_delay(attempt);
            log_warn!(
                "Retry attempt {} after {:?} delay for {}",
                attempt,
                delay,
                url
            );
            tokio::time::sleep(delay).await;
        }

        match download_text_attempt(url).await {
            Ok(text) => {
                log_info!("Text download completed successfully: {}", url);
                return Ok(text);
            }
            Err(e) => {
                log_error!("Download attempt {} failed: {}", attempt + 1, e);
                last_error = Some(e);
                attempt += 1;
            }
        }
    }

    Err(last_error
        .unwrap_or_else(|| CrashError::Download("Download failed after all retries".to_string())))
}

/// Single text download attempt
async fn download_text_attempt(url: &str) -> Result<String> {
    log_debug!("Sending HTTP GET request to {}", url);

    let response = new_client()
        .get(url)
        .send()
        .await
        .map_err(|e| CrashError::Download(format!("HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(CrashError::Download(format!(
            "HTTP request failed with status: {}",
            response.status()
        )));
    }

    log_debug!("Reading response body as text");
    let text = response
        .text()
        .await
        .map_err(|e| CrashError::Download(format!("Failed to read response body: {}", e)))?;

    Ok(text)
}
