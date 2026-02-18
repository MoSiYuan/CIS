//! Dummy Skill Implementation
//!
//! 占位 Skill 实现（用于未提供具体实例的情况）。

use async_trait::async_trait;
use std::sync::Arc;

use super::super::{Event, Skill, SkillContext};
use super::context::SimpleSkillContext;
use crate::error::Result;

/// 占位 Skill 实现（用于未提供具体实例的情况）
pub struct DummySkill {
    name: String,
}

impl DummySkill {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Skill for DummySkill {
    fn name(&self) -> &str {
        &self.name
    }

    async fn handle_event(&self, _ctx: &dyn SkillContext, _event: Event) -> Result<()> {
        // 占位实现，不处理任何事件
        Ok(())
    }
}
