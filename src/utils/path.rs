// Cross-platform path utilities

use crate::error::{CrashError, Result};
use std::path::{Path, PathBuf};

/// Sanitize a path to prevent directory traversal attacks
pub fn sanitize_path(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize().map_err(|e| {
        CrashError::Platform(format!(
            "Failed to canonicalize path {}: {}",
            path.display(),
            e
        ))
    })?;

    // Check for path traversal attempts
    if canonical
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(CrashError::Platform(format!(
            "Path traversal detected in: {}",
            path.display()
        )));
    }

    Ok(canonical)
}

/// Join paths in a cross-platform way
pub fn join_paths(base: &Path, relative: &Path) -> PathBuf {
    base.join(relative)
}

/// Normalize a path by resolving . and .. components
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {
                // Skip current directory references
            }
            _ => {
                normalized.push(component);
            }
        }
    }

    normalized
}

/// Get the executable extension for the current platform
pub fn exe_extension() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}
