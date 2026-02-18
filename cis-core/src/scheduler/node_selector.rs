//! # Node Selector for DAG Tasks
//!
//! Provides node selection capability for heterogeneous task routing.
//! Allows DAG tasks to specify which nodes should execute them based on
//! architecture, features, or custom constraints.

use crate::error::{CisError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node selector for heterogeneous task routing
///
/// Specifies requirements for the node that should execute a DAG task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct NodeSelector {
    /// Target architecture (e.g., "x86_64", "aarch64")
    pub arch: Option<String>,

    /// Required features/capabilities (e.g., "metal", "cuda", "neural-engine")
    pub features: Option<Vec<String>>,

    /// OS type (e.g., "linux", "macos", "windows")
    pub os: Option<String>,

    /// Minimum required resources
    pub min_resources: Option<ResourceRequirements>,

    /// Custom labels as key-value pairs
    pub labels: Option<HashMap<String, String>>,
}

/// Resource requirements for task execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceRequirements {
    /// Minimum CPU cores
    pub min_cpu_cores: Option<u32>,

    /// Minimum memory in MB
    pub min_memory_mb: Option<u64>,

    /// Minimum disk space in GB
    pub min_disk_gb: Option<u64>,

    /// Requires GPU
    pub requires_gpu: Option<bool>,

    /// Minimum GPU memory in MB
    pub min_gpu_memory_mb: Option<u64>,
}

impl NodeSelector {
    /// Create a new node selector
    pub fn new() -> Self {
        Self {
            arch: None,
            features: None,
            os: None,
            min_resources: None,
            labels: None,
        }
    }

    /// Set target architecture
    pub fn with_arch(mut self, arch: impl Into<String>) -> Self {
        self.arch = Some(arch.into());
        self
    }

    /// Add required features
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = Some(features);
        self
    }

    /// Set OS type
    pub fn with_os(mut self, os: impl Into<String>) -> Self {
        self.os = Some(os.into());
        self
    }

    /// Set minimum resources
    pub fn with_min_resources(mut self, resources: ResourceRequirements) -> Self {
        self.min_resources = Some(resources);
        self
    }

    /// Add custom labels
    pub fn with_labels(mut self, labels: HashMap<String, String>) -> Self {
        self.labels = Some(labels);
        self
    }

    /// Check if a node matches the selector criteria
    pub fn matches_node(&self, node: &NodeInfo) -> Result<bool> {
        // Check architecture
        if let Some(required_arch) = &self.arch {
            if node.arch.as_ref() != Some(required_arch) {
                return Ok(false);
            }
        }

        // Check OS
        if let Some(required_os) = &self.os {
            if node.os.as_ref() != Some(required_os) {
                return Ok(false);
            }
        }

        // Check features
        if let Some(required_features) = &self.features {
            let node_features = node.features.as_ref().map(|f| f.as_slice()).unwrap_or(&[]);
            for feature in required_features {
                if !node_features.contains(feature) {
                    return Ok(false);
                }
            }
        }

        // Check resources
        if let Some(min_resources) = &self.min_resources {
            if let Some(node_resources) = &node.resources {
                // Check CPU cores
                if let Some(min_cpu) = min_resources.min_cpu_cores {
                    if node_resources.cpu_cores < min_cpu {
                        return Ok(false);
                    }
                }

                // Check memory
                if let Some(min_memory) = min_resources.min_memory_mb {
                    if node_resources.memory_mb < min_memory {
                        return Ok(false);
                    }
                }

                // Check GPU
                if let Some(true) = min_resources.requires_gpu {
                    if !node_resources.has_gpu {
                        return Ok(false);
                    }
                }

                // Check GPU memory
                if let Some(min_gpu_mem) = min_resources.min_gpu_memory_mb {
                    if node_resources.gpu_memory_mb < Some(min_gpu_mem) {
                        return Ok(false);
                    }
                }
            }
        }

        // Check labels
        if let Some(required_labels) = &self.labels {
            let node_labels = node.labels.as_ref().unwrap_or(&HashMap::new());
            for (key, value) in required_labels {
                if node_labels.get(key) != Some(value) {
                    return Ok(false);
                }
            }
        }

        Ok(true)
    }

    /// Convert from TOML table representation
    pub fn from_toml_table(table: &toml::Value) -> Result<Self> {
        let mut selector = Self::new();

        // Parse architecture
        if let Some(arch) = table.get("arch") {
            if let Some(arch_str) = arch.as_str() {
                selector = selector.with_arch(arch_str);
            }
        }

        // Parse features
        if let Some(features) = table.get("features") {
            if let Some(features_array) = features.as_array() {
                let feature_vec: Vec<String> = features_array
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                selector = selector.with_features(feature_vec);
            }
        }

        // Parse OS
        if let Some(os) = table.get("os") {
            if let Some(os_str) = os.as_str() {
                selector = selector.with_os(os_str);
            }
        }

        // Parse labels
        if let Some(labels) = table.get("labels") {
            if let Some(labels_table) = labels.as_table() {
                let mut labels_map = HashMap::new();
                for (key, value) in labels_table {
                    if let Some(value_str) = value.as_str() {
                        labels_map.insert(key.clone(), value_str.to_string());
                    }
                }
                selector = selector.with_labels(labels_map);
            }
        }

        Ok(selector)
    }
}

