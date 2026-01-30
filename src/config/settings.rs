//! Application settings and configuration
//!
//! This module provides configuration management for the application,
//! loading settings from environment variables with sensible defaults.

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fmt;

/// Application environment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[value(alias = "dev")]
    Development,
    #[value(alias = "stage")]
    Staging,
    #[value(alias = "prod")]
    Production,
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Environment::Development => write!(f, "development"),
            Environment::Staging => write!(f, "staging"),
            Environment::Production => write!(f, "production"),
        }
    }
}

impl Default for Environment {
    fn default() -> Self {
        Environment::Development
    }
}

impl std::str::FromStr for Environment {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "development" | "dev" => Ok(Environment::Development),
            "staging" | "stage" => Ok(Environment::Staging),
            "production" | "prod" => Ok(Environment::Production),
            _ => anyhow::bail!("Invalid environment: {}. Expected: development, staging, or production", s),
        }
    }
}

/// Rate limiting configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_window: u32,
    pub window_seconds: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_window: 100,
            window_seconds: 60,
        }
    }
}

/// Feature flags configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeatureFlags {
    pub enable_tool_use: bool,
    pub enable_ptc: bool,
    pub enable_extended_thinking: bool,
    pub enable_document_support: bool,
    pub prompt_caching_enabled: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self {
            enable_tool_use: true,
            enable_ptc: false,
            enable_extended_thinking: true,
            enable_document_support: true,
            prompt_caching_enabled: false,
        }
    }
}

/// PTC (Programmatic Tool Calling) configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PtcConfig {
    pub sandbox_image: String,
    pub session_timeout_seconds: u64,
    pub execution_timeout_seconds: u64,
    pub memory_limit: String,
    pub network_disabled: bool,
}

impl Default for PtcConfig {
    fn default() -> Self {
        Self {
            sandbox_image: "python:3.11-slim".to_string(),
            session_timeout_seconds: 270, // 4.5 minutes
            execution_timeout_seconds: 60,
            memory_limit: "256m".to_string(),
            network_disabled: true,
        }
    }
}

/// Main application settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    // App settings
    pub app_name: String,
    pub app_version: String,
    pub environment: Environment,
    pub log_level: String,

    // Server settings
    pub host: String,
    pub port: u16,

    // AWS settings
    pub aws_region: String,
    #[serde(skip_serializing)]
    pub aws_access_key_id: Option<String>,
    #[serde(skip_serializing)]
    pub aws_secret_access_key: Option<String>,
    pub dynamodb_endpoint_url: Option<String>,
    pub bedrock_endpoint_url: Option<String>,

    // DynamoDB table names
    pub dynamodb_api_keys_table: String,
    pub dynamodb_usage_table: String,
    pub dynamodb_usage_stats_table: String,
    pub dynamodb_model_mapping_table: String,
    pub dynamodb_model_pricing_table: String,

    // Authentication
    pub require_api_key: bool,
    #[serde(skip_serializing)]
    pub master_api_key: Option<String>,

    // Rate limiting
    pub rate_limit: RateLimitConfig,

    // Feature flags
    pub features: FeatureFlags,

    // PTC configuration
    pub ptc: PtcConfig,

    // Model mapping (Anthropic model ID -> Bedrock model ID)
    pub default_model_mapping: HashMap<String, String>,

    // Streaming configuration
    pub streaming_timeout_seconds: u64,

    // Debug options
    /// Print all request prompts to stdout
    #[serde(default)]
    pub print_prompts: bool,

    /// Ephemeral API key (generated at startup, not stored in DynamoDB)
    /// This is used for simple local development without DynamoDB
    #[serde(skip)]
    pub ephemeral_api_key: Option<String>,
}

