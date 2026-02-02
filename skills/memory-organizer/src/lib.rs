//! Memory Organizer Skill
//!
//! 从 AgentFlow 迁移的记忆整理功能
//! 使用 AI 自动整理和增强记忆

/// 记忆条目
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub category: String,
}

/// 元数据结构
#[derive(Debug, Clone)]
pub struct MemoryMeta {
    pub keywords: Vec<String>,
    pub summary: String,
    pub category: String,
}

pub struct MemoryOrganizer {
    // 通过 WASM host 调用 AI executor
}

impl MemoryOrganizer {
    pub fn new() -> Self { Self }
    
    /// 处理记忆写入事件
    pub fn on_memory_write(&self, entry: &MemoryEntry) {
        let content = String::from_utf8_lossy(&entry.value);
        
        // 1. 生成提取关键词的 prompt
        let keyword_prompt = format!(
            "从以下文本中提取3-5个关键词，用逗号分隔:\n{}",
            content
        );
        
        // 2. 生成摘要的 prompt
        let summary_prompt = format!(
            "用一句话总结以下文本:\n{}",
            content
        );
        
        // 通过 host 调用 AI executor
        // 实际执行时，WASM runtime 会处理这个调用
        self.call_ai("keywords", &keyword_prompt);
        self.call_ai("summary", &summary_prompt);
    }
    
    /// 调用 AI（通过 host 接口）
    fn call_ai(&self, _purpose: &str, prompt: &str) {
        // WASM host 会提供这个函数
        // 实际实现由 CIS core 注入
        let _ = prompt;
    }
    
    /// 解析 AI 返回的关键词
    pub fn parse_keywords(&self, response: &str) -> Vec<String> {
        response
            .split([',', '，', '\n'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
    
    /// 解析 AI 返回的摘要
    pub fn parse_summary(&self, response: &str) -> String {
        response.trim().to_string()
    }
}

impl Default for MemoryOrganizer {
    fn default() -> Self { Self::new() }
}

// ==================== AgentFlow 迁移的代码 ====================

/// 从 AgentFlow memory/mod.rs 迁移的冲突解决逻辑
pub mod conflict {
    use std::cmp::Ordering;
    
    /// 简单时间戳比较（替代 UpdateToken）
    pub fn compare_by_time(a: i64, b: i64) -> Ordering {
        a.cmp(&b)
    }
    
    /// 私域记忆无冲突，直接覆盖
    pub fn resolve_private(newer: bool) -> bool {
        newer
    }
}

/// 从 AgentFlow db/migration.rs 迁移的迁移框架
pub mod migration {
    /// 迁移版本
    pub struct Migration {
        pub version: u32,
        pub name: String,
        pub sql: String,
    }
    
    /// 简单的迁移管理器
    pub struct MigrationManager;
    
    impl MigrationManager {
        pub fn new() -> Self { Self }
        
        /// 运行迁移（WASM 内只定义，实际执行在 host）
        pub fn run(&self, _migrations: &[Migration]) -> Result<(), String> {
            // 由 host 实现
            Ok(())
        }
    }
}

// WASM 导出
#[no_mangle]
pub extern "C" fn skill_init() -> i32 {
    0
}

/// 处理记忆写入
/// 输入: JSON { "key": "...", "value": "...", "category": "..." }
#[no_mangle]
pub extern "C" fn skill_on_memory_write(input_ptr: *const u8, input_len: usize) -> i32 {
    use std::slice;
    
    if input_ptr.is_null() || input_len == 0 {
        return -1;
    }
    
    let input = unsafe { slice::from_raw_parts(input_ptr, input_len) };
    
    let entry: MemoryEntry = match serde_json::from_slice(input) {
        Ok(e) => e,
        Err(_) => return -1,
    };
    
    let organizer = MemoryOrganizer::new();
    organizer.on_memory_write(&entry);
    
    0
}

/// 解析关键词
#[no_mangle]
pub extern "C" fn skill_parse_keywords(input: *const u8, len: usize) -> *mut u8 {
    use std::ffi::CString;
    
    if input.is_null() || len == 0 {
        return CString::new("[]").unwrap().into_raw();
    }
    
    let slice = unsafe { slice::from_raw_parts(input, len) };
    let response = String::from_utf8_lossy(slice);
    
    let organizer = MemoryOrganizer::new();
    let keywords = organizer.parse_keywords(&response);
    
    let json = serde_json::to_string(&keywords).unwrap_or_else(|_| "[]".to_string());
    CString::new(json).unwrap().into_raw()
}
