// Download management module with retry logic

use crate::error::{CrashError, Result};
use crate::{log_debug, log_error, log_info, log_warn};
use reqwest::Client;
use std::path::Path;
use std::time::Duration;

fn new_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .unwrap_or_else(|_| Client::new())
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

/// Download a file from URL to destination path with retry logic
pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    log_info!("Starting download from {} to {}", url, dest.display());

    // Download text content
    let text = download_text(url).await?;

    // Write to file
    log_debug!("Writing {} bytes to {}", text.len(), dest.display());

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        crate::utils::fs::ensure_dir(parent)?;
    }

    std::fs::write(dest, &text).map_err(|e| {
        CrashError::Download(format!("Failed to write file {}: {}", dest.display(), e))
    })?;

    // Validate file was written correctly
    let written_size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);

    if written_size != text.len() as u64 {
        return Err(CrashError::Download(format!(
            "File size mismatch: expected {}, got {}",
            text.len(),
            written_size
        )));
    }

    log_info!("Download completed successfully: {}", url);
    Ok(())
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
