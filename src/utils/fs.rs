// File system utilities

use crate::error::{CrashError, Result};
use std::path::Path;

/// Ensures a directory exists, creating it if necessary
pub fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path).map_err(|e| {
            CrashError::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to create directory {}: {}", path.display(), e),
            ))
        })?;
    }
    Ok(())
}

/// Writes content to a file atomically by writing to a temp file first
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        ensure_dir(parent)?;
    }

    // Write to temporary file first
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, content).map_err(|e| {
        CrashError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to write to temp file {}: {}",
                temp_path.display(),
                e
            ),
        ))
    })?;

    // Rename temp file to target (atomic operation)
    std::fs::rename(&temp_path, path).map_err(|e| {
        CrashError::Io(std::io::Error::new(
            e.kind(),
            format!(
                "Failed to rename {} to {}: {}",
                temp_path.display(),
                path.display(),
                e
            ),
        ))
    })?;

    Ok(())
}

/// Checks if a file exists
pub fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}
