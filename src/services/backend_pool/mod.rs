//! Backend Pool Module
//!
//! This module provides a generic framework for managing multiple backend credentials
//! with load balancing, health checking, and automatic failover support.
//!
//! # Features
//! - Generic credential pool supporting any backend type
//! - Multiple load balancing strategies (RoundRobin, Weighted, Random, Failover)
//! - Automatic health checking and credential disabling
//! - Backward compatible with single-credential configurations
//!
//! # Example
//! ```ignore
//! use backend_pool::{CredentialPool, LoadBalanceStrategy, ApiKeyCredential};
//!
//! // Create credentials
//! let creds = vec![
//!     ApiKeyCredential::new("key1", "primary", 2),
//!     ApiKeyCredential::new("key2", "backup", 1),
//! ];
//!
//! // Create pool with weighted strategy
//! let pool = CredentialPool::new(creds, LoadBalanceStrategy::Weighted);
//!
//! // Get next credential
//! if let Some(cred) = pool.get_next() {
//!     println!("Using credential: {}", cred.name());
//! }
//! ```

mod credential;
mod pool;
mod strategy;

pub use credential::{ApiKeyCredential, AwsCredential, Credential, CredentialHealth};
pub use pool::{CredentialPool, PoolConfig, PoolStats};
pub use strategy::LoadBalanceStrategy;
