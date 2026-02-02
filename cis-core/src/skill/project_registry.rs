//! # Project Skill Registry
//!
//! 项目级技能注册表，管理项目特定的技能和覆盖。
//!
//! ## 功能特性
//!
//! - 项目技能隔离
//! - 全局技能继承
//! - 项目技能覆盖
//! - 动态技能发现

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{CisError, Result};
use crate::skill::manifest::SkillManifest;
use crate::skill::types::{SkillInfo, SkillRuntime, SkillState};

/// 项目技能配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSkillConfig {
    /// 技能ID
    pub skill_id: String,
    /// 技能版本
    pub version: String,
    /// 是否启用
    pub enabled: bool,
    /// 配置参数
    pub config: serde_json::Value,
    /// 覆盖全局设置
    pub overrides: HashMap<String, serde_json::Value>,
}

impl ProjectSkillConfig {
    /// 创建新的项目技能配置
    pub fn new(skill_id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            skill_id: skill_id.into(),
            version: version.into(),
            enabled: true,
            config: serde_json::Value::Object(serde_json::Map::new()),
            overrides: HashMap::new(),
        }
    }

    /// 设置是否启用
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// 设置配置
    pub fn with_config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }

    /// 添加覆盖设置
    pub fn with_override(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.overrides.insert(key.into(), value);
        self
    }
}

/// 项目技能注册信息
#[derive(Debug, Clone)]
pub struct ProjectSkillEntry {
    /// 技能信息
    pub info: SkillInfo,
    /// 项目路径
    pub project_path: PathBuf,
    /// 是否项目专属
    pub is_project_local: bool,
    /// 配置
    pub config: ProjectSkillConfig,
    /// 运行时状态
    pub runtime: crate::skill::types::SkillRuntime,
}

impl ProjectSkillEntry {
    /// 获取技能完整ID（包含项目路径）
    pub fn full_id(&self) -> String {
        if self.is_project_local {
            format!("{}@{}", self.info.meta.name, self.project_path.display())
        } else {
            self.info.meta.name.clone()
        }
    }
}

/// 项目技能注册表
pub struct ProjectSkillRegistry {
    /// 项目路径
    project_path: PathBuf,
    /// 技能条目
    skills: HashMap<String, ProjectSkillEntry>,
    /// 继承的全局技能ID列表
    inherited_skills: Vec<String>,
    /// 项目配置（保存到 .cis/project_skills.toml）
    config_path: PathBuf,
}

impl ProjectSkillRegistry {
    /// 创建新的项目技能注册表
    pub fn new(project_path: impl AsRef<Path>) -> Self {
        let project_path = project_path.as_ref().to_path_buf();
        let config_path = project_path.join(".cis").join("project_skills.toml");
        
        Self {
            project_path,
            skills: HashMap::new(),
            inherited_skills: Vec::new(),
            config_path,
        }
    }

    /// 加载项目技能注册表
    pub fn load(project_path: impl AsRef<Path>) -> Result<Self> {
        let project_path = project_path.as_ref().to_path_buf();
        let config_path = project_path.join(".cis").join("project_skills.toml");
        
        let mut registry = Self {
            project_path: project_path.clone(),
            skills: HashMap::new(),
            inherited_skills: Vec::new(),
            config_path: config_path.clone(),
        };

        // 如果配置文件存在，加载配置
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| CisError::storage(format!("Failed to read project skills config: {}", e)))?;
            let configs: Vec<ProjectSkillConfig> = toml::from_str(&content)
                .map_err(|e| CisError::storage(format!("Failed to parse project skills config: {}", e)))?;
            
