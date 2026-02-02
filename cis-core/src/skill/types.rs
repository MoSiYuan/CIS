//! Skill 类型定义

use chrono::{DateTime, Utc};
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
    /// 已禁用（项目级别）
    Disabled,
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

/// Skill Room 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRoomInfo {
    /// Room ID
    pub room_id: String,
    /// 是否联邦同步
    pub federate: bool,
    /// 创建时间
    pub created_at: Option<DateTime<Utc>>,
}

impl SkillRoomInfo {
    /// 创建新的 Room 信息
    pub fn new(room_id: impl Into<String>, federate: bool) -> Self {
        Self {
            room_id: room_id.into(),
            federate,
            created_at: Some(Utc::now()),
        }
    }

    /// 从 Room ID 字符串解析
    /// 格式: !{skill_name}:cis.local[?federate=true]
    pub fn from_room_id(room_id: &str) -> Option<Self> {
        // 解析 Room ID，检查是否包含联邦标记
        let federate = room_id.contains("federate=true");
        let clean_room_id = room_id.split('?').next().unwrap_or(room_id);
        
        Some(Self {
            room_id: clean_room_id.to_string(),
            federate,
            created_at: Some(Utc::now()),
        })
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
    /// Room 配置（可选）
    #[serde(default)]
    pub room_config: Option<serde_json::Value>,
}

impl SkillMeta {
    /// 获取 Room 信息
    ///
    /// 从 room_config 或根据 Skill 名称自动生成
    pub fn room_info(&self) -> Option<SkillRoomInfo> {
        // 首先尝试从 room_config 解析
        if let Some(config) = &self.room_config {
            if let Some(room_id) = config.get("room_id").and_then(|v| v.as_str()) {
                let federate = config
                    .get("federate")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                return Some(SkillRoomInfo::new(room_id, federate));
            }
        }

        // 默认根据 Skill 名称生成 Room ID
        let room_id = format!("!{}:cis.local", self.name);
        Some(SkillRoomInfo::new(room_id, false))
    }

    /// 设置 Room 配置
    pub fn with_room_config(mut self, room_id: impl Into<String>, federate: bool) -> Self {
        self.room_config = Some(serde_json::json!({
            "room_id": room_id.into(),
            "federate": federate,
        }));
        self
    }
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

impl SkillInfo {
    /// 从 manifest 创建 SkillInfo
    pub fn from_manifest(manifest: &crate::skill::manifest::SkillManifest) -> Self {
        let skill = &manifest.skill;
        Self {
            meta: SkillMeta {
                name: skill.name.clone(),
                version: skill.version.clone(),
                description: skill.description.clone(),
                author: skill.author.clone(),
                skill_type: match skill.skill_type {
                    crate::skill::manifest::SkillType::Native => SkillType::Native,
                    crate::skill::manifest::SkillType::Wasm => SkillType::Wasm,
                    crate::skill::manifest::SkillType::Script => SkillType::Native,
                },
                path: String::new(),
                db_path: String::new(),
                permissions: manifest.permissions.custom.clone(),
                subscriptions: Vec::new(),
                config_schema: None,
                room_config: None,
            },
            runtime: SkillRuntime {
                state: SkillState::Installed,
                loaded_at: None,
                last_active_at: None,
                error: None,
                pid: None,
            },
        }
    }
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
