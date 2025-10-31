// Download management module with retry logic

use crate::error::{CrashError, Result};
use crate::{log_debug, log_error, log_info, log_warn};
use reqwest::Client;
use std::path::Path;
use std::time::Duration;

mod retry;

pub use retry::RetryConfig;

/// Download manager with HTTP client and retry logic
#[derive(Clone)]
pub struct DownloadManager {
    client: Client,
    retry_config: RetryConfig,
}

impl DownloadManager {
    /// Create a new download manager with specified timeout and retry settings
    pub fn new(timeout_secs: u64, max_retries: u32) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            retry_config: RetryConfig::new(max_retries),
        }
    }

    /// Download a file from URL to destination path with retry logic
    pub async fn download_file(&self, url: &str, dest: &Path) -> Result<()> {
        log_info!("Starting download from {} to {}", url, dest.display());

        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= self.retry_config.max_retries {
            if attempt > 0 {
                let delay = self.retry_config.calculate_delay(attempt);
                log_warn!(
                    "Retry attempt {} after {:?} delay for {}",
                    attempt,
                    delay,
                    url
                );
                tokio::time::sleep(delay).await;
            }

            match self.download_attempt(url, dest).await {
                Ok(_) => {
                    log_info!("Download completed successfully: {}", url);
                    return Ok(());
                }
                Err(e) => {
                    log_error!("Download attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                    attempt += 1;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            CrashError::Download("Download failed after all retries".to_string())
        }))
    }

    /// Single download attempt
    async fn download_attempt(&self, url: &str, dest: &Path) -> Result<()> {
        log_debug!("Sending HTTP GET request to {}", url);

        let response = self
            .client
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

        log_debug!("Reading response body");
        let bytes = response
            .bytes()
            .await
            .map_err(|e| CrashError::Download(format!("Failed to read response body: {}", e)))?;

        log_debug!("Writing {} bytes to {}", bytes.len(), dest.display());

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            crate::utils::fs::ensure_dir(parent)?;
        }

        std::fs::write(dest, &bytes).map_err(|e| {
            CrashError::Download(format!("Failed to write file {}: {}", dest.display(), e))
        })?;

        // Validate file was written correctly
        let written_size = std::fs::metadata(dest).map(|m| m.len()).unwrap_or(0);

        if written_size != bytes.len() as u64 {
            return Err(CrashError::Download(format!(
                "File size mismatch: expected {}, got {}",
                bytes.len(),
                written_size
            )));
        }

        Ok(())
    }

    /// Download a file with progress callback
    pub async fn download_with_progress<F>(
        &self,
        url: &str,
        dest: &Path,
        progress_fn: F,
    ) -> Result<()>
    where
        F: Fn(u64, u64),
    {
        log_info!("Starting download with progress tracking from {}", url);

        let response = self
            .client
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

        let total_size = response.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut buffer = Vec::new();

        use futures_util::StreamExt;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| CrashError::Download(format!("Failed to read chunk: {}", e)))?;

            buffer.extend_from_slice(&chunk);
            downloaded += chunk.len() as u64;
            progress_fn(downloaded, total_size);
        }

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            crate::utils::fs::ensure_dir(parent)?;
        }

        std::fs::write(dest, &buffer).map_err(|e| {
            CrashError::Download(format!("Failed to write file {}: {}", dest.display(), e))
        })?;

        log_info!("Download with progress completed: {}", url);
        Ok(())
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new(300, 3) // 5 minute timeout, 3 retries
    }
}
