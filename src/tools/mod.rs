// Tools module - corresponds to original tools/ directory

pub mod shell_ddns;

// Re-export main functionality
pub use shell_ddns::DDNSManager;
