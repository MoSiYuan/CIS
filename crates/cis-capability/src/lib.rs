//! CIS Capability Layer
//! 
//! Unified core capabilities: skill execution, memory management, context extraction.
//! Used by both Skill adapter and MCP adapter.

pub mod context;
pub mod memory;
pub mod skill;
pub mod types;

use std::sync::Arc;
use tokio::sync::RwLock;

pub use context::ContextExtractor;
pub use memory::MemoryService;
pub use skill::SkillEngine;
pub use types::*;

/// Unified capability layer
pub struct CapabilityLayer {
    pub skill: Arc<RwLock<SkillEngine>>,
    pub memory: Arc<RwLock<MemoryService>>,
    pub context: Arc<RwLock<ContextExtractor>>,
}

impl CapabilityLayer {
    /// Initialize with default paths
    pub async fn new() -> types::Result<Self> {
        let skill = Arc::new(RwLock::new(SkillEngine::new()));
        let memory = Arc::new(RwLock::new(MemoryService::open_default()?));
        let context = Arc::new(RwLock::new(ContextExtractor::new()));

        Ok(Self { skill, memory, context })
    }

    /// Initialize with custom paths
    pub async fn with_paths(
        skill_engine: SkillEngine,
        memory_service: MemoryService,
        context_extractor: ContextExtractor,
    ) -> Self {
        Self {
            skill: Arc::new(RwLock::new(skill_engine)),
            memory: Arc::new(RwLock::new(memory_service)),
            context: Arc::new(RwLock::new(context_extractor)),
        }
    }

    /// Convenience: execute a skill with auto context detection
    pub async fn execute(
        &self,
        skill_name: impl Into<String>,
        params: serde_json::Value,
        caller: CallerType,
    ) -> types::Result<ExecutionResult> {
        let skill_name = skill_name.into();
        
        // Detect context
        let context = {
            let ctx = self.context.read().await;
            ctx.detect_current().await?
        };

        let request = ExecutionRequest {
            skill_name,
            params,
            context,
            caller,
        };

        let engine = self.skill.read().await;
        engine.execute(request).await
    }

    /// Convenience: store memory
    pub async fn remember(
        &self,
        key: impl Into<String>,
        value: impl Into<String>,
        scope: MemoryScope,
    ) -> types::Result<MemoryEntry> {
        let context = self.context.read().await;
        let project_path = context.detect_current().await?.project_root;
        
        let memory = self.memory.read().await;
        memory.store(key, value, scope, project_path.as_deref())
    }

    /// Convenience: recall memory
    pub async fn recall(
        &self,
        key: &str,
    ) -> types::Result<Option<String>> {
        let context = self.context.read().await;
        let project_path = context.detect_current().await?.project_root;
        
        let memory = self.memory.read().await;
        memory.recall(key, project_path.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_capability_layer_creation() {
        let capability = CapabilityLayer::new().await;
        assert!(capability.is_ok());
    }
}
