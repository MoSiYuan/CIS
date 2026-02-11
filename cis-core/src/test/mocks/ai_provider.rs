//! # Mock AI Provider
//!
//! AI Provider 的 Mock 实现，用于测试 AI 相关功能。

use async_trait::async_trait;
use crate::error::{CisError, Result};
use crate::traits::{
    AiProvider, CompletionRequest, CompletionResponse,
    EmbeddingRequest, EmbeddingResponse, ModelInfo, TokenUsage,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock as AsyncRwLock;

/// AI Provider Mock
#[derive(Clone)]
pub struct MockAiProvider {
    name: Arc<Mutex<String>>,
    available: Arc<Mutex<bool>>,
    completion_responses: Arc<AsyncRwLock<HashMap<String, CompletionResponse>>>,
    embedding_responses: Arc<AsyncRwLock<HashMap<String, EmbeddingResponse>>>,
    default_response: Arc<Mutex<String>>,
    default_embedding: Arc<Mutex<Vec<f32>>>,
    latency_ms: Arc<Mutex<u64>>,
    should_fail: Arc<Mutex<Option<String>>>,
    models: Arc<Mutex<Vec<ModelInfo>>>,
}

impl std::fmt::Debug for MockAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockAiProvider")
            .field("name", &self.name.lock().unwrap())
            .field("available", &self.available.lock().unwrap())
            .finish()
    }
}

impl MockAiProvider {
    /// 创建新的 Mock
    pub fn new() -> Self {
        let default_models = vec![
            ModelInfo {
                id: "mock-model".to_string(),
                name: "Mock Model".to_string(),
                provider: "mock".to_string(),
                max_context_length: 8192,
                supports_function_calling: false,
                supports_embedding: true,
                metadata: HashMap::new(),
            },
        ];

        Self {
            name: Arc::new(Mutex::new("mock-ai".to_string())),
            available: Arc::new(Mutex::new(true)),
            completion_responses: Arc::new(AsyncRwLock::new(HashMap::new())),
            embedding_responses: Arc::new(AsyncRwLock::new(HashMap::new())),
            default_response: Arc::new(Mutex::new("Mock AI Response".to_string())),
            default_embedding: Arc::new(Mutex::new(vec![0.1; 768])),
            latency_ms: Arc::new(Mutex::new(0)),
            should_fail: Arc::new(Mutex::new(None)),
            models: Arc::new(Mutex::new(default_models)),
        }
    }

    /// 创建有延迟的 Mock
    pub fn with_latency(ms: u64) -> Self {
        let provider = Self::new();
        *provider.latency_ms.lock().unwrap() = ms;
        provider
    }

    /// 创建不可用的 Mock
    pub fn unavailable() -> Self {
        let provider = Self::new();
        *provider.available.lock().unwrap() = false;
        provider
    }

    /// 设置名称
    pub fn with_name(self, name: impl Into<String>) -> Self {
        *self.name.lock().unwrap() = name.into();
        self
    }

    /// 预设补全响应
    pub async fn preset_completion(&self, prompt: impl Into<String>, response: CompletionResponse) {
        let mut responses = self.completion_responses.write().await;
        responses.insert(prompt.into(), response);
    }

    /// 预设嵌入响应
    pub async fn preset_embedding(&self, text: impl Into<String>, response: EmbeddingResponse) {
        let mut responses = self.embedding_responses.write().await;
        responses.insert(text.into(), response);
    }

    /// 设置默认响应
    pub fn set_default_response(&self, response: impl Into<String>) {
        *self.default_response.lock().unwrap() = response.into();
    }

    /// 设置默认嵌入
    pub fn set_default_embedding(&self, embedding: Vec<f32>) {
        *self.default_embedding.lock().unwrap() = embedding;
    }

    /// 设置可用性
    pub fn set_available(&self, available: bool) {
        *self.available.lock().unwrap() = available;
    }

    /// 设置下次调用失败
    pub fn will_fail(&self, message: impl Into<String>) {
        *self.should_fail.lock().unwrap() = Some(message.into());
    }

    /// 模拟延迟
    async fn simulate_latency(&self) {
        let latency = *self.latency_ms.lock().unwrap();
        if latency > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(latency)).await;
        }
    }

    /// 清空调用记录
    pub fn clear(&self) {
        *self.should_fail.lock().unwrap() = None;
    }
}

