// Utility modules for shared functionality
pub mod command;
pub mod download;
pub mod fs;
pub mod monitor;
pub mod path;
pub mod process;
pub mod time;
pub use fs::{atomic_write, ensure_dir, file_exists};
use std::path::Path;
pub use time::{current_timestamp, format_uptime};

use crate::utils::command::execute;

pub fn get_user() -> String {
    if let Ok(v) = std::env::var("USER") {
        return v;
    }

    if let Ok(v) = execute("whoami", &[]) {
        return v;
    }

    "UNKNOWN".to_string()
}

/// Compute the total size of a directory tree in bytes.
///
/// Uses a pure-Rust recursive walk so behaviour is identical across platforms
/// and there is no risk of shell injection from shelling out to `du` or
/// `powershell` with an arbitrary path.
pub fn get_dir_size(path: &Path) -> u64 {
    fn dir_size(path: &Path) -> u64 {
        let mut size = 0;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        size += metadata.len();
                    } else if metadata.is_dir() {
                        size += dir_size(&entry.path());
                    }
                }
            }
        }
        size
    }

    if path.exists() { dir_size(path) } else { 0 }
}

/// Format a byte count as a human-readable string, using binary units
/// (1024-based, e.g. KiB/MiB) consistently across all platforms.
pub fn format_size(n: u64) -> String {
    humansize::format_size(n, humansize::BINARY)
}

const SUFFIXES: [&str; 8] = [
    ".tar.gz", ".tar.xz", ".tar.bz2", ".zip", ".gz", ".xz", ".bz2", ".tgz",
];

pub fn strip_suffix(name: &str) -> &str {
    for suffix in SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }

    name
}

pub fn is_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Probe `url` via the shared HTTP client with a 5s timeout.
///
/// Used as a proxy health check: under TUN + `auto-route` (the mihomo
/// default), this traffic is captured by the TUN device and forwarded
/// through the proxy, so a success implies the proxy is forwarding
/// correctly. A failure triggers a restart in `CrashConfig::start`.
///
/// The short timeout prevents a stuck proxy from hanging the (scheduled)
/// `start` command indefinitely.
pub async fn check_connectivity(url: &str) -> bool {
    let fut = crate::utils::download::new_client().get(url).send();
    match tokio::time::timeout(std::time::Duration::from_secs(5), fut).await {
        Ok(Ok(response)) => response.status().is_success(),
        _ => false,
    }
}
