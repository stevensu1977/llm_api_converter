//! Credential Pool Implementation
//!
//! This module provides the generic `CredentialPool` that manages multiple
//! credentials with load balancing and health checking.

use super::credential::Credential;
use super::strategy::{LoadBalanceStrategy, RoundRobinState, WeightedState};
use rand::prelude::*;
use std::sync::RwLock;

// ============================================================================
// Pool Configuration
// ============================================================================

/// Configuration for credential pool behavior
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Load balancing strategy
    pub strategy: LoadBalanceStrategy,
    /// Maximum failures before disabling a credential
    pub max_failures: u32,
    /// Seconds to wait before retrying a disabled credential
    pub retry_after_secs: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            strategy: LoadBalanceStrategy::RoundRobin,
            max_failures: 3,
            retry_after_secs: 300, // 5 minutes
        }
    }
}

impl PoolConfig {
    pub fn new(strategy: LoadBalanceStrategy) -> Self {
        Self {
            strategy,
            ..Default::default()
        }
    }

    pub fn with_max_failures(mut self, max: u32) -> Self {
        self.max_failures = max;
        self
    }

    pub fn with_retry_after(mut self, secs: u64) -> Self {
        self.retry_after_secs = secs;
        self
    }
}

// ============================================================================
// Credential Pool
// ============================================================================

/// A pool of credentials with load balancing support
///
/// This is the main type for managing multiple backend credentials.
/// It supports different load balancing strategies and automatic
/// health-based credential management.
#[derive(Debug)]
pub struct CredentialPool<C: Credential> {
    /// The credentials in the pool
    credentials: Vec<C>,
    /// Pool configuration
    config: PoolConfig,
    /// State for round-robin selection
    rr_state: RoundRobinState,
    /// State for weighted selection
    weighted_state: RwLock<WeightedState>,
}

impl<C: Credential> CredentialPool<C> {
    /// Create a new credential pool
    pub fn new(credentials: Vec<C>, config: PoolConfig) -> Self {
        let weights: Vec<u32> = credentials.iter().map(|c| c.weight()).collect();
        Self {
            credentials,
            config,
            rr_state: RoundRobinState::new(),
            weighted_state: RwLock::new(WeightedState::new(&weights)),
        }
    }

    /// Create a pool with a single credential (backward compatibility)
    pub fn single(credential: C) -> Self {
        Self::new(vec![credential], PoolConfig::default())
    }

    /// Create a pool with round-robin strategy
    pub fn round_robin(credentials: Vec<C>) -> Self {
        Self::new(credentials, PoolConfig::new(LoadBalanceStrategy::RoundRobin))
    }

    /// Create a pool with weighted strategy
    pub fn weighted(credentials: Vec<C>) -> Self {
        Self::new(credentials, PoolConfig::new(LoadBalanceStrategy::Weighted))
    }

    /// Create a pool with failover strategy
    pub fn failover(credentials: Vec<C>) -> Self {
        Self::new(credentials, PoolConfig::new(LoadBalanceStrategy::Failover))
    }

