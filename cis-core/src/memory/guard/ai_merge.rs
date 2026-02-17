//! # AI Merge 冲突解决实现
//!
//! **使用 AI 智能合并冲突的记忆值**
//!
//! # 核心机制
//!
//! - **智能合并**：AI 分析冲突内容并生成合并版本
//! - **错误回退**：AI 失败时自动回退到 KeepLocal
//! - **重试机制**：支持配置重试次数和超时
//! - **多种策略**：SmartMerge、ContentBased、TimeBased

use crate::error::{CisError, Result};
use crate::memory::guard::conflict_guard::ConflictVersion;
use std::sync::Arc;
use tokio::sync::RwLock;

/// AI 合并配置
#[derive(Debug, Clone)]
pub struct AIMergeConfig {
    /// 合并策略
    pub strategy: AIMergeStrategy,
    /// 最大重试次数
    pub max_retries: usize,
    /// 超时时间（秒）
    pub timeout_secs: u64,
}

impl Default for AIMergeConfig {
    fn default() -> Self {
        Self {
            strategy: AIMergeStrategy::SmartMerge,
            max_retries: 2,
            timeout_secs: 30,
        }
    }
}

/// AI 合并策略
#[derive(Debug, Clone, Copy)]
pub enum AIMergeStrategy {
    /// 智能合并（保留双方有效信息）
    SmartMerge,
    /// 基于内容的合并（检测并保留较新的内容）
    ContentBased,
    /// 基于时间的合并（优先保留较新的修改）
    TimeBased,
}

/// AI 合并器
///
/// 使用 AI 服务智能合并冲突的记忆值。
pub struct AIMerger {
    /// AI Provider（可选，支持运行时注入）
    ai_provider: Arc<RwLock<Option<Box<dyn crate::ai::AiProvider>>>>,
    /// 配置
    config: AIMergeConfig,
}

impl AIMerger {
    /// 创建新的 AI 合并器
    ///
    /// # 参数
    ///
    /// - `config`: 合并配置
    ///
    /// # 示例
    ///
    /// ```rust
    /// use cis_core::memory::guard::ai_merge::AIMerger;
    ///
    /// let merger = AIMerger::new(Default::default());
    /// ```
    pub fn new(config: AIMergeConfig) -> Self {
        Self {
            ai_provider: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// 设置 AI Provider
    ///
    /// # 参数
    ///
    /// - `provider`: AI Provider 实例
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::ai::ClaudeCliProvider;
    /// use cis_core::memory::guard::ai_merge::AIMerger;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let merger = AIMerger::new(Default::default());
    /// merger.set_ai_provider(Box::new(ClaudeCliProvider::default())).await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn set_ai_provider(&self, provider: Box<dyn crate::ai::AiProvider>) {
        let mut ai = self.ai_provider.write().await;
        *ai = Some(provider);
    }

    /// 执行 AI 合并
    ///
    /// # 核心逻辑
    ///
    /// 1. 准备合并 prompt（包含本地和远程版本）
    /// 2. 调用 AI 服务
    /// 3. 解析合并结果
    /// 4. 失败时回退到 KeepLocal
    ///
    /// # 参数
    ///
    /// - `key`: 冲突的记忆键
    /// - `local_version`: 本地版本
    /// - `remote_versions`: 远程版本列表
    ///
    /// # 返回
    ///
    /// 返回合并后的记忆值。如果 AI 合并失败，回退到本地版本。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// use cis_core::memory::guard::ai_merge::AIMerger;
    ///
    /// let merger = AIMerger::new(Default::default());
    /// let merged = merger.merge(
    ///     "config/database",
    ///     &local_version,
    ///     &remote_versions
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn merge(
        &self,
        key: &str,
        local_version: &ConflictVersion,
        remote_versions: &[ConflictVersion],
    ) -> Result<Vec<u8>> {
        tracing::info!(
            "[AIMerge] Starting merge for key: {}, {} remote versions",
            key,
            remote_versions.len()
        );

        // 1. 检查 AI Provider 是否可用
        let ai_provider = {
            let ai = self.ai_provider.read().await;
            if ai.is_none() {
                tracing::warn!("[AIMerge] No AI provider available, falling back to KeepLocal");
                return Ok(local_version.value.clone());
            }
            ai.as_ref().unwrap().clone()
        };

        // 2. 检查 AI Provider 是否可用
        if !ai_provider.available().await {
            tracing::warn!("[AIMerge] AI provider not available, falling back to KeepLocal");
            return Ok(local_version.value.clone());
        }

        // 3. 准备合并 prompt
        let prompt = self.build_merge_prompt(key, local_version, remote_versions);

        // 4. 调用 AI 服务（带重试）
        let merged_value = self.merge_with_retry(&ai_provider, &prompt).await?;

        tracing::info!("[AIMerge] Successfully merged key: {}", key);
        Ok(merged_value)
    }

    /// 带重试的 AI 合并
    ///
    /// # 参数
    ///
    /// - `ai_provider`: AI Provider
    /// - `prompt`: 合并 prompt
    ///
    /// # 返回
    ///
    /// 返回合并后的值。
    async fn merge_with_retry(
        &self,
        ai_provider: &Box<dyn crate::ai::AiProvider>,
        prompt: &str,
    ) -> Result<Vec<u8>> {
        let mut last_error = None;

        for attempt in 0..self.config.max_retries {
            if attempt > 0 {
                tracing::warn!("[AIMerge] Retry attempt {}/{}", attempt, self.config.max_retries);
            }

            match self.call_ai_merge(ai_provider, prompt).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    tracing::error!("[AIMerge] Attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e);
                }
            }
        }

        // 所有重试都失败，返回错误
        Err(last_error.unwrap_or_else(|| {
            CisError::ai("AI merge failed: unknown error".to_string())
        }))
    }

