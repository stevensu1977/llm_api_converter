//! Database module
//!
//! Contains storage backend abstraction and implementations.

pub mod dynamodb;
pub mod dynamodb_backend;
pub mod models;
pub mod repositories;
pub mod storage;

#[cfg(feature = "sqlite")]
pub mod sqlite_backend;

pub use dynamodb::DynamoDbClient;
pub use dynamodb_backend::DynamoDbBackend;
pub use models::{ApiKey, ModelMapping, ModelPricing, UsageRecord, UsageStats};
pub use repositories::{
    ApiKeyError, ApiKeyRepository, ModelMappingError, ModelMappingRepository, UsageError,
    UsageRepository,
};
pub use storage::{StorageBackend, StorageError};

#[cfg(feature = "sqlite")]
pub use sqlite_backend::SqliteBackend;
