//! Server module
//!
//! Contains application state, routing, and server initialization logic.

pub mod app;
pub mod routes;
pub mod state;

pub use app::App;
pub use state::AppState;
