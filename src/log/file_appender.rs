// File appender with log rotation support

use crate::error::{CrashError, Result};
use crate::log::LogLevel;
use crate::utils::fs::ensure_dir;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub struct FileAppender {
    log_dir: PathBuf,
    current_file: Option<File>,
    current_size: u64,
    max_file_size: u64,
    max_files: usize,
}

impl FileAppender {
    pub fn new(log_dir: PathBuf, max_file_size: u64, max_files: usize) -> Result<Self> {
        // Ensure log directory exists
        ensure_dir(&log_dir)?;

        let mut appender = Self {
            log_dir,
            current_file: None,
            current_size: 0,
            max_file_size,
            max_files,
        };

        // Open or create the current log file
        appender.open_current_file()?;

        Ok(appender)
    }

    fn current_log_path(&self) -> PathBuf {
        self.log_dir.join("crash.log")
    }

    fn rotated_log_path(&self, index: usize) -> PathBuf {
        self.log_dir.join(format!("crash.log.{}", index))
    }

    fn open_current_file(&mut self) -> Result<()> {
        let log_path = self.current_log_path();

        // Get current file size if it exists
        self.current_size = if log_path.exists() {
            std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        // Open file in append mode
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| {
                CrashError::Log(format!(
                    "Failed to open log file {}: {}",
                    log_path.display(),
                    e
                ))
            })?;

        self.current_file = Some(file);
        Ok(())
    }

    fn rotate_logs(&mut self) -> Result<()> {
        // Close current file
        self.current_file = None;

        // Remove oldest log file if it exists
        let oldest_path = self.rotated_log_path(self.max_files);
        if oldest_path.exists() {
            std::fs::remove_file(&oldest_path).map_err(|e| {
                CrashError::Log(format!(
                    "Failed to remove old log file {}: {}",
                    oldest_path.display(),
                    e
                ))
            })?;
        }

        // Rotate existing log files
        for i in (1..self.max_files).rev() {
            let from_path = self.rotated_log_path(i);
            let to_path = self.rotated_log_path(i + 1);

            if from_path.exists() {
                std::fs::rename(&from_path, &to_path).map_err(|e| {
                    CrashError::Log(format!(
                        "Failed to rotate log file {} to {}: {}",
                        from_path.display(),
                        to_path.display(),
                        e
                    ))
                })?;
            }
        }

        // Move current log to .1
        let current_path = self.current_log_path();
        let rotated_path = self.rotated_log_path(1);

        if current_path.exists() {
            std::fs::rename(&current_path, &rotated_path).map_err(|e| {
                CrashError::Log(format!(
                    "Failed to rotate current log file {} to {}: {}",
                    current_path.display(),
                    rotated_path.display(),
                    e
                ))
            })?;
        }

        // Open new current file
        self.current_size = 0;
        self.open_current_file()?;

        Ok(())
    }

    pub fn write_log(&mut self, _level: LogLevel, message: &str) -> Result<()> {
        let message_bytes = message.as_bytes();
        let message_len = message_bytes.len() as u64;

        // Check if rotation is needed
        if self.current_size + message_len > self.max_file_size {
            self.rotate_logs()?;
        }

        // Write to current file
        if let Some(file) = &mut self.current_file {
            file.write_all(message_bytes)
                .map_err(|e| CrashError::Log(format!("Failed to write to log file: {}", e)))?;

            file.write_all(b"\n").map_err(|e| {
                CrashError::Log(format!("Failed to write newline to log file: {}", e))
            })?;

            file.flush()
                .map_err(|e| CrashError::Log(format!("Failed to flush log file: {}", e)))?;

            self.current_size += message_len + 1; // +1 for newline
        }

        Ok(())
    }
}