impl Settings {
    /// Load settings from environment variables with defaults
    pub fn load() -> Result<Self> {
        // Load .env file if it exists (ignored in production typically)
        dotenvy::dotenv().ok();

        let settings = Self {
            // App settings
            app_name: env_or_default("APP_NAME", "anthropic-bedrock-proxy"),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: env_or_default("ENVIRONMENT", "development")
                .parse()
                .unwrap_or_default(),
            log_level: env_or_default("LOG_LEVEL", "info"),

            // Server settings
            host: env_or_default("HOST", "0.0.0.0"),
            port: env_or_default("PORT", "8000")
                .parse()
                .context("Invalid PORT value")?,

            // AWS settings
            aws_region: env_or_default("AWS_REGION", "us-east-1"),
            aws_access_key_id: env::var("AWS_ACCESS_KEY_ID").ok(),
            aws_secret_access_key: env::var("AWS_SECRET_ACCESS_KEY").ok(),
            dynamodb_endpoint_url: env::var("DYNAMODB_ENDPOINT_URL").ok(),
            bedrock_endpoint_url: env::var("BEDROCK_ENDPOINT_URL").ok(),

            // DynamoDB table names
            dynamodb_api_keys_table: env_or_default(
                "DYNAMODB_API_KEYS_TABLE",
                "anthropic-proxy-api-keys",
            ),
            dynamodb_usage_table: env_or_default(
                "DYNAMODB_USAGE_TABLE",
                "anthropic-proxy-usage",
            ),
            dynamodb_usage_stats_table: env_or_default(
                "DYNAMODB_USAGE_STATS_TABLE",
                "anthropic-proxy-usage-stats",
            ),
            dynamodb_model_mapping_table: env_or_default(
                "DYNAMODB_MODEL_MAPPING_TABLE",
                "anthropic-proxy-model-mapping",
            ),
            dynamodb_model_pricing_table: env_or_default(
                "DYNAMODB_MODEL_PRICING_TABLE",
                "anthropic-proxy-model-pricing",
            ),

            // Authentication
            require_api_key: env_or_default("REQUIRE_API_KEY", "true")
                .parse()
                .unwrap_or(true),
            master_api_key: env::var("MASTER_API_KEY").ok(),

            // Rate limiting
            rate_limit: RateLimitConfig {
                enabled: env_or_default("RATE_LIMIT_ENABLED", "true")
                    .parse()
                    .unwrap_or(true),
                requests_per_window: env_or_default("RATE_LIMIT_REQUESTS_PER_WINDOW", "100")
                    .parse()
                    .unwrap_or(100),
                window_seconds: env_or_default("RATE_LIMIT_WINDOW_SECONDS", "60")
                    .parse()
                    .unwrap_or(60),
            },

            // Feature flags
            features: FeatureFlags {
                enable_tool_use: env_or_default("ENABLE_TOOL_USE", "true")
                    .parse()
                    .unwrap_or(true),
                enable_ptc: env_or_default("ENABLE_PTC", "false")
                    .parse()
                    .unwrap_or(false),
                enable_extended_thinking: env_or_default("ENABLE_EXTENDED_THINKING", "true")
                    .parse()
                    .unwrap_or(true),
                enable_document_support: env_or_default("ENABLE_DOCUMENT_SUPPORT", "true")
                    .parse()
                    .unwrap_or(true),
                prompt_caching_enabled: env_or_default("PROMPT_CACHING_ENABLED", "false")
                    .parse()
                    .unwrap_or(false),
            },

            // PTC configuration
            ptc: PtcConfig {
                sandbox_image: env_or_default("PTC_SANDBOX_IMAGE", "python:3.11-slim"),
                session_timeout_seconds: env_or_default("PTC_SESSION_TIMEOUT", "270")
                    .parse()
                    .unwrap_or(270),
                execution_timeout_seconds: env_or_default("PTC_EXECUTION_TIMEOUT", "60")
                    .parse()
                    .unwrap_or(60),
                memory_limit: env_or_default("PTC_MEMORY_LIMIT", "256m"),
                network_disabled: env_or_default("PTC_NETWORK_DISABLED", "true")
                    .parse()
                    .unwrap_or(true),
            },

            // Model mapping - load default mappings
            default_model_mapping: Self::load_default_model_mapping(),

            // Streaming
            streaming_timeout_seconds: env_or_default("STREAMING_TIMEOUT_SECONDS", "300")
                .parse()
                .unwrap_or(300),

            // Debug options
            print_prompts: env_or_default("PRINT_PROMPTS", "false")
                .parse()
                .unwrap_or(false),

            // Ephemeral API key (will be generated later if needed)
            ephemeral_api_key: None,
        };

        // Validate settings
        settings.validate()?;

        Ok(settings)
    }

    /// Validate settings
    fn validate(&self) -> Result<()> {
        // Validate port range
        if self.port == 0 {
            anyhow::bail!("Port cannot be 0");
        }

        // Validate rate limit settings
        if self.rate_limit.enabled {
            if self.rate_limit.requests_per_window == 0 {
                anyhow::bail!("Rate limit requests_per_window must be > 0");
            }
            if self.rate_limit.window_seconds == 0 {
                anyhow::bail!("Rate limit window_seconds must be > 0");
            }
        }

        // Validate PTC settings if enabled
        if self.features.enable_ptc {
            if self.ptc.execution_timeout_seconds == 0 {
                anyhow::bail!("PTC execution_timeout must be > 0");
            }
            if self.ptc.session_timeout_seconds == 0 {
                anyhow::bail!("PTC session_timeout must be > 0");
            }
        }

        // Warn if no API key auth in production
        if self.environment == Environment::Production && !self.require_api_key {
            tracing::warn!("Running in production without API key authentication!");
        }

        Ok(())
    }