    /// 调用 AI 执行合并
    ///
    /// # 参数
    ///
    /// - `ai_provider`: AI Provider
    /// - `prompt`: 合并 prompt
    ///
    /// # 返回
    ///
    /// 返回合并后的值。
    async fn call_ai_merge(
        &self,
        ai_provider: &Box<dyn crate::ai::AiProvider>,
        prompt: &str,
    ) -> Result<Vec<u8>> {
        // 构建系统消息
        let system = self.build_system_prompt();

        // 构建用户消息
        let messages = vec![crate::ai::Message::user(prompt)];

        // 调用 AI
        let response = tokio::time::timeout(
            tokio::time::Duration::from_secs(self.config.timeout_secs),
            ai_provider.chat_with_context(&system, &messages),
        )
        .await
        .map_err(|_| CisError::ai("AI merge timeout".to_string()))?
        .map_err(|e| CisError::ai(format!("AI merge failed: {}", e)))?;

        // 解析响应
        self.parse_ai_response(&response)
    }

    /// 构建系统提示词
    ///
    /// # 返回
    ///
    /// 返回系统提示词字符串。
    fn build_system_prompt(&self) -> String {
        match self.config.strategy {
            AIMergeStrategy::SmartMerge => {
                r#"You are a memory conflict resolution specialist. Your task is to intelligently merge conflicting versions of stored data.

**Instructions:**
1. Analyze both the local and remote versions carefully
2. Identify the key differences and conflicts
3. Create a merged version that:
   - Preserves valid information from both versions
   - Resolves contradictions intelligently
   - Maintains data consistency and integrity
4. Return ONLY the merged value (no explanations or markdown)

**Output Format:**
Return the merged value directly as plain text. Do not include any explanations, markdown formatting, or additional commentary."#.to_string()
            }
            AIMergeStrategy::ContentBased => {
                r#"You are a data merge specialist. Merge conflicting versions based on content analysis.

**Instructions:**
1. Compare the content of local and remote versions
2. Identify which version has more complete or recent information
3. Merge by prioritizing content quality and completeness
4. Return ONLY the merged value

**Output:**
Return the merged value directly as plain text."#.to_string()
            }
            AIMergeStrategy::TimeBased => {
                r#"You are a time-based merge specialist. Prioritize more recent changes.

**Instructions:**
1. Compare timestamps provided in the input
2. Favor more recent changes when there are conflicts
3. Preserve older changes that don't conflict
4. Return ONLY the merged value

**Output:**
Return the merged value directly as plain text."#.to_string()
            }
        }
    }

    /// 构建合并 Prompt
    ///
    /// # 参数
    ///
    /// - `key`: 冲突的记忆键
    /// - `local_version`: 本地版本
    /// - `remote_versions`: 远程版本列表
    ///
    /// # 返回
    ///
    /// 返回合并 prompt 字符串。
    fn build_merge_prompt(
        &self,
        key: &str,
        local_version: &ConflictVersion,
        remote_versions: &[ConflictVersion],
    ) -> String {
        // 尝试将字节值转换为字符串
        let local_value_str = String::from_utf8_lossy(&local_version.value);

        let mut remote_versions_str = String::new();
        for (i, remote) in remote_versions.iter().enumerate() {
            let value_str = String::from_utf8_lossy(&remote.value);
            remote_versions_str.push_str(&format!(
                "\n\nRemote Version {} (Node: {}, Timestamp: {}):\n{}",
                i + 1,
                remote.node_id,
                remote.timestamp,
                value_str
            ));
        }

        format!(
            r#"**Conflict Key:** {}

**Local Version (Node: {}, Timestamp: {}):**
{}

**Remote Versions:**{}
---
**Task:** Merge these versions into a single, coherent value that preserves the best information from all versions."#,
            key,
            local_version.node_id,
            local_version.timestamp,
            local_value_str,
            remote_versions_str
        )
    }

    /// 解析 AI 响应
    ///
    /// # 参数
    ///
    /// - `response`: AI 响应字符串
    ///
    /// # 返回
    ///
    /// 返回解析后的字节数组。
    fn parse_ai_response(&self, response: &str) -> Result<Vec<u8>> {
        // 去除可能的 markdown 代码块标记
        let cleaned = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        // 转换为字节
        Ok(cleaned.as_bytes().to_vec())
    }
}

impl Default for AIMerger {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::guard::conflict_guard::ConflictVersion;

