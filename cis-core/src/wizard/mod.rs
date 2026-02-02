//! # 初始化向导模块
//!
//! 引导用户完成 CIS 环境配置和项目初始化。

use std::path::Path;

use crate::error::{CisError, Result};
use crate::project::Project;
use crate::storage::paths::Paths;

pub mod checks;
pub mod config_gen;

pub use checks::EnvironmentChecker;
pub use config_gen::ConfigGenerator;

/// 初始化向导
pub struct InitWizard {
    options: InitOptions,
}

/// 初始化选项
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// 是否初始化项目（而非全局）
    pub project_mode: bool,
    /// 项目目录（仅在 project_mode 时有效）
    pub project_dir: Option<std::path::PathBuf>,
    /// 是否跳过环境检查
    pub skip_checks: bool,
    /// 是否强制覆盖已有配置
    pub force: bool,
    /// 选择的 AI Provider
    pub preferred_provider: Option<String>,
}

impl Default for InitOptions {
    fn default() -> Self {
        Self {
            project_mode: false,
            project_dir: None,
            skip_checks: false,
            force: false,
            preferred_provider: None,
        }
    }
}

/// 向导结果
#[derive(Debug, Clone)]
pub struct InitResult {
    /// 是否成功
    pub success: bool,
    /// 生成的配置文件路径
    pub config_paths: Vec<std::path::PathBuf>,
    /// 警告信息
    pub warnings: Vec<String>,
    /// 建议
    pub recommendations: Vec<String>,
}

impl InitWizard {
    /// 创建新的向导实例
    pub fn new(options: InitOptions) -> Self {
        Self { options }
    }

    /// 运行初始化向导
    pub fn run(&self) -> Result<InitResult> {
        let mut result = InitResult {
            success: true,
            config_paths: vec![],
            warnings: vec![],
            recommendations: vec![],
        };

        // 1. 环境检查
        if !self.options.skip_checks {
            let checker = EnvironmentChecker::new();
            let check_result = checker.run_all_checks()?;

            if !check_result.can_proceed {
                return Err(CisError::configuration(
                    "Environment checks failed. Please fix the issues and try again."
                ));
            }

            result.warnings = check_result.warnings;
            result.recommendations = check_result.recommendations;
        }

        // 2. 生成配置
        if self.options.project_mode {
            // 项目级初始化
            let project_dir = self.options.project_dir.clone()
                .unwrap_or_else(|| std::env::current_dir().unwrap());

            let _project = self.init_project(&project_dir)?;
            result.config_paths.push(project_dir.join(".cis/project.toml"));
        } else {
            // 全局初始化
            self.init_global()?;
            result.config_paths.push(Paths::config_file());
            result.config_paths.push(Paths::node_key_file());
        }

        Ok(result)
    }

    /// 初始化全局配置
    fn init_global(&self) -> Result<()> {
        // 确保目录存在
        Paths::ensure_dirs()?;

        // 生成节点密钥
        self.generate_node_key()?;

        // 生成全局配置
        let generator = ConfigGenerator::new();
        let config = generator.generate_global_config(self.options.preferred_provider.as_deref())?;

        // 保存配置
        let config_path = Paths::config_file();
        std::fs::write(&config_path, config)
            .map_err(|e| CisError::storage(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// 初始化项目
    fn init_project(&self, dir: &Path) -> Result<Project> {
        // 检查是否已有项目
        if dir.join(".cis/project.toml").exists() && !self.options.force {
            return Err(CisError::already_exists(
                "Project already initialized. Use --force to overwrite."
            ));
        }

        // 确保全局配置已存在
        if !Paths::config_file().exists() {
            self.init_global()?;
        }

        // 创建项目
        let project_name = dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed");

        let project = Project::init(dir, project_name)?;

        Ok(project)
    }

    /// 生成节点密钥
    fn generate_node_key(&self) -> Result<()> {
        let key_path = Paths::node_key_file();

        if key_path.exists() && !self.options.force {
            return Ok(()); // 已存在且不强制覆盖
        }

        // 生成随机密钥
        let key: Vec<u8> = (0..32).map(|_| rand::random::<u8>()).collect();

        // 保存密钥
        std::fs::write(&key_path, &key)
            .map_err(|e| CisError::storage(format!("Failed to write node key: {}", e)))?;

        // 设置权限 (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&key_path)?.permissions();
            perms.set_mode(0o600); // 仅所有者可读写
            std::fs::set_permissions(&key_path, perms)?;
        }

        Ok(())
    }
}

/// 快速初始化（使用默认值）
pub fn quick_init(project_mode: bool) -> Result<InitResult> {
    let wizard = InitWizard::new(InitOptions {
        project_mode,
        ..Default::default()
    });
    wizard.run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_wizard() {
        // 测试向导创建
        let wizard = InitWizard::new(InitOptions::default());
        assert!(!wizard.options.project_mode);
    }
}
