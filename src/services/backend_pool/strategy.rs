//! Load balancing strategies
//!
//! This module provides different load balancing strategies for distributing
//! requests across multiple credentials.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// Load Balance Strategy
// ============================================================================

/// Load balancing strategy for credential selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadBalanceStrategy {
    /// Round-robin selection (default)
    #[default]
    RoundRobin,
    /// Weighted selection based on credential weights
    Weighted,
    /// Random selection
    Random,
    /// Failover: use first available, switch on failure
    Failover,
}

impl LoadBalanceStrategy {
    /// Parse from string (case-insensitive)
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "round_robin" | "roundrobin" => Self::RoundRobin,
            "weighted" => Self::Weighted,
            "random" => Self::Random,
            "failover" => Self::Failover,
            _ => Self::RoundRobin,
        }
    }
}

impl std::fmt::Display for LoadBalanceStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RoundRobin => write!(f, "round_robin"),
            Self::Weighted => write!(f, "weighted"),
            Self::Random => write!(f, "random"),
            Self::Failover => write!(f, "failover"),
        }
    }
}

// ============================================================================
// Strategy State
// ============================================================================

/// State for round-robin selection
#[derive(Debug, Default)]
pub struct RoundRobinState {
    counter: AtomicUsize,
}

impl RoundRobinState {
    pub fn new() -> Self {
        Self {
            counter: AtomicUsize::new(0),
        }
    }

    /// Get next index for a given total count
    pub fn next(&self, total: usize) -> usize {
        if total == 0 {
            return 0;
        }
        self.counter.fetch_add(1, Ordering::SeqCst) % total
    }

    /// Reset the counter
    pub fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

/// State for weighted selection
#[derive(Debug)]
pub struct WeightedState {
    /// Current position in the weighted cycle
    position: AtomicUsize,
    /// Precomputed weighted indices
    weighted_indices: Vec<usize>,
}

impl WeightedState {
    /// Create a new weighted state from weights
    pub fn new(weights: &[u32]) -> Self {
        let mut weighted_indices = Vec::new();
        for (idx, &weight) in weights.iter().enumerate() {
            for _ in 0..weight {
                weighted_indices.push(idx);
            }
        }
        // If no weights, add at least one entry per credential
        if weighted_indices.is_empty() {
            weighted_indices = (0..weights.len()).collect();
        }
        Self {
            position: AtomicUsize::new(0),
            weighted_indices,
        }
    }

    /// Get next index based on weights
    pub fn next(&self) -> usize {
        if self.weighted_indices.is_empty() {
            return 0;
        }
        let pos = self.position.fetch_add(1, Ordering::SeqCst) % self.weighted_indices.len();
        self.weighted_indices[pos]
    }

    /// Rebuild weighted indices with new weights
    pub fn rebuild(&mut self, weights: &[u32]) {
        self.weighted_indices.clear();
        for (idx, &weight) in weights.iter().enumerate() {
            for _ in 0..weight {
                self.weighted_indices.push(idx);
            }
        }
        if self.weighted_indices.is_empty() {
            self.weighted_indices = (0..weights.len()).collect();
        }
        self.position.store(0, Ordering::SeqCst);
    }
}

impl Default for WeightedState {
    fn default() -> Self {
        Self {
            position: AtomicUsize::new(0),
            weighted_indices: Vec::new(),
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
    fn test_strategy_from_str() {
        assert_eq!(
            LoadBalanceStrategy::from_str("round_robin"),
            LoadBalanceStrategy::RoundRobin
        );
        assert_eq!(
            LoadBalanceStrategy::from_str("weighted"),
            LoadBalanceStrategy::Weighted
        );
        assert_eq!(
            LoadBalanceStrategy::from_str("RANDOM"),
            LoadBalanceStrategy::Random
        );
        assert_eq!(
            LoadBalanceStrategy::from_str("failover"),
            LoadBalanceStrategy::Failover
        );
        assert_eq!(
            LoadBalanceStrategy::from_str("unknown"),
            LoadBalanceStrategy::RoundRobin
        );
    }

    #[test]
    fn test_round_robin_state() {
        let state = RoundRobinState::new();
        assert_eq!(state.next(3), 0);
        assert_eq!(state.next(3), 1);
        assert_eq!(state.next(3), 2);
        assert_eq!(state.next(3), 0);
    }

    #[test]
    fn test_weighted_state() {
        // Weights: [2, 1] means credential 0 should be selected twice as often
        let state = WeightedState::new(&[2, 1]);
        let mut counts = [0, 0];
        for _ in 0..6 {
            let idx = state.next();
            counts[idx] += 1;
        }
        // After 6 selections: credential 0 should have ~4, credential 1 should have ~2
        assert_eq!(counts[0], 4);
        assert_eq!(counts[1], 2);
    }
}