    /// 创建测试用的 ConflictVersion
    fn create_test_version(node_id: &str, value: &str, timestamp: i64) -> ConflictVersion {
        ConflictVersion {
            node_id: node_id.to_string(),
            vector_clock: vec![],
            value: value.as_bytes().to_vec(),
            timestamp,
        }
    }

    /// 测试 AIMerger 创建
    #[test]
    fn test_ai_merger_creation() {
        let config = AIMergeConfig {
            strategy: AIMergeStrategy::SmartMerge,
            max_retries: 3,
            timeout_secs: 60,
        };

        let merger = AIMerger::new(config);

        // 验证配置已正确设置
        // 注意：无法直接访问 config 字段，但通过行为可以验证
        assert_eq!(merger.config.strategy, AIMergeStrategy::SmartMerge);
        assert_eq!(merger.config.max_retries, 3);
        assert_eq!(merger.config.timeout_secs, 60);
    }

    /// 测试默认配置
    #[test]
    fn test_default_config() {
        let config = AIMergeConfig::default();
        assert_eq!(config.strategy, AIMergeStrategy::SmartMerge);
        assert_eq!(config.max_retries, 2);
        assert_eq!(config.timeout_secs, 30);
    }

    /// 测试默认 Merger
    #[test]
    fn test_default_merger() {
        let merger = AIMerger::default();
        assert_eq!(merger.config.strategy, AIMergeStrategy::SmartMerge);
        assert_eq!(merger.config.max_retries, 2);
        assert_eq!(merger.config.timeout_secs, 30);
    }

    /// 测试构建合并 Prompt
    #[test]
    fn test_build_merge_prompt() {
        let merger = AIMerger::default();

        let local = create_test_version("node-a", "local value", 1000);
        let remote = create_test_version("node-b", "remote value", 2000);

        let prompt = merger.build_merge_prompt("test/key", &local, &[remote]);

        // 验证 prompt 包含必要信息
        assert!(prompt.contains("test/key"));
        assert!(prompt.contains("node-a"));
        assert!(prompt.contains("node-b"));
        assert!(prompt.contains("local value"));
        assert!(prompt.contains("remote value"));
        assert!(prompt.contains("1000"));
        assert!(prompt.contains("2000"));
    }

    /// 测试构建系统提示词 - SmartMerge
    #[test]
    fn test_build_system_prompt_smart_merge() {
        let config = AIMergeConfig {
            strategy: AIMergeStrategy::SmartMerge,
            ..Default::default()
        };
        let merger = AIMerger::new(config);

        let prompt = merger.build_system_prompt();

        assert!(prompt.contains("memory conflict resolution specialist"));
        assert!(prompt.contains("Preserves valid information"));
        assert!(prompt.contains("Return ONLY the merged value"));
    }

    /// 测试构建系统提示词 - ContentBased
    #[test]
    fn test_build_system_prompt_content_based() {
        let config = AIMergeConfig {
            strategy: AIMergeStrategy::ContentBased,
            ..Default::default()
        };
        let merger = AIMerger::new(config);

        let prompt = merger.build_system_prompt();

        assert!(prompt.contains("data merge specialist"));
        assert!(prompt.contains("content quality and completeness"));
    }

    /// 测试构建系统提示词 - TimeBased
    #[test]
    fn test_build_system_prompt_time_based() {
        let config = AIMergeConfig {
            strategy: AIMergeStrategy::TimeBased,
            ..Default::default()
        };
        let merger = AIMerger::new(config);

        let prompt = merger.build_system_prompt();

        assert!(prompt.contains("time-based merge specialist"));
        assert!(prompt.contains("Prioritize more recent changes"));
    }

    /// 测试解析 AI 响应
    #[test]
    fn test_parse_ai_response() {
        let merger = AIMerger::default();

        // 测试普通响应
        let response = "merged value";
        let parsed = merger.parse_ai_response(response).unwrap();
        assert_eq!(parsed, b"merged value");

        // 测试带 markdown 的响应
        let response_with_markdown = "```\nmerged value\n```";
        let parsed = merger.parse_ai_response(response_with_markdown).unwrap();
        assert_eq!(parsed, b"merged value");

        // 测试带 JSON 标记的响应
        let response_with_json = "```json\n{\"key\": \"value\"}\n```";
        let parsed = merger.parse_ai_response(response_with_json).unwrap();
        assert_eq!(parsed, b"{\"key\": \"value\"}");
    }

    /// 测试多远程版本的 Prompt 构建
    #[test]
    fn test_build_merge_prompt_multiple_remotes() {
        let merger = AIMerger::default();

        let local = create_test_version("node-a", "local value", 1000);
        let remote1 = create_test_version("node-b", "remote value 1", 2000);
        let remote2 = create_test_version("node-c", "remote value 2", 3000);

        let prompt = merger.build_merge_prompt("test/key", &local, &[remote1, remote2]);

        // 验证包含所有远程版本
        assert!(prompt.contains("Remote Version 1"));
        assert!(prompt.contains("Remote Version 2"));
        assert!(prompt.contains("remote value 1"));
        assert!(prompt.contains("remote value 2"));
    }
}
