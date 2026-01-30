//! PTC-specific exceptions and error types
//!
//! This module defines error types for Programmatic Tool Calling (PTC) operations.

use thiserror::Error;

/// Errors that can occur during PTC operations.
#[derive(Debug, Error)]
pub enum PtcError {
    /// Docker daemon is not available
    #[error("Docker not available: {0}")]
    DockerNotAvailable(String),

    /// Failed to create container
    #[error("Failed to create container: {0}")]
    ContainerCreationFailed(String),

    /// Failed to start container
    #[error("Failed to start container: {0}")]
    ContainerStartFailed(String),

    /// Container execution timeout
    #[error("Code execution timeout after {0} seconds")]
    ExecutionTimeout(u64),

    /// Container exited unexpectedly
    #[error("Container exited with code {0}: {1}")]
    ContainerExited(i64, String),

    /// Failed to copy files to container
    #[error("Failed to copy files to container: {0}")]
    FileCopyFailed(String),

    /// Failed to execute command in container
    #[error("Failed to execute command: {0}")]
    ExecFailed(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session expired
    #[error("Session expired: {0}")]
    SessionExpired(String),

    /// Invalid tool result
    #[error("Invalid tool result: {0}")]
    InvalidToolResult(String),

    /// IPC communication error
    #[error("IPC communication error: {0}")]
    IpcError(String),

    /// Code execution error
    #[error("Code execution error: {0}")]
    CodeExecutionError(String),

    /// Max iterations exceeded
    #[error("Maximum code execution iterations ({0}) exceeded")]
    MaxIterationsExceeded(u32),

    /// Image not found
    #[error("Docker image not found: {0}")]
    ImageNotFound(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// Internal error
    #[error("Internal PTC error: {0}")]
    Internal(String),
}

impl PtcError {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            PtcError::DockerNotAvailable(_)
                | PtcError::NetworkError(_)
                | PtcError::ExecutionTimeout(_)
        )
    }

    /// Convert to HTTP status code
    pub fn status_code(&self) -> u16 {
        match self {
            PtcError::DockerNotAvailable(_) => 503, // Service Unavailable
            PtcError::SessionNotFound(_) => 404,
            PtcError::SessionExpired(_) => 410, // Gone
            PtcError::InvalidToolResult(_) => 400,
            PtcError::ExecutionTimeout(_) => 504, // Gateway Timeout
            PtcError::MaxIterationsExceeded(_) => 429, // Too Many Requests
            _ => 500,
        }
    }
}

/// Result type for PTC operations
pub type PtcResult<T> = Result<T, PtcError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_retryable() {
        assert!(PtcError::DockerNotAvailable("test".to_string()).is_retryable());
        assert!(PtcError::NetworkError("test".to_string()).is_retryable());
        assert!(PtcError::ExecutionTimeout(60).is_retryable());
        assert!(!PtcError::SessionNotFound("test".to_string()).is_retryable());
    }

    #[test]
    fn test_error_status_codes() {
        assert_eq!(PtcError::DockerNotAvailable("test".to_string()).status_code(), 503);
        assert_eq!(PtcError::SessionNotFound("test".to_string()).status_code(), 404);
        assert_eq!(PtcError::SessionExpired("test".to_string()).status_code(), 410);
        assert_eq!(PtcError::InvalidToolResult("test".to_string()).status_code(), 400);
        assert_eq!(PtcError::ExecutionTimeout(60).status_code(), 504);
        assert_eq!(PtcError::MaxIterationsExceeded(10).status_code(), 429);
        assert_eq!(PtcError::Internal("test".to_string()).status_code(), 500);
    }

    #[test]
    fn test_error_display() {
        let err = PtcError::ExecutionTimeout(60);
        assert_eq!(err.to_string(), "Code execution timeout after 60 seconds");

        let err = PtcError::SessionNotFound("sess_123".to_string());
        assert_eq!(err.to_string(), "Session not found: sess_123");
    }
}