    /// Get the next available credential based on the load balancing strategy
    pub fn get_next(&self) -> Option<&C> {
        if self.credentials.is_empty() {
            return None;
        }

        // Get list of healthy credentials
        let healthy_indices: Vec<usize> = self
            .credentials
            .iter()
            .enumerate()
            .filter(|(_, c)| self.is_credential_available(c))
            .map(|(i, _)| i)
            .collect();

        if healthy_indices.is_empty() {
            // Try to recover a disabled credential
            return self.try_recover_credential();
        }

        let idx = match self.config.strategy {
            LoadBalanceStrategy::RoundRobin => {
                let pos = self.rr_state.next(healthy_indices.len());
                healthy_indices[pos]
            }
            LoadBalanceStrategy::Weighted => {
                // For weighted, we need to consider only healthy credentials
                let healthy_weights: Vec<u32> = healthy_indices
                    .iter()
                    .map(|&i| self.credentials[i].weight())
                    .collect();
                let total_weight: u32 = healthy_weights.iter().sum();
                if total_weight == 0 {
                    healthy_indices[0]
                } else {
                    let mut rng = thread_rng();
                    let random_weight = rng.gen_range(0..total_weight);
                    let mut cumulative = 0;
                    let mut selected = 0;
                    for (i, &weight) in healthy_weights.iter().enumerate() {
                        cumulative += weight;
                        if random_weight < cumulative {
                            selected = i;
                            break;
                        }
                    }
                    healthy_indices[selected]
                }
            }
            LoadBalanceStrategy::Random => {
                let mut rng = thread_rng();
                let pos = rng.gen_range(0..healthy_indices.len());
                healthy_indices[pos]
            }
            LoadBalanceStrategy::Failover => {
                // Always use the first available (lowest index = highest priority)
                healthy_indices[0]
            }
        };

        Some(&self.credentials[idx])
    }

    /// Get a credential by name
    pub fn get_by_name(&self, name: &str) -> Option<&C> {
        self.credentials.iter().find(|c| c.name() == name)
    }

    /// Get all credentials
    pub fn all(&self) -> &[C] {
        &self.credentials
    }

    /// Get the number of credentials
    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    /// Check if the pool is empty
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }

    /// Get the number of healthy credentials
    pub fn healthy_count(&self) -> usize {
        self.credentials
            .iter()
            .filter(|c| self.is_credential_available(c))
            .count()
    }

    /// Get the number of disabled credentials
    pub fn disabled_count(&self) -> usize {
        self.credentials.iter().filter(|c| !c.is_enabled()).count()
    }

    /// Record a successful request for a credential
    pub fn record_success(&self, name: &str) {
        if let Some(cred) = self.credentials.iter().find(|c| c.name() == name) {
            cred.record_success();
        }
    }

    /// Record a failed request for a credential
    /// Returns true if the credential was disabled due to max failures
    pub fn record_failure(&self, name: &str) -> bool {
        if let Some(cred) = self.credentials.iter().find(|c| c.name() == name) {
            cred.record_failure();
            if cred.failure_count() >= self.config.max_failures {
                cred.disable();
                tracing::warn!(
                    credential = name,
                    failures = cred.failure_count(),
                    "Credential disabled due to max failures"
                );
                return true;
            }
        }
        false
    }

    /// Manually disable a credential
    pub fn disable(&self, name: &str) {
        if let Some(cred) = self.credentials.iter().find(|c| c.name() == name) {
            cred.disable();
        }
    }

    /// Manually enable a credential
    pub fn enable(&self, name: &str) {
        if let Some(cred) = self.credentials.iter().find(|c| c.name() == name) {
            cred.enable();
            cred.reset_health();
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            total: self.credentials.len(),
            healthy: self.healthy_count(),
            disabled: self.disabled_count(),
            strategy: self.config.strategy,
        }
    }

    /// Check if a credential is available (enabled and not at max failures)
    fn is_credential_available(&self, cred: &C) -> bool {
        if !cred.is_enabled() {
            // Disabled credentials are not available
            // They can only be re-enabled via try_recover_credential or manual enable()
            return false;
        }
        cred.failure_count() < self.config.max_failures
    }

    /// Try to recover a disabled credential for use
    fn try_recover_credential(&self) -> Option<&C> {
        // Find a disabled credential that's ready for retry
        for cred in &self.credentials {
            if !cred.is_enabled() && cred.health().should_retry(self.config.retry_after_secs) {
                tracing::info!(
                    credential = cred.name(),
                    "Attempting to recover disabled credential"
                );
                cred.enable();
                return Some(cred);
            }
        }
        // Last resort: return the first credential even if it's unhealthy
        self.credentials.first()
    }
}

// ============================================================================
// Pool Statistics
// ============================================================================

