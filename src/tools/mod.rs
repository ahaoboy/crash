// Tools module - corresponds to original tools/ directory

pub mod shell_ddns;
pub mod stop;

// Re-export main functionality
pub use shell_ddns::DDNSManager;
