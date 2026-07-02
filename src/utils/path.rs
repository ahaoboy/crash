// Cross-platform path utilities

/// Get the executable extension for the current platform
pub fn exe_extension() -> &'static str {
    if cfg!(windows) { ".exe" } else { "" }
}