            for config in configs {
                registry.inherited_skills.push(config.skill_id.clone());
            }
        }

        Ok(registry)
    }

    /// 保存项目配置
    pub fn save(&self) -> Result<()> {
        // 确保目录存在
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::storage(format!("Failed to create config directory: {}", e)))?;
        }

        // 收集所有配置
        let configs: Vec<_> = self.skills
            .values()
            .map(|entry| entry.config.clone())
            .collect();

        // 序列化为 TOML
        let content = toml::to_string_pretty(&configs)
            .map_err(|e| CisError::other(format!("Serialization error: {}", e)))?;

        // 写入文件
        std::fs::write(&self.config_path, content)
            .map_err(|e| CisError::storage(format!("Failed to write project skills config: {}", e)))?;

        Ok(())
    }

    /// 注册项目专属技能
    pub fn register_local(&mut self, info: SkillInfo, config: ProjectSkillConfig) -> Result<()> {
        let skill_id = info.meta.name.clone();
        
        let entry = ProjectSkillEntry {
            info,
            project_path: self.project_path.clone(),
            is_project_local: true,
            config,
            runtime: SkillRuntime {
                state: SkillState::Registered,
                loaded_at: None,
                last_active_at: None,
                error: None,
                pid: None,
            },
        };
        
        self.skills.insert(skill_id, entry);
        Ok(())
    }

    /// 继承全局技能
    pub fn inherit_skill(&mut self, info: SkillInfo, config: ProjectSkillConfig) -> Result<()> {
        // 检查是否已存在
        if self.skills.contains_key(&info.meta.name) {
            return Err(CisError::skill(format!(
                "Skill '{}' already registered in project",
                info.meta.name
            )));
        }

        let entry = ProjectSkillEntry {
            info,
            project_path: self.project_path.clone(),
            is_project_local: false,
            config,
            runtime: SkillRuntime {
                state: SkillState::Registered,
                loaded_at: None,
                last_active_at: None,
                error: None,
                pid: None,
            },
        };
        
        let name = entry.info.meta.name.clone();
        self.skills.insert(name.clone(), entry);
        self.inherited_skills.push(name);
        Ok(())
    }

    /// 覆盖全局技能配置
    pub fn override_skill(&mut self, skill_id: &str, overrides: HashMap<String, serde_json::Value>) -> Result<()> {
        let entry = self.skills.get_mut(skill_id)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found in project", skill_id)))?;
        
        entry.config.overrides = overrides;
        Ok(())
    }

    /// 禁用继承的技能
    pub fn disable_skill(&mut self, skill_id: &str) -> Result<()> {
        let entry = self.skills.get_mut(skill_id)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found in project", skill_id)))?;
        
        entry.config.enabled = false;
        entry.runtime.state = SkillState::Disabled;
        Ok(())
    }

    /// 启用技能
    pub fn enable_skill(&mut self, skill_id: &str) -> Result<()> {
        let entry = self.skills.get_mut(skill_id)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found in project", skill_id)))?;
        
        entry.config.enabled = true;
        entry.runtime.state = SkillState::Registered;
        Ok(())
    }

    /// 获取技能条目
    pub fn get(&self, skill_id: &str) -> Option<&ProjectSkillEntry> {
        self.skills.get(skill_id)
    }

    /// 获取技能条目（可变）
    pub fn get_mut(&mut self, skill_id: &str) -> Option<&mut ProjectSkillEntry> {
        self.skills.get_mut(skill_id)
    }

    /// 检查技能是否启用
    pub fn is_enabled(&self, skill_id: &str) -> bool {
        self.skills
            .get(skill_id)
            .map(|e| e.config.enabled && e.runtime.state != SkillState::Disabled)
            .unwrap_or(false)
    }

    /// 列出所有启用的技能
    pub fn list_enabled(&self) -> Vec<&ProjectSkillEntry> {
        self.skills
            .values()
            .filter(|e| e.config.enabled)
            .collect()
    }

    /// 列出所有项目专属技能
    pub fn list_local(&self) -> Vec<&ProjectSkillEntry> {
        self.skills
            .values()
            .filter(|e| e.is_project_local)
            .collect()
    }

    /// 列出所有继承的技能
    pub fn list_inherited(&self) -> Vec<&ProjectSkillEntry> {
        self.skills
            .values()
            .filter(|e| !e.is_project_local)
            .collect()
    }

    /// 列出所有技能
    pub fn list_all(&self) -> Vec<&ProjectSkillEntry> {
        self.skills.values().collect()
    }

    /// 获取项目路径
    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    /// 从清单安装技能
    pub fn install_from_manifest(&mut self, manifest: &SkillManifest) -> Result<()> {
        let info = SkillInfo::from_manifest(manifest);
        let config = ProjectSkillConfig::new(&info.meta.name, &info.meta.version)
            .with_enabled(true);
        
        self.register_local(info, config)
    }

    /// 卸载技能
    pub fn uninstall(&mut self, skill_id: &str) -> Result<bool> {
        // 只有项目专属技能可以完全卸载
        if let Some(entry) = self.skills.get(skill_id) {
            if entry.is_project_local {
                Ok(self.skills.remove(skill_id).is_some())
            } else {
                // 继承的技能只能禁用
                self.disable_skill(skill_id)?;
                Ok(true)
            }
        } else {
            Ok(false)
        }
    }

    /// 获取技能数量
    pub fn count(&self) -> usize {
        self.skills.len()
    }

    /// 获取项目专属技能数量
    pub fn local_count(&self) -> usize {
        self.skills.values().filter(|e| e.is_project_local).count()
    }

    /// 清空所有项目专属技能
    pub fn clear_local(&mut self) {
        self.skills.retain(|_, e| !e.is_project_local);
    }
}

