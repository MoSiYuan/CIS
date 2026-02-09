//! # 内置 Skill 管理
//!
//! 管理 CIS 内置的 Skill，支持自动编译和安装。

use std::path::PathBuf;
use std::process::Command;
use tracing::{info, warn, error};

use crate::error::{CisError, Result};
use crate::skill::manager::SkillManager;
use crate::skill::types::{SkillMeta, SkillType};

/// 内置 Skill 信息
#[derive(Debug, Clone)]
pub struct BuiltinSkill {
    /// Skill 名称
    pub name: &'static str,
    /// 源码路径（相对于项目根目录）
    pub source_path: &'static str,
    /// 是否必需
    pub required: bool,
    /// Skill 描述
    pub description: &'static str,
    /// 编译依赖
    pub dependencies: &'static [&'static str],
}

/// 内置 Skill 清单
pub const BUILTIN_SKILLS: &[BuiltinSkill] = &[
    BuiltinSkill {
        name: "init-wizard",
        source_path: "skills/init-wizard",
        required: true,
        description: "初始化向导 - 帮助用户配置 CIS 环境",
        dependencies: &[],
    },
    BuiltinSkill {
        name: "memory-organizer",
        source_path: "skills/memory-organizer",
        required: true,
        description: "记忆整理 - 自动整理和归档记忆",
        dependencies: &[],
    },
    BuiltinSkill {
        name: "dag-executor",
        source_path: "skills/dag-executor",
        required: true,
        description: "DAG 执行器 - 执行有向无环图任务",
        dependencies: &["init-wizard"],
    },
    BuiltinSkill {
        name: "ai-executor",
        source_path: "skills/ai-executor",
        required: true,
        description: "AI 执行器 - 调用 AI 模型执行任务",
        dependencies: &[],
    },
    BuiltinSkill {
        name: "push-client",
        source_path: "skills/push-client",
        required: false,
        description: "推送客户端 - 接收推送通知",
        dependencies: &[],
    },
    BuiltinSkill {
        name: "im",
        source_path: "skills/im",
        required: false,
        description: "即时消息 - Matrix/Slack 消息处理",
        dependencies: &[],
    },
];

/// 内置 Skill 安装器
pub struct BuiltinSkillInstaller {
    /// 项目根目录
    project_root: PathBuf,
    /// 是否编译 release 版本
    release_mode: bool,
}

impl BuiltinSkillInstaller {
    /// 创建新的安装器
    pub fn new() -> Result<Self> {
        let project_root = Self::detect_project_root()?;
        Ok(Self {
            project_root,
            release_mode: true,
        })
    }

    /// 设置编译模式
    pub fn with_release_mode(mut self, release: bool) -> Self {
        self.release_mode = release;
        self
    }

