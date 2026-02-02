//! 飞书 API 客户端
//!
//! 提供飞书开放平台 API 的调用封装

use reqwest::{Client, Error as ReqwestError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use thiserror::Error;
use chrono::{DateTime, Utc};

/// 飞书 API 错误
#[derive(Error, Debug)]
pub enum FeishuApiError {
    #[error("HTTP 请求错误: {0}")]
    Request(#[from] ReqwestError),

    #[error("JSON 解析错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("API 错误: code={0}, msg={1}")]
    Api(i32, String),

    #[error("认证失败: {0}")]
    Auth(String),

    #[error("Token 过期")]
    TokenExpired,

    #[error("限流: {0}")]
    RateLimit(String),
}

/// 飞书 API 客户端
#[derive(Clone)]
pub struct FeishuApiClient {
    /// App ID
    app_id: String,
    /// App Secret
    app_secret: String,
    /// 访问令牌
    access_token: Arc<RwLock<Option<String>>>,
    /// HTTP 客户端
    client: Client,
    /// API 基础 URL
    base_url: String,
}

/// 访问令牌响应
#[derive(Debug, Deserialize)]
struct TokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: Option<String>,
    expire: Option<i64>,
}

/// 消息列表响应
#[derive(Debug, Deserialize)]
struct MessagesResponse {
    code: i32,
    msg: String,
    data: Option<Value>,
}

/// 消息项
#[derive(Debug, Clone, Deserialize)]
pub struct FeishuMessage {
    /// 消息 ID
    pub message_id: String,
    /// 消息类型 (text, post, image, etc.)
    pub msg_type: String,
    /// 消息内容
    pub content: String,
    /// 发送者
    pub sender: FeishuSender,
    /// 创建时间（毫秒时间戳）
    pub create_time: String,
    /// 会话 ID
    pub chat_id: String,
    /// 更新时间（毫秒时间戳）
    pub update_time: String,
    /// 是否已删除
    pub deleted: bool,
}

/// 发送者信息
#[derive(Debug, Clone, Deserialize)]
pub struct FeishuSender {
    /// 用户 ID
    pub sender_id: String,
    /// 发送者类型
    pub sender_type: String,
    /// 租户密钥
    pub tenant_key: String,
}

/// 会话信息
#[derive(Debug, Clone, Deserialize)]
pub struct FeishuConversation {
    /// 会话 ID
    pub chat_id: String,
    /// 会话名称
    pub name: String,
    /// 会话类型 (p2p, group, public)
    pub chat_type: String,
    /// 描述
    pub description: Option<String>,
    /// 创建时间
    pub create_time: String,
    /// 是否外部会话
    pub external: bool,
}

impl FeishuApiClient {
    /// 创建新的 API 客户端
    pub fn new(app_id: String, app_secret: String) -> Self {
        Self {
            app_id,
            app_secret,
            access_token: Arc::new(RwLock::new(None)),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            base_url: "https://open.feishu.cn/open-apis".to_string(),
        }
    }

    /// 获取访问令牌
    pub async fn get_access_token(&self) -> Result<String, FeishuApiError> {
        // 检查缓存
        {
            let token = self.access_token.read().await;
            if let Some(ref t) = *token {
                return Ok(t.clone());
            }
        }

        // 获取新令牌
        let url = format!("{}/auth/v3/tenant_access_token/internal", self.base_url);
        let body = json!({
            "app_id": self.app_id,
            "app_secret": self.app_secret
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await?;

        let token_response: TokenResponse = response.json().await?;

        if token_response.code != 0 {
            return Err(FeishuApiError::Api(
                token_response.code,
                token_response.msg,
            ));
        }

        let token = token_response.tenant_access_token
            .ok_or_else(|| FeishuApiError::Auth("未返回 token".to_string()))?;

        // 缓存令牌
        let mut token_guard = self.access_token.write().await;
        *token_guard = Some(token.clone());

        Ok(token)
    }

    /// 刷新访问令牌
    pub async fn refresh_token(&self) -> Result<(), FeishuApiError> {
        let mut token_guard = self.access_token.write().await;
        *token_guard = None;
        drop(token_guard);

        self.get_access_token().await?;
        Ok(())
    }

    /// 获取消息列表
    ///
    /// # 参数
    /// - `container_id`: 会话 ID
    /// - `start_time`: 起始时间（毫秒时间戳，可选）
    /// - `page_size`: 分页大小
    pub async fn list_messages(
        &self,
        container_id: &str,
        start_time: Option<i64>,
        page_size: u32,
    ) -> Result<Vec<FeishuMessage>, FeishuApiError> {
        let token = self.get_access_token().await?;

        let mut url = format!(
            "{}/im/v1/messages?container_id_type=chat&container_id={}&page_size={}",
            self.base_url, container_id, page_size
        );

        if let Some(start) = start_time {
            url.push_str(&format!("&start_time={}", start));
        }

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let messages_response: MessagesResponse = response.json().await?;

        if messages_response.code == 99991663 {
            // Token 过期
            return Err(FeishuApiError::TokenExpired);
        } else if messages_response.code != 0 {
            return Err(FeishuApiError::Api(
                messages_response.code,
                messages_response.msg,
            ));
        }

        // 解析消息列表
        let data = messages_response.data.ok_or_else(|| {
            FeishuApiError::Api(-1, "未返回数据".to_string())
        })?;

        let items = data["items"]
            .as_array()
            .ok_or_else(|| FeishuApiError::Api(-1, "items 不是数组".to_string()))?;

        let messages: Result<Vec<FeishuMessage>, _> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()))
            .collect();

        messages.map_err(FeishuApiError::from)
    }

    /// 发送文本消息
    ///
    /// # 参数
    /// - `receive_id`: 接收者 ID（用户 ID 或会话 ID）
    /// - `receive_id_type`: 接收者类型 ("user", "chat", "email")
    /// - `content`: 消息内容
    pub async fn send_text_message(
        &self,
        receive_id: &str,
        receive_id_type: &str,
        content: &str,
    ) -> Result<String, FeishuApiError> {
        let token = self.get_access_token().await?;

        let url = format!("{}/im/v1/messages?receive_id_type={}", self.base_url, receive_id_type);

        let body = json!({
            "receive_id": receive_id,
            "msg_type": "text",
            "content": serde_json::json!({"text": content}).to_string()
        });

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;

        let result: Value = response.json().await?;

        if result["code"] == 99991663 {
            return Err(FeishuApiError::TokenExpired);
        } else if result["code"] != 0 {
            return Err(FeishuApiError::Api(
                result["code"].as_i64().unwrap_or(-1) as i32,
                result["msg"].as_str().unwrap_or("未知错误").to_string(),
            ));
        }

        let message_id = result["data"]["message_id"]
            .as_str()
            .ok_or_else(|| FeishuApiError::Api(-1, "未返回 message_id".to_string()))?;

        Ok(message_id.to_string())
    }

    /// 获取会话列表
    pub async fn list_conversations(&self) -> Result<Vec<FeishuConversation>, FeishuApiError> {
        let token = self.get_access_token().await?;

        let url = format!("{}/im/v1/chats", self.base_url);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let result: Value = response.json().await?;

        if result["code"] == 99991663 {
            return Err(FeishuApiError::TokenExpired);
        } else if result["code"] != 0 {
            return Err(FeishuApiError::Api(
                result["code"].as_i64().unwrap_or(-1) as i32,
                result["msg"].as_str().unwrap_or("未知错误").to_string(),
            ));
        }

        let items = result["data"]["items"]
            .as_array()
            .ok_or_else(|| FeishuApiError::Api(-1, "items 不是数组".to_string()))?;

        let conversations: Result<Vec<FeishuConversation>, _> = items
            .iter()
            .map(|item| serde_json::from_value(item.clone()))
            .collect();

        conversations.map_err(FeishuApiError::from)
    }

    /// 获取用户信息
    pub async fn get_user_info(&self, user_id: &str) -> Result<Value, FeishuApiError> {
        let token = self.get_access_token().await?;

        let url = format!("{}/contact/v3/users/{}", self.base_url, user_id);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let result: Value = response.json().await?;

        if result["code"] != 0 {
            return Err(FeishuApiError::Api(
                result["code"].as_i64().unwrap_or(-1) as i32,
                result["msg"].as_str().unwrap_or("未知错误").to_string(),
            ));
        }

        Ok(result["data"].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_api_client_creation() {
        let client = FeishuApiClient::new(
            "test_app_id".to_string(),
            "test_app_secret".to_string(),
        );

        assert_eq!(client.app_id, "test_app_id");
        assert_eq!(client.app_secret, "test_app_secret");
    }
}
