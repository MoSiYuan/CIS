//! Skill Context Implementation
//!
//! 提供 SimpleSkillContext 实现。

use async_trait::async_trait;

use super::super::{Skill, SkillContext, SkillConfig};
use crate::error::Result;

/// 简单的 SkillContext 实现
pub struct SimpleSkillContext {
    config: SkillConfig,
}

impl SimpleSkillContext {
    pub fn new(config: SkillConfig) -> Self {
        Self { config }
    }
}

impl SkillContext for SimpleSkillContext {
    fn log_info(&self, message: &str) {
        tracing::info!("[Skill] {}", message);
    }

    fn log_debug(&self, message: &str) {
        tracing::debug!("[Skill] {}", message);
    }

    fn log_warn(&self, message: &str) {
        tracing::warn!("[Skill] {}", message);
    }

    fn log_error(&self, message: &str) {
        tracing::error!("[Skill] {}", message);
    }

    fn memory_get(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }

    fn memory_set(&self, _key: &str, _value: &[u8]) -> Result<()> {
        Ok(())
    }

    fn memory_delete(&self, _key: &str) -> Result<()> {
        Ok(())
    }

    fn config(&self) -> &SkillConfig {
        &self.config
    }
}