    /// Load default model mappings
    ///
    /// Supports environment variable overrides:
    /// - ANTHROPIC_DEFAULT_MODEL: Override ALL models to use this Bedrock model (highest priority)
    /// - ANTHROPIC_DEFAULT_SONNET_MODEL: Override default sonnet model (maps all sonnet variants)
    /// - ANTHROPIC_DEFAULT_HAIKU_MODEL: Override default haiku model (maps all haiku variants)
    /// - ANTHROPIC_DEFAULT_OPUS_MODEL: Override default opus model (maps all opus variants)
    ///
    /// Also maps Bedrock model IDs (with us./global. prefixes and # suffix) to overrides
    /// when environment variables are set. This allows Claude CLI to send Bedrock model IDs
    /// directly while still applying the overrides.
    fn load_default_model_mapping() -> HashMap<String, String> {
        let mut mapping = HashMap::new();

        // Check for environment variable overrides
        let global_override = env::var("ANTHROPIC_DEFAULT_MODEL").ok();
        let sonnet_override = env::var("ANTHROPIC_DEFAULT_SONNET_MODEL").ok();
        let haiku_override = env::var("ANTHROPIC_DEFAULT_HAIKU_MODEL").ok();
        let opus_override = env::var("ANTHROPIC_DEFAULT_OPUS_MODEL").ok();

        // Helper to add all variants of a Bedrock model ID to the mapping
        let add_bedrock_variants = |mapping: &mut HashMap<String, String>, base_model: &str, target: &str| {
            // Add variants with different prefixes and suffixes
            // e.g., base_model = "anthropic.claude-3-5-haiku-20241022-v1:0"
            // Variants:
            //   - anthropic.claude-3-5-haiku-20241022-v1:0
            //   - anthropic.claude-3-5-haiku-20241022-v1:0#
            //   - us.anthropic.claude-3-5-haiku-20241022-v1:0
            //   - us.anthropic.claude-3-5-haiku-20241022-v1:0#
            //   - global.anthropic.claude-3-5-haiku-20241022-v1:0
            //   - global.anthropic.claude-3-5-haiku-20241022-v1:0#
            mapping.insert(base_model.to_string(), target.to_string());
            mapping.insert(format!("{}#", base_model), target.to_string());
            mapping.insert(format!("us.{}", base_model), target.to_string());
            mapping.insert(format!("us.{}#", base_model), target.to_string());
            mapping.insert(format!("global.{}", base_model), target.to_string());
            mapping.insert(format!("global.{}#", base_model), target.to_string());
        };

        // If ANTHROPIC_DEFAULT_MODEL is set, use it for all models
        if let Some(ref global_model) = global_override {
            // Map all known Anthropic model IDs to the global override
            let all_anthropic_models = [
                "claude-3-5-sonnet-20241022",
                "claude-3-5-sonnet-latest",
                "claude-3-sonnet-20240229",
                "claude-sonnet-4-20250514",
                "claude-3-5-haiku-20241022",
                "claude-3-haiku-20240307",
                "claude-3-opus-20240229",
                "claude-opus-4-5-20251101",
            ];
            for model in all_anthropic_models {
                mapping.insert(model.to_string(), global_model.clone());
            }

            // Also map all Bedrock model IDs to the global override
            let all_bedrock_models = [
                "anthropic.claude-3-5-sonnet-20241022-v2:0",
                "anthropic.claude-3-sonnet-20240229-v1:0",
                "anthropic.claude-sonnet-4-20250514-v1:0",
                "anthropic.claude-3-5-haiku-20241022-v1:0",
                "anthropic.claude-3-haiku-20240307-v1:0",
                "anthropic.claude-3-opus-20240229-v1:0",
                "anthropic.claude-opus-4-5-20251101-v1:0",
            ];
            for model in all_bedrock_models {
                add_bedrock_variants(&mut mapping, model, global_model);
            }

            return mapping;
        }

        // Claude Sonnet models (Anthropic format)
        let sonnet_model = sonnet_override.clone()
            .unwrap_or_else(|| "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string());
        mapping.insert("claude-3-5-sonnet-20241022".to_string(), sonnet_model.clone());
        mapping.insert("claude-3-5-sonnet-latest".to_string(), sonnet_model.clone());
        mapping.insert("claude-3-sonnet-20240229".to_string(), sonnet_model.clone());

        // Claude Sonnet 4 (Anthropic format)
        let sonnet4_model = sonnet_override.clone()
            .unwrap_or_else(|| "anthropic.claude-sonnet-4-20250514-v1:0".to_string());
        mapping.insert("claude-sonnet-4-20250514".to_string(), sonnet4_model.clone());

        // Claude Haiku models (Anthropic format)
        let haiku_model = haiku_override.clone()
            .unwrap_or_else(|| "anthropic.claude-3-5-haiku-20241022-v1:0".to_string());
        mapping.insert("claude-3-5-haiku-20241022".to_string(), haiku_model.clone());
        mapping.insert("claude-3-haiku-20240307".to_string(), haiku_model.clone());

        // Claude Opus models (Anthropic format)
        let opus_model = opus_override.clone()
            .unwrap_or_else(|| "anthropic.claude-3-opus-20240229-v1:0".to_string());
        mapping.insert("claude-3-opus-20240229".to_string(), opus_model.clone());

        // Claude Opus 4.5 (Anthropic format)
        let opus45_model = opus_override.clone()
            .unwrap_or_else(|| "anthropic.claude-opus-4-5-20251101-v1:0".to_string());
        mapping.insert("claude-opus-4-5-20251101".to_string(), opus45_model.clone());

        // Add Bedrock model ID variants ONLY when overrides are set
        // This allows redirecting Bedrock model IDs to different models

        if let Some(ref target) = sonnet_override {
            // Sonnet variants
            add_bedrock_variants(&mut mapping, "anthropic.claude-3-5-sonnet-20241022-v2:0", target);
            add_bedrock_variants(&mut mapping, "anthropic.claude-3-sonnet-20240229-v1:0", target);
            add_bedrock_variants(&mut mapping, "anthropic.claude-sonnet-4-20250514-v1:0", target);
        }

        if let Some(ref target) = haiku_override {
            // Haiku variants
            add_bedrock_variants(&mut mapping, "anthropic.claude-3-5-haiku-20241022-v1:0", target);
            add_bedrock_variants(&mut mapping, "anthropic.claude-3-haiku-20240307-v1:0", target);
        }

        if let Some(ref target) = opus_override {
            // Opus variants
            add_bedrock_variants(&mut mapping, "anthropic.claude-3-opus-20240229-v1:0", target);
            add_bedrock_variants(&mut mapping, "anthropic.claude-opus-4-5-20251101-v1:0", target);
        }

        mapping
    }

    /// Check if running in development mode
    pub fn is_development(&self) -> bool {
        self.environment == Environment::Development
    }

    /// Check if running in production mode
    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }

