// Platform abstraction layer for cross-platform compatibility

pub mod command;
pub mod path;
pub mod process;

pub use command::CommandExecutor;
pub use process::{ProcessManager, get_process_manager};