/// 项目技能发现器
pub struct ProjectSkillDiscovery;

impl ProjectSkillDiscovery {
    /// 扫描项目中的本地技能
    pub fn scan_project(project_path: impl AsRef<Path>) -> Result<Vec<SkillManifest>> {
        let project_path = project_path.as_ref();
        let mut manifests = Vec::new();

        // 扫描 .cis/skills/ 目录
        let skills_dir = project_path.join(".cis").join("skills");
        if skills_dir.exists() {
            for entry in std::fs::read_dir(&skills_dir)
                .map_err(|e| CisError::storage(format!("Failed to read skills directory: {}", e)))? {
                let entry = entry
                    .map_err(|e| CisError::storage(format!("Failed to read directory entry: {}", e)))?;
                let path = entry.path();
                
                if path.is_dir() {
                    // 查找 skill.toml
                    let manifest_path = path.join("skill.toml");
                    if manifest_path.exists() {
                        match Self::load_manifest(&manifest_path) {
                            Ok(manifest) => manifests.push(manifest),
                            Err(e) => {
                                tracing::warn!("Failed to load skill manifest from {:?}: {}", manifest_path, e);
                            }
                        }
                    }
                }
            }
        }

        Ok(manifests)
    }

    /// 加载单个技能清单
    fn load_manifest(path: &Path) -> Result<SkillManifest> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CisError::storage(format!("Failed to read manifest file: {}", e)))?;
        let manifest: SkillManifest = toml::from_str(&content)
            .map_err(|e| CisError::other(format!("Parse error: {}", e)))?;
        Ok(manifest)
    }

    /// 查找项目的依赖技能
    pub fn find_dependencies(project_path: impl AsRef<Path>) -> Result<Vec<String>> {
        let project_path = project_path.as_ref();
        let mut dependencies = Vec::new();

        // 检查 .cis/project.toml
        let config_path = project_path.join(".cis").join("project.toml");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)
                .map_err(|e| CisError::storage(format!("Failed to read project config: {}", e)))?;
            
            // 简单解析 skills 字段
            if let Ok(config) = toml::from_str::<toml::Value>(&content) {
                if let Some(skills) = config.get("skills").and_then(|s| s.as_array()) {
                    for skill in skills {
                        if let Some(id) = skill.get("id").and_then(|i| i.as_str()) {
                            dependencies.push(id.to_string());
                        } else if let Some(id) = skill.as_str() {
                            dependencies.push(id.to_string());
                        }
                    }
                }
            }
        }

        Ok(dependencies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_skill_info(id: &str, name: &str) -> SkillInfo {
        SkillInfo {
            id: id.to_string(),
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: Some("Test skill".to_string()),
            author: Some("Test".to_string()),
            skill_type: crate::skill::types::SkillType::Wasm,
        }
    }

    #[test]
    fn test_project_skill_registry() {
        let temp_dir = tempdir().unwrap();
        let project_path = temp_dir.path();
        
        let mut registry = ProjectSkillRegistry::new(project_path);
        
        // 注册本地技能
        let info1 = create_test_skill_info("local1", "Local Skill 1");
        let config1 = ProjectSkillConfig::new("local1", "0.1.0");
        registry.register_local(info1, config1).unwrap();
        
        // 继承全局技能
        let info2 = create_test_skill_info("global1", "Global Skill 1");
        let config2 = ProjectSkillConfig::new("global1", "1.0.0");
        registry.inherit_skill(info2, config2).unwrap();
        
        // 测试列表
        assert_eq!(registry.list_all().len(), 2);
        assert_eq!(registry.list_local().len(), 1);
        assert_eq!(registry.list_inherited().len(), 1);
        
        // 测试禁用
        registry.disable_skill("global1").unwrap();
        assert!(!registry.is_enabled("global1"));
        
        // 测试启用列表
        assert_eq!(registry.list_enabled().len(), 1);
        
        // 测试卸载（继承的技能只能禁用）
        assert!(registry.uninstall("global1").unwrap());
        assert!(registry.get("global1").is_some()); // 仍然存在于注册表中
        assert!(!registry.is_enabled("global1"));
        
        // 测试卸载本地技能
        assert!(registry.uninstall("local1").unwrap());
        assert!(registry.get("local1").is_none()); // 完全移除
    }

    #[test]
    fn test_project_skill_config() {
        let config = ProjectSkillConfig::new("skill1", "1.0.0")
            .with_enabled(false)
            .with_override("timeout", serde_json::json!(30));
        
        assert!(!config.enabled);
        assert_eq!(config.overrides.get("timeout"), Some(&serde_json::json!(30)));
    }
}
