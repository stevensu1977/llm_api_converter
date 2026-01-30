//! Docker sandbox executor for PTC
//!
//! This module handles the creation and management of Docker containers
//! for secure code execution in the PTC (Programmatic Tool Calling) system.

use super::exceptions::{PtcError, PtcResult};
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, LogsOptions, RemoveContainerOptions,
    StartContainerOptions, StopContainerOptions, UploadToContainerOptions,
};
use bollard::exec::{CreateExecOptions, StartExecResults};
use bollard::Docker;
use futures::StreamExt;
use std::time::Duration;
use tokio::time::timeout;

// ============================================================================
// Configuration
// ============================================================================

/// Default Docker image for sandbox execution
pub const DEFAULT_SANDBOX_IMAGE: &str = "python:3.11-slim";

/// Default memory limit for containers (256MB)
pub const DEFAULT_MEMORY_LIMIT: i64 = 256 * 1024 * 1024;

/// Default CPU period (100ms)
pub const DEFAULT_CPU_PERIOD: i64 = 100_000;

/// Default CPU quota (50% of one core)
pub const DEFAULT_CPU_QUOTA: i64 = 50_000;

/// Default execution timeout in seconds
pub const DEFAULT_EXECUTION_TIMEOUT: u64 = 60;

/// Default session timeout in seconds (4.5 minutes)
pub const DEFAULT_SESSION_TIMEOUT: u64 = 270;

// ============================================================================
// Sandbox Configuration
// ============================================================================

/// Configuration for the Docker sandbox
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    /// Docker image to use
    pub image: String,
    /// Memory limit in bytes
    pub memory_limit: i64,
    /// CPU period in microseconds
    pub cpu_period: i64,
    /// CPU quota in microseconds
    pub cpu_quota: i64,
    /// Execution timeout in seconds
    pub execution_timeout: u64,
    /// Whether network is disabled
    pub network_disabled: bool,
    /// Working directory in container
    pub working_dir: String,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            image: DEFAULT_SANDBOX_IMAGE.to_string(),
            memory_limit: DEFAULT_MEMORY_LIMIT,
            cpu_period: DEFAULT_CPU_PERIOD,
            cpu_quota: DEFAULT_CPU_QUOTA,
            execution_timeout: DEFAULT_EXECUTION_TIMEOUT,
            network_disabled: true,
            working_dir: "/tmp".to_string(),
        }
    }
}

// ============================================================================
// Container Info
// ============================================================================

/// Information about a running container
#[derive(Debug, Clone)]
pub struct ContainerInfo {
    /// Container ID
    pub id: String,
    /// Container name
    pub name: String,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Whether the container is running
    pub running: bool,
}

// ============================================================================
// Execution Result
// ============================================================================

/// Result of code execution in the sandbox
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Exit code
    pub exit_code: i64,
    /// Whether execution timed out
    pub timed_out: bool,
}

impl ExecutionResult {
    /// Check if execution was successful
    pub fn is_success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

// ============================================================================
// Sandbox Executor
// ============================================================================

/// Docker sandbox executor for secure code execution
pub struct SandboxExecutor {
    /// Docker client
    docker: Docker,
    /// Sandbox configuration
    config: SandboxConfig,
}

impl SandboxExecutor {
    /// Create a new sandbox executor with default configuration
    pub async fn new() -> PtcResult<Self> {
        Self::with_config(SandboxConfig::default()).await
    }

    /// Create a new sandbox executor with custom configuration
    pub async fn with_config(config: SandboxConfig) -> PtcResult<Self> {
        let docker = Docker::connect_with_local_defaults()
            .map_err(|e| PtcError::DockerNotAvailable(e.to_string()))?;

        // Test Docker connection
        docker
            .ping()
            .await
            .map_err(|e| PtcError::DockerNotAvailable(format!("Failed to ping Docker: {}", e)))?;

        Ok(Self { docker, config })
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        self.docker.ping().await.is_ok()
    }

    /// Get Docker version info
    pub async fn version(&self) -> PtcResult<String> {
        let version = self
            .docker
            .version()
            .await
            .map_err(|e| PtcError::DockerNotAvailable(e.to_string()))?;

        Ok(format!(
            "Docker {} (API {})",
            version.version.unwrap_or_default(),
            version.api_version.unwrap_or_default()
        ))
    }

    // ========================================================================
    // Container Lifecycle
    // ========================================================================

    /// Create a new container for code execution
    pub async fn create_container(&self, name: Option<&str>) -> PtcResult<ContainerInfo> {
        // Generate container name if not provided
        let container_name = name
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("ptc_sandbox_{}", uuid::Uuid::new_v4()));