    /// 检测项目根目录
    fn detect_project_root() -> Result<PathBuf> {
        // 1. 检查环境变量
        if let Ok(root) = std::env::var("CIS_PROJECT_ROOT") {
            let path = PathBuf::from(root);
            if path.join("Cargo.toml").exists() {
                return Ok(path);
            }
        }

        // 2. 检查当前目录
        let current = std::env::current_dir()?;
        if current.join("Cargo.toml").exists() {
            // 检查是否是 CIS 项目
            let cargo_toml = std::fs::read_to_string(current.join("Cargo.toml"))?;
            if cargo_toml.contains("cis-core") || cargo_toml.contains("cis-node") {
                return Ok(current);
            }
        }

        // 3. 检查可执行文件所在目录
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // 检查是否是开发环境
                let possible_root = exe_dir.parent().and_then(|p| p.parent());
                if let Some(root) = possible_root {
                    if root.join("Cargo.toml").exists() {
                        let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml"))?;
                        if cargo_toml.contains("cis-core") {
                            return Ok(root.to_path_buf());
                        }
                    }
                }
            }
        }

        // 4. 检查安装目录（用户主目录）
        if let Some(home) = dirs::home_dir() {
            let install_root = home.join(".cis").join("source");
            if install_root.join("Cargo.toml").exists() {
                return Ok(install_root);
            }
        }

        Err(CisError::configuration(
            "无法检测 CIS 项目根目录。请设置 CIS_PROJECT_ROOT 环境变量。"
        ))
    }

    /// 检查 Skill 是否已安装
    fn is_skill_installed(&self, name: &str) -> bool {
        // 检查注册表中是否存在
        use crate::storage::db::DbManager;
        
        match DbManager::new() {
            Ok(db_manager) => {
                let db_manager = std::sync::Arc::new(db_manager);
                match crate::skill::manager::SkillManager::new(db_manager) {
                    Ok(manager) => manager.get_info(name).map(|info| info.is_some()).unwrap_or(false),
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    /// 编译 Native Skill
    fn compile_skill(&self, skill: &BuiltinSkill) -> Result<PathBuf> {
        let source_path = self.project_root.join(skill.source_path);
        
        if !source_path.exists() {
            return Err(CisError::skill(format!(
                "Skill '{}' 源码不存在: {:?}",
                skill.name, source_path
            )));
        }

        info!("正在编译内置 Skill '{}'...", skill.name);

        // 构建编译命令
        let mut cmd = Command::new("cargo");
        cmd.arg("build")
            .arg("-p")
            .arg(skill.name)
            .current_dir(&self.project_root);

        if self.release_mode {
            cmd.arg("--release");
        }

        // 执行编译
        let output = cmd.output()
            .map_err(|e| CisError::skill(format!("编译命令执行失败: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Skill '{}' 编译失败:\n{}", skill.name, stderr);
            return Err(CisError::skill(format!(
                "编译失败: {}", stderr.lines().next().unwrap_or("未知错误")
            )));
        }

        info!("Skill '{}' 编译成功", skill.name);
        
        // 返回编译输出目录
        let target_dir = if self.release_mode {
            self.project_root.join("target/release")
        } else {
            self.project_root.join("target/debug")
        };
        
        Ok(target_dir)
    }

    /// 安装单个 Skill
    fn install_single_skill(&self, skill: &BuiltinSkill, manager: &SkillManager) -> Result<()> {
        // 检查是否已安装
        if self.is_skill_installed(skill.name) {
            info!("Skill '{}' 已安装，跳过", skill.name);
            return Ok(());
        }

        info!("正在安装内置 Skill '{}'...", skill.name);

        // 编译 Skill
        let _output_dir = self.compile_skill(skill)?;

        // 安装到系统
        let source_path = self.project_root.join(skill.source_path);
        manager.install(&source_path, SkillType::Native)?;

        info!("内置 Skill '{}' 安装完成", skill.name);
        Ok(())
    }

    /// 安装所有必需的内置 Skill
    pub fn install_required_skills(&self) -> Result<Vec<String>> {
        use crate::storage::db::DbManager;
        
        let db_manager = DbManager::new()?;
        let db_manager = std::sync::Arc::new(db_manager);
        let manager = crate::skill::manager::SkillManager::new(db_manager)?;
        
        let mut installed = Vec::new();
        let mut failed = Vec::new();

        info!("开始安装必需的内置 Skills...");

        for skill in BUILTIN_SKILLS.iter().filter(|s| s.required) {
            match self.install_single_skill(skill, &manager) {
                Ok(_) => installed.push(skill.name.to_string()),
                Err(e) => {
                    warn!("安装 Skill '{}' 失败: {}", skill.name, e);
                    failed.push((skill.name, e));
                }
            }
        }

        if !failed.is_empty() {
            return Err(CisError::skill(format!(
                "以下必需 Skill 安装失败: {:?}",
                failed.iter().map(|(n, _)| *n).collect::<Vec<_>>()
            )));
        }

        info!("所有必需内置 Skills 安装完成: {:?}", installed);
        Ok(installed)
    }

    /// 安装所有内置 Skill（包括可选）
    pub fn install_all_skills(&self) -> Result<(Vec<String>, Vec<String>)> {
        use crate::storage::db::DbManager;
        
        let db_manager = DbManager::new()?;
        let db_manager = std::sync::Arc::new(db_manager);
        let manager = crate::skill::manager::SkillManager::new(db_manager)?;
        
        let mut installed = Vec::new();
        let mut failed = Vec::new();

        info!("开始安装所有内置 Skills...");

        for skill in BUILTIN_SKILLS.iter() {
            match self.install_single_skill(skill, &manager) {
                Ok(_) => installed.push(skill.name.to_string()),
                Err(e) => {
                    if skill.required {
                        error!("必需 Skill '{}' 安装失败: {}", skill.name, e);
                    } else {
                        warn!("可选 Skill '{}' 安装失败: {}", skill.name, e);
                    }
                    failed.push(skill.name.to_string());
                }
            }
        }

        info!(
            "内置 Skills 安装完成: {} 成功, {} 失败",
            installed.len(),
            failed.len()
        );
        
        Ok((installed, failed))
    }

    /// 获取内置 Skill 列表
    pub fn list_builtin_skills() -> &'static [BuiltinSkill] {
        BUILTIN_SKILLS
    }
}

impl Default for BuiltinSkillInstaller {
    fn default() -> Self {
        Self::new().expect("Failed to create BuiltinSkillInstaller")
    }
}

/// 检查并安装缺失的必需 Skills
pub fn ensure_required_skills() -> Result<Vec<String>> {
    let installer = BuiltinSkillInstaller::new()?;
    installer.install_required_skills()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_skills_list() {
        let skills = BuiltinSkillInstaller::list_builtin_skills();
        assert!(!skills.is_empty());
        
        // 检查必需 skill
        let required: Vec<_> = skills.iter().filter(|s| s.required).collect();
        assert!(!required.is_empty());
        
        // 检查 init-wizard 存在
        assert!(skills.iter().any(|s| s.name == "init-wizard"));
    }

    #[test]
    fn test_detect_project_root() {
        // 这个测试在开发环境中应该通过
        // 在 CI 中可能需要设置 CIS_PROJECT_ROOT
        if std::env::var("CIS_PROJECT_ROOT").is_ok() {
            let root = BuiltinSkillInstaller::detect_project_root();
            assert!(root.is_ok());
        }
    }
}