impl Default for NodeSelector {
    fn default() -> Self {
        Self::new()
    }
}

/// Node information for matching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID
    pub node_id: String,

    /// Architecture (e.g., "x86_64", "aarch64")
    pub arch: Option<String>,

    /// OS type (e.g., "linux", "macos", "windows")
    pub os: Option<String>,

    /// Available features/capabilities
    pub features: Option<Vec<String>>,

    /// Available resources
    pub resources: Option<NodeResources>,

    /// Custom labels
    pub labels: Option<HashMap<String, String>>,
}

/// Node resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeResources {
    /// Number of CPU cores
    pub cpu_cores: u32,

    /// Available memory in MB
    pub memory_mb: u64,

    /// Available disk space in GB
    pub disk_gb: u64,

    /// Has GPU
    pub has_gpu: bool,

    /// GPU memory in MB (if GPU available)
    pub gpu_memory_mb: Option<u64>,
}

/// Node selector filter for batch operations
#[derive(Debug, Clone)]
pub struct NodeSelectorFilter {
    selectors: Vec<NodeSelector>,
}

impl NodeSelectorFilter {
    /// Create a new filter from multiple selectors
    pub fn new(selectors: Vec<NodeSelector>) -> Self {
        Self { selectors }
    }

    /// Filter nodes that match any of the selectors
    pub fn filter_nodes(&self, nodes: &[NodeInfo]) -> Vec<NodeInfo> {
        nodes
            .iter()
            .filter(|node| {
                self
                    .selectors
                    .iter()
                    .any(|selector| selector.matches_node(node).unwrap_or(false))
            })
            .cloned()
            .collect()
    }

    /// Find the best matching node based on selector criteria
    pub fn find_best_match(&self, nodes: &[NodeInfo]) -> Option<NodeInfo> {
        let matching = self.filter_nodes(nodes);
        matching.into_iter().next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_selector_arch() {
        let selector = NodeSelector::new().with_arch("aarch64");

        let mac_node = NodeInfo {
            node_id: "mac-1".to_string(),
            arch: Some("aarch64".to_string()),
            os: Some("macos".to_string()),
            features: Some(vec!["metal".to_string()]),
            resources: None,
            labels: None,
        };

        let linux_node = NodeInfo {
            node_id: "linux-1".to_string(),
            arch: Some("x86_64".to_string()),
            os: Some("linux".to_string()),
            features: None,
            resources: None,
            labels: None,
        };

        assert!(selector.matches_node(&mac_node).unwrap());
        assert!(!selector.matches_node(&linux_node).unwrap());
    }

    #[test]
    fn test_node_selector_features() {
        let selector = NodeSelector::new().with_features(vec!["metal".to_string(), "neural-engine".to_string()]);

        let metal_node = NodeInfo {
            node_id: "mac-1".to_string(),
            arch: None,
            os: None,
            features: Some(vec!["metal".to_string(), "neural-engine".to_string()]),
            resources: None,
            labels: None,
        };

        let cuda_node = NodeInfo {
            node_id: "linux-1".to_string(),
            arch: None,
            os: None,
            features: Some(vec!["cuda".to_string()]),
            resources: None,
            labels: None,
        };

        assert!(selector.matches_node(&metal_node).unwrap());
        assert!(!selector.matches_node(&cuda_node).unwrap());
    }

    #[test]
    fn test_node_selector_labels() {
        let mut labels = HashMap::new();
        labels.insert("team".to_string(), "ai-research".to_string());
        labels.insert("environment".to_string(), "production".to_string());

        let selector = NodeSelector::new().with_labels(labels.clone());

        let matching_node = NodeInfo {
            node_id: "node-1".to_string(),
            arch: None,
            os: None,
            features: None,
            resources: None,
            labels: Some(labels.clone()),
        };

        let non_matching_node = NodeInfo {
            node_id: "node-2".to_string(),
            arch: None,
            os: None,
            features: None,
            resources: None,
            labels: Some({
                let mut labels = HashMap::new();
                labels.insert("team".to_string(), "backend".to_string());
                labels
            }),
        };

        assert!(selector.matches_node(&matching_node).unwrap());
        assert!(!selector.matches_node(&non_matching_node).unwrap());
    }

    #[test]
    fn test_node_filter() {
        let metal_selector = NodeSelector::new().with_features(vec!["metal".to_string()]);
        let cuda_selector = NodeSelector::new().with_features(vec!["cuda".to_string()]);

        let filter = NodeSelectorFilter::new(vec![metal_selector, cuda_selector]);

        let nodes = vec![
            NodeInfo {
                node_id: "mac-1".to_string(),
                arch: None,
                os: None,
                features: Some(vec!["metal".to_string()]),
                resources: None,
                labels: None,
            },
            NodeInfo {
                node_id: "linux-1".to_string(),
                arch: None,
                os: None,
                features: Some(vec!["cuda".to_string()]),
                resources: None,
                labels: None,
            },
            NodeInfo {
                node_id: "linux-2".to_string(),
                arch: None,
                os: None,
                features: Some(vec!["vulkan".to_string()]),
                resources: None,
                labels: None,
            },
        ];

        let filtered = filter.filter_nodes(&nodes);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().any(|n| n.node_id == "mac-1"));
        assert!(filtered.iter().any(|n| n.node_id == "linux-1"));
    }
}
