//! Tool name mapping utilities
//!
//! This module provides functionality to handle tool names that exceed
//! AWS Bedrock's 64-character limit by creating reversible short names.

use std::collections::HashMap;

/// Maximum length for tool names in AWS Bedrock API
pub const BEDROCK_TOOL_NAME_MAX_LENGTH: usize = 64;

/// Prefix used for shortened tool names
const SHORT_NAME_PREFIX: &str = "t_";

/// Tool name mapper for handling long tool names
///
/// AWS Bedrock API has a 64-character limit for tool names. This mapper
/// creates short aliases for long names and maintains a bidirectional
/// mapping for request/response conversion.
#[derive(Debug, Clone, Default)]
pub struct ToolNameMapper {
    /// Maps original (long) names to short names
    original_to_short: HashMap<String, String>,
    /// Maps short names back to original names
    short_to_original: HashMap<String, String>,
}

impl ToolNameMapper {
    /// Create a new empty mapper
    pub fn new() -> Self {
        Self {
            original_to_short: HashMap::new(),
            short_to_original: HashMap::new(),
        }
    }

    /// Get or create a short name for the given tool name
    ///
    /// If the name is already within the limit, returns it unchanged.
    /// Otherwise, creates a unique short name and stores the mapping.
    pub fn get_or_create_short_name(&mut self, original_name: &str) -> String {
        // If name is within limit, no mapping needed
        if original_name.len() <= BEDROCK_TOOL_NAME_MAX_LENGTH {
            return original_name.to_string();
        }

        // Check if we already have a mapping
        if let Some(short_name) = self.original_to_short.get(original_name) {
            return short_name.clone();
        }

        // Create a new short name using hash
        let short_name = self.generate_short_name(original_name);

        // Store bidirectional mapping
        self.original_to_short
            .insert(original_name.to_string(), short_name.clone());
        self.short_to_original
            .insert(short_name.clone(), original_name.to_string());

        tracing::debug!(
            original_name = %original_name,
            short_name = %short_name,
            original_len = original_name.len(),
            "Created tool name mapping for long name"
        );

        short_name
    }

