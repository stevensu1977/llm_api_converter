//! Configuration management module
//!
//! This module handles loading and validating application configuration
//! from environment variables and .env files.

pub mod aws;
pub mod settings;

pub use aws::{
    build_aws_config, create_bedrock_client, create_bedrock_client_with_profile,
    create_dynamodb_client, AwsConfigBuilder,
};
pub use settings::{
    BackendPoolConfig, BedrockConfig, BedrockProfileConfig, Environment, FeatureFlags,
    GeminiConfig, PtcConfig, RateLimitConfig, Settings,
};
