//! Skill 注册表
//!
//! 管理 Skill 元数据和生命周期状态。

use std::collections::HashMap;
use std::fs;


use super::types::{SkillInfo, SkillMeta, SkillRuntime, SkillState};
use crate::error::{CisError, Result};
use crate::storage::paths::Paths;

/// Skill 注册表版本
const REGISTRY_VERSION: &str = "1.0";

/// Skill 注册表
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillRegistry {
    /// 注册表版本
    pub version: String,
    /// 已注册的 Skills
    pub skills: HashMap<String, SkillInfo>,
}

impl SkillRegistry {
    /// 加载注册表
    pub fn load() -> Result<Self> {
        let path = Paths::skill_registry();

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)?;
        let registry: SkillRegistry = serde_json::from_str(&content)?;

        // 版本兼容性检查
        if registry.version != REGISTRY_VERSION {
            // 未来这里可以添加迁移逻辑
            tracing::warn!(
                "Skill registry version mismatch: {} != {}",
                registry.version,
                REGISTRY_VERSION
            );
        }

        Ok(registry)
    }

    /// 保存注册表
    pub fn save(&self) -> Result<()> {
        let path = Paths::skill_registry();

        // 确保目录存在
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;

        Ok(())
    }

    /// 注册新 Skill
    pub fn register(&mut self, meta: SkillMeta) -> Result<()> {
        let name = meta.name.clone();

        if self.skills.contains_key(&name) {
            return Err(CisError::already_exists(format!(
                "Skill '{}' already registered",
                name
            )));
        }

        let info = SkillInfo {
            meta,
            runtime: SkillRuntime {
                state: SkillState::Registered,
                loaded_at: None,
                last_active_at: None,
                error: None,
                pid: None,
            },
        };

        self.skills.insert(name, info);
        self.save()?;

        Ok(())
    }

    /// 注销 Skill
    pub fn unregister(&mut self, name: &str) -> Result<()> {
        let info = self
            .skills
            .get(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        // 检查状态
        if info.runtime.state.is_active() {
            return Err(CisError::skill(format!(
                "Skill '{}' is still active, unload first",
                name
            )));
        }

        self.skills.remove(name);
        self.save()?;

        Ok(())
    }

    /// 更新 Skill 状态
    pub fn update_state(&mut self, name: &str, state: SkillState) -> Result<()> {
        let info = self
            .skills
            .get_mut(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        let now = chrono::Utc::now().timestamp() as u64;

        match state {
            SkillState::Loaded => {
                info.runtime.loaded_at = Some(now);
            }
            SkillState::Active => {
                info.runtime.last_active_at = Some(now);
            }
            SkillState::Unloaded => {
                info.runtime.pid = None;
            }
            _ => {}
        }

        info.runtime.state = state;
        self.save()?;

        Ok(())
    }

    /// 设置错误信息
    pub fn set_error(&mut self, name: &str, error: &str) -> Result<()> {
        let info = self
            .skills
            .get_mut(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        info.runtime.state = SkillState::Error;
        info.runtime.error = Some(error.to_string());
        self.save()?;

        Ok(())
    }

    /// 获取 Skill 信息
    pub fn get(&self, name: &str) -> Option<&SkillInfo> {
        self.skills.get(name)
    }

    /// 获取可变的 Skill 信息
    pub fn get_mut(&mut self, name: &str) -> Option<&mut SkillInfo> {
        self.skills.get_mut(name)
    }

    /// 检查 Skill 是否存在
    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    /// 列出所有 Skills
    pub fn list_all(&self) -> Vec<&SkillInfo> {
        self.skills.values().collect()
    }

    /// 列出特定状态的 Skills
    pub fn list_by_state(&self, state: SkillState) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|info| info.runtime.state == state)
            .collect()
    }

    /// 列出活跃 Skills
    pub fn list_active(&self) -> Vec<&SkillInfo> {
        self.skills
            .values()
            .filter(|info| info.runtime.state.is_active())
            .collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self {
            version: REGISTRY_VERSION.to_string(),
            skills: HashMap::new(),
        }
    }
}

/// Skill 注册信息（用于自动注册）
pub struct SkillRegistration {
    pub name: String,
    pub version: String,
    pub skill_type: super::types::SkillType,
    pub factory: Box<dyn Fn() -> Box<dyn SkillInstance> + Send + Sync>,
}

/// Skill 实例接口（Native Skill 实现）
pub trait SkillInstance: Send + Sync {
    /// Skill 名称
    fn name(&self) -> &str;

    /// 初始化
    fn init(&mut self, config: super::types::SkillConfig) -> Result<()>;

    /// 启动
    fn start(&self) -> Result<()>;

    /// 停止
    fn stop(&self) -> Result<()>;

    /// 处理事件
    fn handle_event(&self, event: &str, data: &[u8]) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{SkillMeta, SkillType};
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup_test_env() -> std::path::PathBuf {
        let _guard = TEST_MUTEX.lock().unwrap();
        let pid = std::process::id();
        let uuid = uuid::Uuid::new_v4().to_string();
        let temp_dir = std::env::temp_dir().join(format!("cis_test_registry_{}_{}", pid, uuid));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
        crate::storage::paths::Paths::ensure_dirs().unwrap();
        temp_dir
    }

    fn cleanup_test_env(temp_dir: &std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(temp_dir);
        std::env::remove_var("CIS_DATA_DIR");
    }

    #[test]
    fn test_register_and_unregister() {
        let temp_dir = setup_test_env();

        let mut registry = SkillRegistry::load().unwrap();

        let meta = SkillMeta {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "Test skill".to_string(),
            author: "Test".to_string(),
            skill_type: SkillType::Native,
            path: "/test".to_string(),
            db_path: "/test/data.db".to_string(),
            permissions: vec![],
            subscriptions: vec![],
            config_schema: None,
            room_config: None,
        };

        // 注册
        registry.register(meta.clone()).unwrap();
        assert!(registry.contains("test-skill"));

        // 更新状态
        registry.update_state("test-skill", SkillState::Active).unwrap();
        let info = registry.get("test-skill").unwrap();
        assert_eq!(info.runtime.state, SkillState::Active);

        // 先更新为非活跃状态才能注销
        registry.update_state("test-skill", SkillState::Unloaded).unwrap();
        registry.unregister("test-skill").unwrap();
        assert!(!registry.contains("test-skill"));

        cleanup_test_env(&temp_dir);
    }
}
