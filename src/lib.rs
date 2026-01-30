//! Anthropic-Bedrock API Proxy library

// Public modules
pub mod api;
pub mod config;
pub mod converters;
pub mod db;
pub mod error;
pub mod logging;
pub mod middleware;
pub mod schemas;
pub mod server;
pub mod services;
pub mod utils;

// Re-export commonly used types
pub use config::Settings;
pub use error::ApiError;
pub use server::App;