impl Default for MockAiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AiProvider for MockAiProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        self.simulate_latency().await;

        // 检查是否应该失败
        if let Some(msg) = self.should_fail.lock().unwrap().take() {
            return Err(CisError::ai(format!("Mock completion failed: {}", msg)));
        }

        // 检查是否有预设响应
        let responses = self.completion_responses.read().await;
        
        // 尝试精确匹配
        if let Some(response) = responses.get(&request.prompt) {
            return Ok(response.clone());
        }
        
        // 尝试部分匹配
        for (key, response) in responses.iter() {
            if request.prompt.contains(key) || key.contains(&request.prompt) {
                return Ok(response.clone());
            }
        }
        drop(responses);

        // 返回默认响应
        Ok(CompletionResponse {
            text: self.default_response.lock().unwrap().clone(),
            usage: Some(TokenUsage {
                prompt_tokens: request.prompt.len() as u64 / 4,
                completion_tokens: 10,
                total_tokens: request.prompt.len() as u64 / 4 + 10,
            }),
            model: self.name.lock().unwrap().clone(),
            finish_reason: Some("stop".to_string()),
            id: format!("mock-completion-{}", uuid::Uuid::new_v4()),
            created_at: current_timestamp(),
        })
    }

    async fn embedding(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        self.simulate_latency().await;

        // 检查是否应该失败
        if let Some(msg) = self.should_fail.lock().unwrap().take() {
            return Err(CisError::ai(format!("Mock embedding failed: {}", msg)));
        }

        // 检查是否有预设响应（使用第一个文本作为 key）
        if let Some(first_text) = request.texts.first() {
            let responses = self.embedding_responses.read().await;
            if let Some(response) = responses.get(first_text) {
                return Ok(response.clone());
            }
        }

        // 生成默认嵌入
        let dimension = request.dimensions.unwrap_or(768);
        let embeddings: Vec<Vec<f32>> = request
            .texts
            .iter()
            .map(|_| self.default_embedding.lock().unwrap().clone())
            .collect();

        Ok(EmbeddingResponse {
            embeddings,
            usage: Some(TokenUsage {
                prompt_tokens: request.texts.iter().map(|t| t.len() as u64 / 4).sum(),
                completion_tokens: 0,
                total_tokens: request.texts.iter().map(|t| t.len() as u64 / 4).sum(),
            }),
            model: format!("{}-embedding", self.name.lock().unwrap()),
            dimension,
            id: format!("mock-embedding-{}", uuid::Uuid::new_v4()),
        })
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(self.models.lock().unwrap().clone())
    }

    async fn health_check(&self) -> Result<bool> {
        if let Some(msg) = self.should_fail.lock().unwrap().take() {
            return Err(CisError::ai(format!("Health check failed: {}", msg)));
        }
        Ok(*self.available.lock().unwrap())
    }

    fn default_model(&self) -> Result<ModelInfo> {
        let models = self.models.lock().unwrap();
        models.first()
            .cloned()
            .ok_or_else(|| CisError::ai("No models available".to_string()))
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_complete() {
        let mock = MockAiProvider::new();

        let request = CompletionRequest::new("Hello");
        let response = mock.complete(request).await.unwrap();

        assert!(!response.text.is_empty());
        assert!(response.usage.is_some());
    }

    #[tokio::test]
    async fn test_mock_preset_completion() {
        let mock = MockAiProvider::new();
        
        let preset = CompletionResponse {
            text: "Custom response".to_string(),
            usage: None,
            model: "test".to_string(),
            finish_reason: None,
            id: "test-1".to_string(),
            created_at: 0,
        };
        
        mock.preset_completion("Hello", preset.clone()).await;

        let request = CompletionRequest::new("Hello");
        let response = mock.complete(request).await.unwrap();

        assert_eq!(response.text, "Custom response");
    }

    #[tokio::test]
    async fn test_mock_embedding() {
        let mock = MockAiProvider::new();

        let request = EmbeddingRequest::new(vec![
            "Hello".to_string(),
            "World".to_string(),
        ]);
        let response = mock.embedding(request).await.unwrap();

        assert_eq!(response.embeddings.len(), 2);
        assert_eq!(response.embeddings[0].len(), 768);
    }

    #[tokio::test]
    async fn test_mock_health_check() {
        let mock = MockAiProvider::new();
        
        assert!(mock.health_check().await.unwrap());
        
        mock.set_available(false);
        assert!(!mock.health_check().await.unwrap());
    }

    #[tokio::test]
    async fn test_mock_list_models() {
        let mock = MockAiProvider::new();
        
        let models = mock.list_models().await.unwrap();
        assert!(!models.is_empty());
        assert_eq!(models[0].id, "mock-model");
    }

    #[tokio::test]
    async fn test_mock_default_model() {
        let mock = MockAiProvider::new();
        
        let model = mock.default_model().unwrap();
        assert_eq!(model.id, "mock-model");
    }

    #[tokio::test]
    async fn test_mock_error() {
        let mock = MockAiProvider::new();
        mock.will_fail("Simulated error");

        let request = CompletionRequest::new("Test");
        let result = mock.complete(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_latency() {
        let mock = MockAiProvider::with_latency(100);

        let start = std::time::Instant::now();
        let request = CompletionRequest::new("Test");
        mock.complete(request).await.unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed >= std::time::Duration::from_millis(100));
    }
}
