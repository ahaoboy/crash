use crate::config::get_log_dir;
// File appender with log trimming support
use crate::error::{CrashError, Result};
use crate::log::LogLevel;
use crate::utils::fs::ensure_dir;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write as _};
use std::path::PathBuf;

pub struct FileAppender {
    log_dir: PathBuf,
    current_file: Option<File>,
    current_size: u64,
    max_file_size: u64,
}

impl FileAppender {
    pub fn new(log_dir: PathBuf, max_file_size: u64) -> Result<Self> {
        // Ensure log directory exists
        ensure_dir(&log_dir)?;
        let mut appender = Self {
            log_dir,
            current_file: None,
            current_size: 0,
            max_file_size,
        };
        // Open or create the current log file
        appender.open_current_file()?;
        Ok(appender)
    }

    fn current_log_path(&self) -> PathBuf {
        self.log_dir.join("crash.log")
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

    pub fn write_log(&mut self, _level: LogLevel, message: &str) -> Result<()> {
        let message_bytes = message.as_bytes();
        let message_len = message_bytes.len() as u64 + 1; // +1 for newline

        ensure_dir(&get_log_dir())?;

        if self.current_size + message_len > self.max_file_size {
            // Close current file if open
            self.current_file = None;

            let path = self.current_log_path();

            // Read lines
            let file_read = File::open(&path).map_err(|e| {
                CrashError::Log(format!("Failed to open log file for reading: {}", e))
            })?;
            let reader = BufReader::new(file_read);
            let mut lines: Vec<String> = Vec::new();
            for line_res in reader.lines() {
                let mut line = line_res.map_err(|e| {
                    CrashError::Log(format!("Failed to read line from log file: {}", e))
                })?;
                line.push('\n');
                lines.push(line);
            }

            // Remove oldest lines until enough space
            let mut removed_size = 0u64;
            while !lines.is_empty()
                && self.current_size + message_len - removed_size > self.max_file_size
            {
                removed_size += lines[0].len() as u64;
                lines.remove(0);
            }

            // Overwrite the file with remaining lines
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .map_err(|e| {
                    CrashError::Log(format!("Failed to open log file for truncation: {}", e))
                })?;
            for line in &lines {
                file.write_all(line.as_bytes()).map_err(|e| {
                    CrashError::Log(format!("Failed to write trimmed line to log file: {}", e))
                })?;
            }

            // Append the new message
            file.write_all(message_bytes)
                .map_err(|e| CrashError::Log(format!("Failed to write to log file: {}", e)))?;
            file.write_all(b"\n").map_err(|e| {
                CrashError::Log(format!("Failed to write newline to log file: {}", e))
            })?;
            file.flush()
                .map_err(|e| CrashError::Log(format!("Failed to flush log file: {}", e)))?;

            // Update current size
            self.current_size = (self.current_size - removed_size) + message_len;

            // Keep the file open for future appends
            self.current_file = Some(file);
        } else {
            // Write to current file
            if let Some(file) = &mut self.current_file {
                file.write_all(message_bytes)
                    .map_err(|e| CrashError::Log(format!("Failed to write to log file: {}", e)))?;
                file.write_all(b"\n").map_err(|e| {
                    CrashError::Log(format!("Failed to write newline to log file: {}", e))
                })?;
                file.flush()
                    .map_err(|e| CrashError::Log(format!("Failed to flush log file: {}", e)))?;
                self.current_size += message_len;
            }
        }
        Ok(())
    }
}
