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
use std::process::Command;
pub use time::{current_timestamp, format_uptime};

use crate::utils::command::execute;

pub async fn check_google() -> bool {
    if let Ok(response) = reqwest::get("https://www.google.com").await {
        return response.status().is_success();
    }
    false
}

pub fn get_user() -> String {
    if let Ok(v) = std::env::var("USER") {
        return v;
    }

    if let Ok(v) = execute("whoami", &[],None) {
        return v;
    }

    "root".to_string()
}

pub fn get_dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = Command::new("du").arg("-s").arg(path).output()
            && output.status.success()
            && let Some(size_str) = String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .next()
            && let Ok(size) = size_str.parse::<u64>()
        {
            return size * 1024;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = Command::new("du").arg("-s").arg(path).output() {
            if output.status.success() {
                if let Some(size_str) = String::from_utf8_lossy(&output.stdout)
                    .split_whitespace()
                    .next()
                {
                    if let Ok(size_kb) = size_str.parse::<u64>() {
                        return size_kb * 1024;
                    }
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = Command::new("powershell")
            .args([
                "-c",
                &format!(
                    "(Get-ChildItem -Recurse '{}' | Measure-Object -Property Length -Sum).Sum",
                    path.display()
                ),
            ])
            .output()
            && output.status.success()
            && let Ok(size_str) = String::from_utf8(output.stdout)
            && let Ok(size) = size_str.trim().parse::<u64>()
        {
            return size;
        }
    }

    fn fallback_size(path: &Path) -> u64 {
        let mut size = 0;
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        size += metadata.len();
                    } else if metadata.is_dir() {
                        size += fallback_size(&entry.path());
                    }
                }
            }
        }
        size
    }

    fallback_size(path)
}

pub fn format_size(n: u64) -> String {
    humansize::format_size(
        n,
        if cfg!(windows) {
            humansize::WINDOWS
        } else {
            humansize::DECIMAL
        },
    )
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
