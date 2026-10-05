// File appender with rolling (rotate-on-size) support.
//
// When the current log file reaches `max_file_size`, it is rotated:
//   mho.log       -> mho.log.1
//   mho.log.1     -> mho.log.2
//   ...
//   mho.log.{N-1} -> mho.log.N   (mho.log.N is deleted first)
//
// Rotation is O(1) (a fixed number of `rename` syscalls) and never reads the
// file contents into memory, which matters on memory-constrained devices
// like routers. This also matches the rolling-file behaviour documented in
// the README.

use crate::error::{MhoError, Result};
use crate::log::LogLevel;
use crate::utils::fs::ensure_dir;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;

/// Number of rotated backup files to keep (`mho.log.1` .. `mho.log.N`).
const MAX_BACKUPS: usize = 5;

pub struct FileAppender {
    log_dir: PathBuf,
    current_file: Option<File>,
    current_size: u64,
    max_file_size: u64,
}

impl FileAppender {
    pub fn new(log_dir: PathBuf, max_file_size: u64) -> Result<Self> {
        ensure_dir(&log_dir)?;
        let mut appender = Self {
            log_dir,
            current_file: None,
            current_size: 0,
            max_file_size,
        };
        appender.open_current_file()?;
        Ok(appender)
    }

    fn current_log_path(&self) -> PathBuf {
        self.log_dir.join("mho.log")
    }

    fn backup_path(&self, n: usize) -> PathBuf {
        self.log_dir.join(format!("mho.log.{}", n))
    }

    fn open_current_file(&mut self) -> Result<()> {
        let log_path = self.current_log_path();
        self.current_size = if log_path.exists() {
            std::fs::metadata(&log_path).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| {
                MhoError::Log(format!(
                    "Failed to open log file {}: {}",
                    log_path.display(),
                    e
                ))
            })?;
        self.current_file = Some(file);
        Ok(())
    }

    /// Rotate the current log file out to `mho.log.1`, shifting older
    /// backups down and dropping the one that falls off the end. Cheap: a
    /// handful of `rename` calls, no file reads.
    fn rotate(&mut self) -> Result<()> {
        self.current_file = None;

        // Drop the oldest backup if it exists.
        let oldest = self.backup_path(MAX_BACKUPS);
        if oldest.exists() {
            let _ = std::fs::remove_file(&oldest);
        }

        // Shift mho.log.{i} -> mho.log.{i+1} for i = MAX_BACKUPS-1 .. 1.
        for i in (1..MAX_BACKUPS).rev() {
            let from = self.backup_path(i);
            if from.exists() {
                let _ = std::fs::rename(&from, self.backup_path(i + 1));
            }
        }

        // mho.log -> mho.log.1
        let cur = self.current_log_path();
        if cur.exists() {
            let _ = std::fs::rename(&cur, self.backup_path(1));
        }

        // Open a fresh mho.log.
        self.open_current_file()?;
        Ok(())
    }

    pub fn write_log(&mut self, _level: LogLevel, message: &str) -> Result<()> {
        let message_bytes = message.as_bytes();
        let message_len = message_bytes.len() as u64 + 1; // +1 for newline

        if self.current_size + message_len > self.max_file_size {
            self.rotate()?;
        }

        if let Some(file) = &mut self.current_file {
            file.write_all(message_bytes)
                .map_err(|e| MhoError::Log(format!("Failed to write to log file: {}", e)))?;
            file.write_all(b"\n").map_err(|e| {
                MhoError::Log(format!("Failed to write newline to log file: {}", e))
            })?;
            file.flush()
                .map_err(|e| MhoError::Log(format!("Failed to flush log file: {}", e)))?;
            self.current_size += message_len;
        }
        Ok(())
    }
}