    /// Restore the original name from a potentially shortened name
    ///
    /// If the name was shortened, returns the original.
    /// Otherwise, returns the input unchanged.
    pub fn restore_original_name(&self, name: &str) -> String {
        self.short_to_original
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    /// Check if any mappings exist
    pub fn has_mappings(&self) -> bool {
        !self.original_to_short.is_empty()
    }

    /// Get the number of mappings
    pub fn mapping_count(&self) -> usize {
        self.original_to_short.len()
    }

    /// Generate a unique short name for a long tool name
    ///
    /// Uses a hash-based approach to create a deterministic, collision-resistant short name.
    /// The format is: `t_<meaningful_prefix>_<hash>`
    fn generate_short_name(&self, original_name: &str) -> String {
        use std::hash::{Hash, Hasher};

        // Calculate a hash of the original name
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        original_name.hash(&mut hasher);
        let hash = hasher.finish();

        // Extract meaningful parts from the name
        // For MCP tools: mcp__<server>__<tool> -> extract server and tool parts
        let meaningful_prefix = self.extract_meaningful_prefix(original_name);

        // Format: t_<prefix>_<hash>
        // Keep prefix short to stay well under limit
        let hash_str = format!("{:016x}", hash);
        let short_name = format!("{}{}_{}", SHORT_NAME_PREFIX, meaningful_prefix, hash_str);

        // Ensure we're under the limit (should always be the case)
        if short_name.len() > BEDROCK_TOOL_NAME_MAX_LENGTH {
            // Fallback to just prefix + hash
            format!("{}{}", SHORT_NAME_PREFIX, hash_str)
        } else {
            short_name
        }
    }

    /// Extract a meaningful prefix from the tool name
    ///
    /// For MCP tools, extracts the tool name part.
    /// For other tools, extracts the first meaningful segment.
    fn extract_meaningful_prefix(&self, name: &str) -> String {
        // MCP format: mcp__<server-name>__<tool-name>
        if name.starts_with("mcp__") {
            if let Some(last_part) = name.rsplit("__").next() {
                // Take up to 20 chars of the tool name
                let prefix: String = last_part.chars().take(20).collect();
                // Clean up: replace hyphens with underscores for consistency
                return prefix.replace('-', "_");
            }
        }

        // Generic approach: take the last segment after common separators
        let parts: Vec<&str> = name.split(&['_', '-', '.', ':'][..]).collect();
        if let Some(last) = parts.last() {
            let prefix: String = last.chars().take(20).collect();
            return prefix;
        }

        // Fallback: just take first 20 chars
        name.chars().take(20).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_name_passthrough() {
        let mut mapper = ToolNameMapper::new();
        let short_name = "my_tool";
        assert_eq!(mapper.get_or_create_short_name(short_name), short_name);
        assert!(!mapper.has_mappings());
    }

    #[test]
    fn test_exact_length_passthrough() {
        let mut mapper = ToolNameMapper::new();
        // Create a name exactly at the limit
        let name = "a".repeat(BEDROCK_TOOL_NAME_MAX_LENGTH);
        assert_eq!(mapper.get_or_create_short_name(&name), name);
        assert!(!mapper.has_mappings());
    }

    #[test]
    fn test_long_name_mapping() {
        let mut mapper = ToolNameMapper::new();
        let long_name = "mcp__awslabs_billing-cost-management-mcp-server__compute-optimizer";
        assert!(long_name.len() > BEDROCK_TOOL_NAME_MAX_LENGTH);

        let short_name = mapper.get_or_create_short_name(long_name);

        // Should be shortened
        assert!(short_name.len() <= BEDROCK_TOOL_NAME_MAX_LENGTH);
        assert!(mapper.has_mappings());
        assert_eq!(mapper.mapping_count(), 1);

        // Should be restorable
        assert_eq!(mapper.restore_original_name(&short_name), long_name);
    }

    #[test]
    fn test_consistent_mapping() {
        let mut mapper = ToolNameMapper::new();
        let long_name = "mcp__awslabs_billing-cost-management-mcp-server__compute-optimizer";

        let short1 = mapper.get_or_create_short_name(long_name);
        let short2 = mapper.get_or_create_short_name(long_name);

        // Same input should return same output
        assert_eq!(short1, short2);
        // Only one mapping should exist
        assert_eq!(mapper.mapping_count(), 1);
    }

    #[test]
    fn test_multiple_long_names() {
        let mut mapper = ToolNameMapper::new();
        let names = [
            "mcp__awslabs_billing-cost-management-mcp-server__compute-optimizer",
            "mcp__awslabs_billing-cost-management-mcp-server__cost-optimization",
            "mcp__awslabs_billing-cost-management-mcp-server__bcm-pricing-calc",
        ];

        let mut short_names = Vec::new();
        for name in &names {
            let short = mapper.get_or_create_short_name(name);
            assert!(short.len() <= BEDROCK_TOOL_NAME_MAX_LENGTH);
            short_names.push(short);
        }

        // All short names should be unique
        let unique: std::collections::HashSet<_> = short_names.iter().collect();
        assert_eq!(unique.len(), names.len());

        // All should be restorable
        for (short, original) in short_names.iter().zip(names.iter()) {
            assert_eq!(mapper.restore_original_name(short), *original);
        }
    }

    #[test]
    fn test_restore_unknown_name() {
        let mapper = ToolNameMapper::new();
        let unknown = "unknown_tool";
        // Unknown names should pass through unchanged
        assert_eq!(mapper.restore_original_name(unknown), unknown);
    }

    #[test]
    fn test_meaningful_prefix_extraction() {
        let mapper = ToolNameMapper::new();

        // MCP format
        let prefix = mapper.extract_meaningful_prefix(
            "mcp__awslabs_billing-cost-management-mcp-server__compute-optimizer",
        );
        assert_eq!(prefix, "compute_optimizer");

        // Generic format
        let prefix = mapper.extract_meaningful_prefix("some_very_long_tool_name");
        assert_eq!(prefix, "name");
    }
}