        // Build container config
        let host_config = bollard::service::HostConfig {
            memory: Some(self.config.memory_limit),
            cpu_period: Some(self.config.cpu_period),
            cpu_quota: Some(self.config.cpu_quota),
            network_mode: if self.config.network_disabled {
                Some("none".to_string())
            } else {
                None
            },
            security_opt: Some(vec!["no-new-privileges".to_string()]),
            cap_drop: Some(vec!["ALL".to_string()]),
            ..Default::default()
        };

        let config = Config {
            image: Some(self.config.image.clone()),
            working_dir: Some(self.config.working_dir.clone()),
            host_config: Some(host_config),
            tty: Some(true),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            open_stdin: Some(true),
            // Keep container running with a simple command
            cmd: Some(vec!["tail".to_string(), "-f".to_string(), "/dev/null".to_string()]),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: container_name.as_str(),
            platform: None,
        };

        // Create the container
        let response = self
            .docker
            .create_container(Some(options), config)
            .await
            .map_err(|e| PtcError::ContainerCreationFailed(e.to_string()))?;

        Ok(ContainerInfo {
            id: response.id,
            name: container_name,
            created_at: chrono::Utc::now(),
            running: false,
        })
    }

    /// Start a container
    pub async fn start_container(&self, container_id: &str) -> PtcResult<()> {
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .map_err(|e| PtcError::ContainerStartFailed(e.to_string()))?;

        Ok(())
    }

    /// Stop a container
    pub async fn stop_container(&self, container_id: &str) -> PtcResult<()> {
        let options = StopContainerOptions { t: 5 }; // 5 second timeout

        self.docker
            .stop_container(container_id, Some(options))
            .await
            .map_err(|e| PtcError::Internal(format!("Failed to stop container: {}", e)))?;

        Ok(())
    }

    /// Remove a container
    pub async fn remove_container(&self, container_id: &str) -> PtcResult<()> {
        let options = RemoveContainerOptions {
            force: true,
            ..Default::default()
        };

        self.docker
            .remove_container(container_id, Some(options))
            .await
            .map_err(|e| PtcError::Internal(format!("Failed to remove container: {}", e)))?;

        Ok(())
    }

    /// Get container logs
    pub async fn get_logs(&self, container_id: &str) -> PtcResult<(String, String)> {
        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: "all".to_string(),
            ..Default::default()
        };

        let mut stdout = String::new();
        let mut stderr = String::new();

        let mut stream = self.docker.logs(container_id, Some(options));

        while let Some(result) = stream.next().await {
            match result {
                Ok(LogOutput::StdOut { message }) => {
                    stdout.push_str(&String::from_utf8_lossy(&message));
                }
                Ok(LogOutput::StdErr { message }) => {
                    stderr.push_str(&String::from_utf8_lossy(&message));
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::warn!("Error reading container logs: {}", e);
                }
            }
        }