/// Statistics about a credential pool
#[derive(Debug, Clone)]
pub struct PoolStats {
    /// Total number of credentials
    pub total: usize,
    /// Number of healthy credentials
    pub healthy: usize,
    /// Number of disabled credentials
    pub disabled: usize,
    /// Current load balancing strategy
    pub strategy: LoadBalanceStrategy,
}

impl PoolStats {
    /// Check if the pool is healthy (at least one credential available)
    pub fn is_healthy(&self) -> bool {
        self.healthy > 0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::super::credential::ApiKeyCredential;
    use super::*;

    fn create_test_credentials() -> Vec<ApiKeyCredential> {
        vec![
            ApiKeyCredential::new("key1", "primary", 2),
            ApiKeyCredential::new("key2", "secondary", 1),
            ApiKeyCredential::new("key3", "backup", 1),
        ]
    }

    #[test]
    fn test_single_credential_pool() {
        let cred = ApiKeyCredential::new("single-key", "default", 1);
        let pool = CredentialPool::single(cred);

        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());

        let selected = pool.get_next().unwrap();
        assert_eq!(selected.name(), "default");
    }

    #[test]
    fn test_round_robin_selection() {
        let pool = CredentialPool::round_robin(create_test_credentials());

        // Should cycle through all credentials
        let names: Vec<&str> = (0..6).map(|_| pool.get_next().unwrap().name()).collect();

        // Should see each credential at least once
        assert!(names.contains(&"primary"));
        assert!(names.contains(&"secondary"));
        assert!(names.contains(&"backup"));
    }

    #[test]
    fn test_failover_selection() {
        let pool = CredentialPool::failover(create_test_credentials());

        // Should always return the first credential
        for _ in 0..5 {
            let selected = pool.get_next().unwrap();
            assert_eq!(selected.name(), "primary");
        }

        // Disable the first credential
        pool.disable("primary");

        // Should now return the second credential
        let selected = pool.get_next().unwrap();
        assert_eq!(selected.name(), "secondary");
    }

    #[test]
    fn test_record_failure_and_disable() {
        let pool = CredentialPool::new(
            create_test_credentials(),
            PoolConfig::new(LoadBalanceStrategy::Failover).with_max_failures(2),
        );

        // First failure
        assert!(!pool.record_failure("primary"));
        assert_eq!(pool.get_by_name("primary").unwrap().failure_count(), 1);

        // Second failure - should disable
        assert!(pool.record_failure("primary"));
        assert!(!pool.get_by_name("primary").unwrap().is_enabled());

        // Pool should now use secondary
        let selected = pool.get_next().unwrap();
        assert_eq!(selected.name(), "secondary");
    }

    #[test]
    fn test_record_success_resets_failures() {
        let pool = CredentialPool::round_robin(create_test_credentials());

        pool.record_failure("primary");
        pool.record_failure("primary");
        assert_eq!(pool.get_by_name("primary").unwrap().failure_count(), 2);

        pool.record_success("primary");
        assert_eq!(pool.get_by_name("primary").unwrap().failure_count(), 0);
    }

    #[test]
    fn test_pool_stats() {
        let pool = CredentialPool::round_robin(create_test_credentials());
        let stats = pool.stats();

        assert_eq!(stats.total, 3);
        assert_eq!(stats.healthy, 3);
        assert_eq!(stats.disabled, 0);
        assert!(stats.is_healthy());

        pool.disable("primary");
        let stats = pool.stats();
        assert_eq!(stats.disabled, 1);
        assert_eq!(stats.healthy, 2);
    }

    #[test]
    fn test_get_by_name() {
        let pool = CredentialPool::round_robin(create_test_credentials());

        let cred = pool.get_by_name("secondary").unwrap();
        assert_eq!(cred.api_key(), "key2");

        assert!(pool.get_by_name("nonexistent").is_none());
    }
}
