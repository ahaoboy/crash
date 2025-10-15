// ShellCrash Rust Implementation
// Copyright (C) Rust Port

pub mod common;
pub mod scripts;
pub mod tools;
pub mod core;


// Re-export commonly used types
pub use common::{
    config::Config,
    error::{Result, ShellCrashError},
    logger::Logger,
    shell::ShellExecutor,
};
