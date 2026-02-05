//! Credential types and trait definitions
//!
//! This module defines the `Credential` trait and common credential implementations
//! for different backend types.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;

// ============================================================================
// Credential Health
// ============================================================================

/// Health status for a credential
#[derive(Debug)]
pub struct CredentialHealth {
    /// Whether the credential is currently enabled
    enabled: AtomicBool,
    /// Number of consecutive failures
    failure_count: AtomicU32,
    /// Last failure timestamp (for recovery)
    last_failure: std::sync::Mutex<Option<Instant>>,
    /// Last success timestamp
    last_success: std::sync::Mutex<Option<Instant>>,
}

impl Default for CredentialHealth {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialHealth {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(true),
            failure_count: AtomicU32::new(0),
            last_failure: std::sync::Mutex::new(None),
            last_success: std::sync::Mutex::new(None),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::SeqCst)
    }

    pub fn record_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut last) = self.last_failure.lock() {
            *last = Some(Instant::now());
        }
    }

    pub fn record_success(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        if let Ok(mut last) = self.last_success.lock() {
            *last = Some(Instant::now());
        }
    }

    pub fn reset(&self) {
        self.failure_count.store(0, Ordering::SeqCst);
        self.enabled.store(true, Ordering::SeqCst);
    }

    /// Check if enough time has passed since last failure for retry
    pub fn should_retry(&self, retry_after_secs: u64) -> bool {
        if let Ok(last) = self.last_failure.lock() {
            if let Some(instant) = *last {
                return instant.elapsed().as_secs() >= retry_after_secs;
            }
        }
        true
    }
}

// ============================================================================
// Credential Trait
// ============================================================================

/// Trait for backend credentials
pub trait Credential: Send + Sync {
    /// Get the credential name/identifier
    fn name(&self) -> &str;

    /// Get the weight for weighted load balancing
    fn weight(&self) -> u32;

    /// Get health status
    fn health(&self) -> &CredentialHealth;

    /// Check if credential is enabled
    fn is_enabled(&self) -> bool {
        self.health().is_enabled()
    }

    /// Record a successful request
    fn record_success(&self) {
        self.health().record_success();
    }

    /// Record a failed request
    fn record_failure(&self) {
        self.health().record_failure();
    }

    /// Get failure count
    fn failure_count(&self) -> u32 {
        self.health().failure_count()
    }

    /// Disable the credential
    fn disable(&self) {
        self.health().set_enabled(false);
    }

    /// Enable the credential
    fn enable(&self) {
        self.health().set_enabled(true);
    }

    /// Reset health status
    fn reset_health(&self) {
        self.health().reset();
    }
}

// ============================================================================
// API Key Credential
// ============================================================================

/// Simple API key credential for services like Gemini, OpenAI, DeepSeek, Anthropic
#[derive(Debug)]
pub struct ApiKeyCredential {
    /// Credential name for identification
    name: String,
    /// The API key
    api_key: String,
    /// Weight for load balancing
    weight: u32,
    /// Health status
    health: CredentialHealth,
    /// Optional organization ID (for OpenAI)
    organization: Option<String>,
    /// Optional base URL override
    base_url: Option<String>,
}

impl ApiKeyCredential {
    /// Create a new API key credential
    pub fn new(api_key: impl Into<String>, name: impl Into<String>, weight: u32) -> Self {
        Self {
            name: name.into(),
            api_key: api_key.into(),
            weight,
            health: CredentialHealth::new(),
            organization: None,
            base_url: None,
        }
    }

    /// Create with default weight of 1
    pub fn with_key(api_key: impl Into<String>, name: impl Into<String>) -> Self {
        Self::new(api_key, name, 1)
    }

    /// Set organization ID
    pub fn with_organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }

    /// Set base URL override
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Get the API key
    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    /// Get the organization ID
    pub fn organization(&self) -> Option<&str> {
        self.organization.as_deref()
    }

    /// Get the base URL override
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }
}

impl Credential for ApiKeyCredential {
    fn name(&self) -> &str {
        &self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn health(&self) -> &CredentialHealth {
        &self.health
    }
}

// ============================================================================
// AWS Credential
// ============================================================================

/// AWS credential for Bedrock service
#[derive(Debug)]
pub struct AwsCredential {
    /// Credential name for identification
    name: String,
    /// AWS region
    region: String,
    /// AWS profile name (if using profile-based auth)
    profile: Option<String>,
    /// Access key ID (if using access key auth)
    access_key_id: Option<String>,
    /// Secret access key (if using access key auth)
    secret_access_key: Option<String>,
    /// Session token (for temporary credentials)
    session_token: Option<String>,
    /// Weight for load balancing
    weight: u32,
    /// Health status
    health: CredentialHealth,
}

impl AwsCredential {
    /// Create a new AWS credential with profile
    pub fn with_profile(
        profile: impl Into<String>,
        region: impl Into<String>,
        name: impl Into<String>,
        weight: u32,
    ) -> Self {
        Self {
            name: name.into(),
            region: region.into(),
            profile: Some(profile.into()),
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
            weight,
            health: CredentialHealth::new(),
        }
    }

    /// Create a new AWS credential with access keys
    pub fn with_access_key(
        access_key_id: impl Into<String>,
        secret_access_key: impl Into<String>,
        region: impl Into<String>,
        name: impl Into<String>,
        weight: u32,
    ) -> Self {
        Self {
            name: name.into(),
            region: region.into(),
            profile: None,
            access_key_id: Some(access_key_id.into()),
            secret_access_key: Some(secret_access_key.into()),
            session_token: None,
            weight,
            health: CredentialHealth::new(),
        }
    }

