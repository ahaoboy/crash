// Output formatting utilities for CLI

/// Output formatter for consistent CLI output
pub struct OutputFormatter;

impl OutputFormatter {
    /// Format a success message
    pub fn success(message: &str) -> String {
        format!("✅ {}", message)
    }

    /// Format an error message
    pub fn error(message: &str) -> String {
        format!("❌ Error: {}", message)
    }

    /// Format a warning message
    pub fn warning(message: &str) -> String {
        format!("⚠️  Warning: {}", message)
    }

    /// Format an info message
    pub fn info(message: &str) -> String {
        format!("ℹ️  {}", message)
    }

    /// Format a key-value pair for status display
    pub fn key_value(key: &str, value: &str, width: usize) -> String {
        format!("{:width$} : {}", key, value, width = width)
    }
}