        Ok((stdout, stderr))
    }

    // ========================================================================
    // File Operations
    // ========================================================================

    /// Copy a file to the container using put_archive API
    ///
    /// This method creates a tar archive and uploads it to the container,
    /// which works correctly in Docker-in-Docker scenarios where bind mounts fail.
    pub async fn copy_file_to_container(
        &self,
        container_id: &str,
        content: &[u8],
        dest_path: &str,
    ) -> PtcResult<()> {
        // Create a tar archive containing the file
        let mut tar_buffer = Vec::new();
        {
            let mut tar_builder = tar::Builder::new(&mut tar_buffer);

            // Extract filename from dest_path
            let filename = std::path::Path::new(dest_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file");

            // Create tar header
            let mut header = tar::Header::new_gnu();
            header.set_path(filename).map_err(|e| {
                PtcError::FileCopyFailed(format!("Failed to set tar path: {}", e))
            })?;
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();

            // Add file to archive
            tar_builder.append(&header, content).map_err(|e| {
                PtcError::FileCopyFailed(format!("Failed to append to tar: {}", e))
            })?;

            tar_builder.finish().map_err(|e| {
                PtcError::FileCopyFailed(format!("Failed to finish tar: {}", e))
            })?;
        }

        // Get the directory path
        let dir_path = std::path::Path::new(dest_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/tmp".to_string());

        // Upload the tar archive to the container
        let options = UploadToContainerOptions {
            path: dir_path,
            ..Default::default()
        };

        self.docker
            .upload_to_container(container_id, Some(options), tar_buffer.into())
            .await
            .map_err(|e| PtcError::FileCopyFailed(format!("Failed to upload to container: {}", e)))?;

        Ok(())
    }

    // ========================================================================
    // Code Execution
    // ========================================================================

    /// Execute a command in the container
    pub async fn exec_command(
        &self,
        container_id: &str,
        command: Vec<&str>,
    ) -> PtcResult<ExecutionResult> {
        self.exec_command_with_timeout(container_id, command, self.config.execution_timeout)
            .await
    }

    /// Execute a command with custom timeout
    pub async fn exec_command_with_timeout(
        &self,
        container_id: &str,
        command: Vec<&str>,
        timeout_secs: u64,
    ) -> PtcResult<ExecutionResult> {
        let exec_config = CreateExecOptions {
            cmd: Some(command.iter().map(|s| s.to_string()).collect()),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            working_dir: Some(self.config.working_dir.clone()),
            ..Default::default()
        };

        // Create exec instance
        let exec = self
            .docker
            .create_exec(container_id, exec_config)
            .await
            .map_err(|e| PtcError::ExecFailed(format!("Failed to create exec: {}", e)))?;

        // Start exec and collect output with timeout
        let exec_result = timeout(
            Duration::from_secs(timeout_secs),
            self.collect_exec_output(&exec.id),
        )
        .await;

        match exec_result {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(ExecutionResult {
                stdout: String::new(),
                stderr: "Execution timed out".to_string(),
                exit_code: -1,
                timed_out: true,
            }),
        }
    }

    /// Execute Python code in the container
    pub async fn execute_python(
        &self,
        container_id: &str,
        code: &str,
    ) -> PtcResult<ExecutionResult> {
        // Write code to a temporary file
        let script_path = "/tmp/script.py";
        self.copy_file_to_container(container_id, code.as_bytes(), script_path)
            .await?;

        // Execute the script
        self.exec_command(container_id, vec!["python", script_path])
            .await
    }

    /// Collect output from an exec instance
    async fn collect_exec_output(&self, exec_id: &str) -> PtcResult<ExecutionResult> {
        let start_result = self
            .docker
            .start_exec(exec_id, None)
            .await
            .map_err(|e| PtcError::ExecFailed(format!("Failed to start exec: {}", e)))?;

        let mut stdout = String::new();
        let mut stderr = String::new();

        if let StartExecResults::Attached { mut output, .. } = start_result {
            while let Some(result) = output.next().await {
                match result {
                    Ok(LogOutput::StdOut { message }) => {
                        stdout.push_str(&String::from_utf8_lossy(&message));
                    }
                    Ok(LogOutput::StdErr { message }) => {
                        stderr.push_str(&String::from_utf8_lossy(&message));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Error reading exec output: {}", e);
                    }
                }
            }
        }

        // Get exit code
        let inspect = self
            .docker
            .inspect_exec(exec_id)
            .await
            .map_err(|e| PtcError::ExecFailed(format!("Failed to inspect exec: {}", e)))?;

        let exit_code = inspect.exit_code.unwrap_or(-1);

        Ok(ExecutionResult {
            stdout,
            stderr,
            exit_code,
            timed_out: false,
        })
    }

    // ========================================================================
    // Convenience Methods
    // ========================================================================

    /// Create and start a container in one step
    pub async fn create_and_start(&self, name: Option<&str>) -> PtcResult<ContainerInfo> {
        let mut info = self.create_container(name).await?;
        self.start_container(&info.id).await?;
        info.running = true;
        Ok(info)
    }

    /// Stop and remove a container in one step
    pub async fn stop_and_remove(&self, container_id: &str) -> PtcResult<()> {
        // Try to stop first, but don't fail if already stopped
        let _ = self.stop_container(container_id).await;
        self.remove_container(container_id).await
    }

    /// Check if a container exists
    pub async fn container_exists(&self, container_id: &str) -> bool {
        self.docker.inspect_container(container_id, None).await.is_ok()
    }

    /// Check if a container is running
    pub async fn is_container_running(&self, container_id: &str) -> bool {
        if let Ok(info) = self.docker.inspect_container(container_id, None).await {
            info.state
                .and_then(|s| s.running)
                .unwrap_or(false)
        } else {
            false
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert_eq!(config.image, DEFAULT_SANDBOX_IMAGE);
        assert_eq!(config.memory_limit, DEFAULT_MEMORY_LIMIT);
        assert!(config.network_disabled);
    }

    #[test]
    fn test_execution_result_success() {
        let result = ExecutionResult {
            stdout: "output".to_string(),
            stderr: String::new(),
            exit_code: 0,
            timed_out: false,
        };
        assert!(result.is_success());
    }

    #[test]
    fn test_execution_result_failure() {
        let result = ExecutionResult {
            stdout: String::new(),
            stderr: "error".to_string(),
            exit_code: 1,
            timed_out: false,
        };
        assert!(!result.is_success());
    }

    #[test]
    fn test_execution_result_timeout() {
        let result = ExecutionResult {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: 0,
            timed_out: true,
        };
        assert!(!result.is_success());
    }

    #[test]
    fn test_container_info() {
        let info = ContainerInfo {
            id: "abc123".to_string(),
            name: "test_container".to_string(),
            created_at: chrono::Utc::now(),
            running: true,
        };
        assert_eq!(info.id, "abc123");
        assert!(info.running);
    }
}