    /// Create a default credential (uses environment/instance role)
    pub fn default_credential(region: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            region: region.into(),
            profile: None,
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
            weight: 1,
            health: CredentialHealth::new(),
        }
    }

    /// Set session token for temporary credentials
    pub fn with_session_token(mut self, token: impl Into<String>) -> Self {
        self.session_token = Some(token.into());
        self
    }

    /// Get the region
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Get the profile name
    pub fn profile(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// Get the access key ID
    pub fn access_key_id(&self) -> Option<&str> {
        self.access_key_id.as_deref()
    }

    /// Get the secret access key
    pub fn secret_access_key(&self) -> Option<&str> {
        self.secret_access_key.as_deref()
    }

    /// Get the session token
    pub fn session_token(&self) -> Option<&str> {
        self.session_token.as_deref()
    }

    /// Check if this uses profile-based auth
    pub fn uses_profile(&self) -> bool {
        self.profile.is_some()
    }

    /// Check if this uses access key auth
    pub fn uses_access_key(&self) -> bool {
        self.access_key_id.is_some() && self.secret_access_key.is_some()
    }

    /// Check if this uses default credentials (env/instance role)
    pub fn uses_default(&self) -> bool {
        !self.uses_profile() && !self.uses_access_key()
    }
}

impl Credential for AwsCredential {
    fn name(&self) -> &str {
        &self.name
    }

    fn weight(&self) -> u32 {
        self.weight
    }

    fn health(&self) -> &CredentialHealth {
        &self.health
    }
}

// ============================================================================
// Configuration Structures (for deserialization)
// ============================================================================

/// Configuration for API key credentials
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiKeyCredentialConfig {
    pub api_key: String,
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub enabled: Option<bool>,
    pub organization: Option<String>,
    pub base_url: Option<String>,
}

/// Configuration for AWS credentials
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AwsCredentialConfig {
    #[serde(default = "default_name")]
    pub name: String,
    pub region: String,
    pub profile: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_access_key: Option<String>,
    pub session_token: Option<String>,
    #[serde(default = "default_weight")]
    pub weight: u32,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn default_name() -> String {
    "default".to_string()
}

fn default_weight() -> u32 {
    1
}

impl From<ApiKeyCredentialConfig> for ApiKeyCredential {
    fn from(config: ApiKeyCredentialConfig) -> Self {
        let mut cred = ApiKeyCredential::new(config.api_key, config.name, config.weight);
        if let Some(org) = config.organization {
            cred = cred.with_organization(org);
        }
        if let Some(url) = config.base_url {
            cred = cred.with_base_url(url);
        }
        if config.enabled == Some(false) {
            cred.disable();
        }
        cred
    }
}

impl From<AwsCredentialConfig> for AwsCredential {
    fn from(config: AwsCredentialConfig) -> Self {
        let cred = if let Some(profile) = config.profile {
            AwsCredential::with_profile(profile, config.region, config.name, config.weight)
        } else if let (Some(key_id), Some(secret)) =
            (config.access_key_id, config.secret_access_key)
        {
            let mut cred =
                AwsCredential::with_access_key(key_id, secret, config.region, config.name, config.weight);
            if let Some(token) = config.session_token {
                cred = cred.with_session_token(token);
            }
            cred
        } else {
            AwsCredential::default_credential(config.region, config.name)
        };

        if config.enabled == Some(false) {
            cred.disable();
        }
        cred
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_credential() {
        let cred = ApiKeyCredential::new("test-key", "primary", 2);
        assert_eq!(cred.name(), "primary");
        assert_eq!(cred.api_key(), "test-key");
        assert_eq!(cred.weight(), 2);
        assert!(cred.is_enabled());
    }

    #[test]
    fn test_credential_health() {
        let cred = ApiKeyCredential::new("test-key", "test", 1);
        assert_eq!(cred.failure_count(), 0);

        cred.record_failure();
        assert_eq!(cred.failure_count(), 1);

        cred.record_failure();
        assert_eq!(cred.failure_count(), 2);

        cred.record_success();
        assert_eq!(cred.failure_count(), 0);
    }

    #[test]
    fn test_credential_enable_disable() {
        let cred = ApiKeyCredential::new("test-key", "test", 1);
        assert!(cred.is_enabled());

        cred.disable();
        assert!(!cred.is_enabled());

        cred.enable();
        assert!(cred.is_enabled());
    }

    #[test]
    fn test_aws_credential_with_profile() {
        let cred = AwsCredential::with_profile("my-profile", "us-east-1", "primary", 2);
        assert_eq!(cred.name(), "primary");
        assert_eq!(cred.region(), "us-east-1");
        assert_eq!(cred.profile(), Some("my-profile"));
        assert!(cred.uses_profile());
        assert!(!cred.uses_access_key());
    }

    #[test]
    fn test_aws_credential_with_access_key() {
        let cred = AwsCredential::with_access_key("AKIA...", "secret", "us-west-2", "backup", 1);
        assert_eq!(cred.name(), "backup");
        assert_eq!(cred.region(), "us-west-2");
        assert!(cred.uses_access_key());
        assert!(!cred.uses_profile());
    }
}