    /// Get the server address string
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            app_name: "anthropic-bedrock-proxy".to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: Environment::Development,
            log_level: "info".to_string(),
            host: "0.0.0.0".to_string(),
            port: 8000,
            aws_region: "us-east-1".to_string(),
            aws_access_key_id: None,
            aws_secret_access_key: None,
            dynamodb_endpoint_url: None,
            bedrock_endpoint_url: None,
            dynamodb_api_keys_table: "anthropic-proxy-api-keys".to_string(),
            dynamodb_usage_table: "anthropic-proxy-usage".to_string(),
            dynamodb_usage_stats_table: "anthropic-proxy-usage-stats".to_string(),
            dynamodb_model_mapping_table: "anthropic-proxy-model-mapping".to_string(),
            dynamodb_model_pricing_table: "anthropic-proxy-model-pricing".to_string(),
            require_api_key: true,
            master_api_key: None,
            rate_limit: RateLimitConfig::default(),
            features: FeatureFlags::default(),
            ptc: PtcConfig::default(),
            default_model_mapping: Self::load_default_model_mapping(),
            streaming_timeout_seconds: 300,
            print_prompts: false,
            ephemeral_api_key: None,
        }
    }
}

impl Settings {
    /// Generate and set an ephemeral API key
    /// Returns the generated key
    pub fn generate_ephemeral_key(&mut self) -> String {
        let key = format!("sk-{}", uuid::Uuid::new_v4().to_string().replace("-", ""));
        self.ephemeral_api_key = Some(key.clone());
        key
    }
}

/// Helper function to get environment variable with default
fn env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.app_name, "anthropic-bedrock-proxy");
        assert_eq!(settings.port, 8000);
        assert!(settings.require_api_key);
    }

    #[test]
    fn test_environment_parsing() {
        assert_eq!("development".parse::<Environment>().unwrap(), Environment::Development);
        assert_eq!("dev".parse::<Environment>().unwrap(), Environment::Development);
        assert_eq!("production".parse::<Environment>().unwrap(), Environment::Production);
        assert_eq!("prod".parse::<Environment>().unwrap(), Environment::Production);
    }

    #[test]
    fn test_model_mapping() {
        let settings = Settings::default();
        assert!(settings.default_model_mapping.contains_key("claude-3-5-sonnet-20241022"));
    }

    #[test]
    fn test_server_addr() {
        let settings = Settings::default();
        assert_eq!(settings.server_addr(), "0.0.0.0:8000");
    }
}
