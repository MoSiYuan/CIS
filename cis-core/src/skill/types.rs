//! Skill 类型定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Skill 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillType {
    /// 本地编译的 Skill
    Native,
    /// WASM 沙箱 Skill
    Wasm,
    /// 远程 Skill (预留)
    Remote,
}

/// Skill 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    /// 已安装但未注册
    Installed,
    /// 已注册
    Registered,
    /// 已加载到内存
    Loaded,
    /// 正常运行中
    Active,
    /// 已暂停
    Paused,
    /// 正在卸载
    Unloading,
    /// 已卸载
    Unloaded,
    /// 加载/运行出错
    Error,
    /// 已移除
    Removed,
}

impl SkillState {
    /// 是否可以加载
    pub fn can_load(&self) -> bool {
        matches!(self, Self::Registered | Self::Unloaded)
    }

    /// 是否可以卸载
    pub fn can_unload(&self) -> bool {
        matches!(self, Self::Loaded | Self::Active | Self::Paused | Self::Error)
    }

    /// 是否可以暂停
    pub fn can_pause(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// 是否可以恢复
    pub fn can_resume(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// 是否处于活动状态
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Skill 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMeta {
    /// Skill 名称（唯一标识）
    pub name: String,
    /// 版本号 (semver)
    pub version: String,
    /// 描述
    pub description: String,
    /// 作者
    pub author: String,
    /// Skill 类型
    pub skill_type: SkillType,
    /// 代码路径
    pub path: String,
    /// 数据库路径
    pub db_path: String,
    /// 权限列表
    pub permissions: Vec<String>,
    /// 订阅的事件
    pub subscriptions: Vec<String>,
    /// 配置 schema
    pub config_schema: Option<serde_json::Value>,
}

/// Skill 运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRuntime {
    /// 当前状态
    pub state: SkillState,
    /// 加载时间戳
    pub loaded_at: Option<u64>,
    /// 最后活动时间
    pub last_active_at: Option<u64>,
    /// 错误信息（如果有）
    pub error: Option<String>,
    /// 进程 ID (Native Skill)
    pub pid: Option<u32>,
}

/// Skill 完整信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// 元数据
    #[serde(flatten)]
    pub meta: SkillMeta,
    /// 运行时信息
    pub runtime: SkillRuntime,
}

/// Skill 配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfig {
    /// 配置值
    #[serde(flatten)]
    pub values: HashMap<String, serde_json::Value>,
}

impl SkillConfig {
    /// 获取配置值
    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.values
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// 设置配置值
    pub fn set<T: Serialize>(&mut self, key: impl Into<String>, value: T) {
        if let Ok(v) = serde_json::to_value(value) {
            self.values.insert(key.into(), v);
        }
    }
}

/// Skill 加载选项
#[derive(Debug, Clone, Default)]
pub struct LoadOptions {
    /// 是否自动激活
    pub auto_activate: bool,
    /// 配置
    pub config: Option<SkillConfig>,
    /// 强制重新加载（即使已加载）
    pub force_reload: bool,
}

/// Skill 统计信息
#[derive(Debug, Clone, Default)]
pub struct SkillStats {
    /// 处理的事件数
    pub events_processed: u64,
    /// 错误数
    pub errors: u64,
    /// 平均处理时间 (ms)
    pub avg_process_time_ms: f64,
    /// 内存使用 (bytes)
    pub memory_usage: u64,
}
