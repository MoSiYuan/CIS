//! # Core Command Handlers
//!
//! Implementations for core commands.

pub mod init;
pub mod status;
pub mod config;
pub mod doctor;
pub mod completion;

pub use init::execute as init_execute;
pub use status::execute as status_execute;
pub use config::execute as config_execute;
pub use doctor::execute as doctor_execute;
pub use completion::execute as completion_execute;
