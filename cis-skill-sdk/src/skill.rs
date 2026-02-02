//! Skill 核心 trait 定义

use crate::error::Result;
use crate::types::{Event, InvokeContext, LogLevel, SkillConfig, SkillMeta};

/// Skill 统一接口
///
/// # 示例
///
/// ```rust
/// use cis_skill_sdk::{Skill, SkillContext, Event, Result};
///
/// pub struct MySkill {
///     config: SkillConfig,
/// }
///
/// impl Skill for MySkill {
///     fn name(&self) -> &str {
///         "my-skill"
///     }
///     
///     fn version(&self) -> &str {
///         "0.1.0"
///     }
///     
///     fn init(&mut self, config: SkillConfig) -> Result<()> {
///         self.config = config;
///         Ok(())
///     }
///     
///     fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
///         ctx.log_info(&format!("Received event: {:?}", event));
///         Ok(())
///     }
/// }
/// ```
pub trait Skill: Send + Sync {
    /// Skill 名称（唯一标识）
    fn name(&self) -> &str;
    
    /// 版本号
    fn version(&self) -> &str {
        "0.1.0"
    }
    
    /// 描述
    fn description(&self) -> &str {
        ""
    }
    
    /// 获取元数据
    fn meta(&self) -> SkillMeta {
        SkillMeta {
            name: self.name().to_string(),
            version: self.version().to_string(),
            description: self.description().to_string(),
            author: String::new(),
            permissions: vec![],
            subscriptions: vec![],
            config_schema: None,
        }
    }
    
    /// 初始化
    ///
    /// 在 Skill 加载后调用，传入配置
    fn init(&mut self, _config: SkillConfig) -> Result<()> {
        Ok(())
    }
    
    /// 处理事件
    ///
    /// 核心方法，处理来自 CIS 核心的事件
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()>;
    
    /// 关闭
    ///
    /// 在 Skill 卸载前调用，执行清理
    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// Skill 上下文接口
///
/// 由 CIS 核心提供，Skill 通过此接口与核心交互
pub trait SkillContext: Send + Sync {
    /// 获取 Skill 名称
    fn skill_name(&self) -> &str;
    
    /// 获取配置
    fn config(&self) -> &SkillConfig;
    
    /// 记录日志
    fn log(&self, level: LogLevel, message: &str);
    
    /// 记录 info 日志（便捷方法）
    fn log_info(&self, message: &str) {
        self.log(LogLevel::Info, message);
    }
    
    /// 记录 warn 日志（便捷方法）
    fn log_warn(&self, message: &str) {
        self.log(LogLevel::Warn, message);
    }
    
    /// 记录 error 日志（便捷方法）
    fn log_error(&self, message: &str) {
        self.log(LogLevel::Error, message);
    }
    
    /// 记录 debug 日志（便捷方法）
    fn log_debug(&self, message: &str) {
        self.log(LogLevel::Debug, message);
    }
    
    /// 读取记忆
    fn memory_get(&self, key: &str) -> Option<Vec<u8>>;
    
    /// 写入记忆
    fn memory_set(&self, key: &str, value: &[u8]) -> Result<()>;
    
    /// 删除记忆
    fn memory_delete(&self, key: &str) -> Result<()>;
    
    /// 列出记忆键
    fn memory_list(&self, prefix: &str) -> Vec<String>;
    
    /// 调用 AI（同步）
    fn ai_chat(&self, prompt: &str) -> Result<String>;
    
    /// 调用 AI 生成 JSON（同步）
    fn ai_generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value>;
    
    /// 发送 HTTP 请求（同步）
    fn http_request(&self, request: crate::types::HttpRequest) -> Result<crate::types::HttpResponse>;
    
    /// 获取调用上下文
    fn invoke_context(&self) -> &InvokeContext;
}

// ==================== Native 模式特定 trait ====================

#[cfg(feature = "native")]
use async_trait::async_trait;

/// Native 模式 Skill 接口（支持异步）
#[cfg(feature = "native")]
#[async_trait]
pub trait NativeSkill: Send + Sync {
    /// Skill 名称
    fn name(&self) -> &str;
    
    /// 版本号
    fn version(&self) -> &str {
        "0.1.0"
    }
    
    /// 描述
    fn description(&self) -> &str {
        ""
    }
    
    /// 异步初始化
    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        let _ = config;
        Ok(())
    }
    
    /// 异步处理事件
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()>;
    
    /// 异步关闭
    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

// ==================== WASM 模式特定 trait ====================

/// WASM 模式 Skill 接口
#[cfg(feature = "wasm")]
pub trait WasmSkill {
    /// 初始化（WASM 导出函数 `skill_init` 会调用此方法）
    fn init(&mut self, config: &[u8]) -> i32;
    
    /// 处理事件（WASM 导出函数 `skill_handle_event` 会调用此方法）
    fn handle_event(&self, event_ptr: *const u8, event_len: usize) -> i32;
    
    /// 关闭（WASM 导出函数 `skill_shutdown` 会调用此方法）
    fn shutdown(&self) -> i32;
}
