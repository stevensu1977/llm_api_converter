//! Application server
//!
//! This module provides the main application server implementation
//! including initialization and graceful shutdown handling.

use crate::{
    config::Settings,
    server::{routes, state::AppState},
};
use anyhow::Result;
use std::net::SocketAddr;
use tokio::signal;

/// Main application struct
pub struct App {
    settings: Settings,
    state: AppState,
}

impl App {
    /// Create a new application instance
    ///
    /// This initializes all services and prepares the application for running.
    pub async fn new(settings: Settings) -> Result<Self> {
        tracing::debug!("Initializing application state");
        let state = AppState::new(settings.clone()).await?;

        Ok(Self { settings, state })
    }

    /// Run the server (without graceful shutdown)
    pub async fn run(self) -> Result<()> {
        let addr = self.settings.server_addr().parse::<SocketAddr>()?;
        let router = routes::create_router(self.state);

        tracing::info!("Starting server on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Run the server with graceful shutdown support
    ///
    /// The server will shut down gracefully when receiving SIGINT (Ctrl+C)
    /// or SIGTERM signals.
    pub async fn run_with_graceful_shutdown(self) -> Result<()> {
        let addr = self.settings.server_addr().parse::<SocketAddr>()?;
        let router = routes::create_router(self.state.clone());

        tracing::info!("Starting server on {} with graceful shutdown enabled", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        // Cleanup resources
        self.cleanup().await;

        Ok(())
    }

    /// Cleanup application resources
    async fn cleanup(&self) {
        tracing::info!("Cleaning up application resources");
        // TODO: Add cleanup for PTC containers in Phase 7
        // TODO: Add cleanup for any pending DynamoDB writes in Phase 2
    }

    /// Get a reference to the application state
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Get a reference to the settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

/// Create a future that completes when a shutdown signal is received
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, initiating graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
