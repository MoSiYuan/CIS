//! # CIS Node CLI Library
//!
//! CLI tool for CIS (Cluster of Independent Systems).
//! Provides commands for skill management, memory operations, task management, and more.

pub mod commands;

use anyhow::Result;
use std::sync::Arc;

/// CLI context shared across commands
pub struct CliContext {
    /// Core storage database manager
    pub db_manager: Arc<cis_core::storage::db::DbManager>,
    /// Skill manager
    pub skill_manager: Option<cis_core::skill::SkillManager>,
    /// Memory service
    pub memory_service: Option<cis_core::memory::MemoryService>,
}

impl CliContext {
    /// Initialize CLI context
    pub fn new() -> Result<Self> {
        let db_manager = Arc::new(cis_core::storage::db::DbManager::new()?);
        
        Ok(Self {
            db_manager: db_manager.clone(),
            skill_manager: None,
            memory_service: None,
        })
    }

    /// Initialize skill manager
    pub fn init_skill_manager(&mut self) -> Result<()> {
        let manager = cis_core::skill::SkillManager::new(self.db_manager.clone())?;
        self.skill_manager = Some(manager);
        Ok(())
    }

    /// Initialize memory service
    pub fn init_memory_service(&mut self) -> Result<()> {
        let core_db = self.db_manager.core();
        let service = cis_core::memory::MemoryService::new(core_db);
        self.memory_service = Some(service);
        Ok(())
    }
}

impl Default for CliContext {
    fn default() -> Self {
        Self::new().expect("Failed to initialize CLI context")
    }
}
