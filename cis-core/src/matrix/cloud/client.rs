//! # Cloud Anchor 客户端实现

use super::{
    CloudAnchorConfig, CloudAnchorError, CloudAnchorResult, DiscoveredPeer, HolePunchInfo,
    HolePunchRequest, HolePunchResponse, NatType, NodeRegistration, PunchCoordination,
    RegistrationResponse, RelayMessage, QuotaInfo,
    types::{HeartbeatRequest, HeartbeatResponse},
};
use reqwest::{Client, ClientBuilder, Response, StatusCode};
use serde::Deserialize;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock as AsyncRwLock;
use tokio::task::JoinHandle;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Cloud Anchor 客户端
///
/// 提供云端节点注册、发现、NAT 穿透协助和消息中继功能。
pub struct CloudAnchorClient {
    /// 配置
    config: CloudAnchorConfig,
    /// HTTP 客户端
    http_client: Client,
    /// 当前注册令牌
    token: AsyncRwLock<Option<String>>,
    /// 注册的节点 ID
    node_id: AsyncRwLock<Option<String>>,
    /// 是否已注册
    is_registered: AtomicBool,
    /// 最后一次心跳时间
    last_heartbeat: AtomicU64,
    /// 心跳任务句柄
    heartbeat_handle: AsyncRwLock<Option<JoinHandle<()>>>,
    /// 配额使用情况
    quota_used: AtomicU64,
    /// 统计信息
    stats: Arc<RwLock<ClientStats>>,
    /// 配额信息缓存
    quota_cache: AsyncRwLock<Option<(QuotaInfo, Instant)>>,
    /// 配额缓存有效期（秒）
    quota_cache_ttl: Duration,
}

/// 客户端统计信息
#[derive(Debug, Default, Clone)]
pub struct ClientStats {
    pub register_count: u64,
    pub heartbeat_count: u64,
    pub query_count: u64,
    pub relay_count: u64,
    pub relay_bytes_sent: u64,
    pub relay_bytes_received: u64,
    pub hole_punch_count: u64,
    pub error_count: u64,
}

impl CloudAnchorClient {
    /// 创建新的 Cloud Anchor 客户端
    pub fn new(config: CloudAnchorConfig) -> CloudAnchorResult<Self> {
        if config.server_url.is_empty() {
            return Err(CloudAnchorError::configuration(
                "Server URL cannot be empty",
            ));
        }

        let http_client = ClientBuilder::new()
            .timeout(Duration::from_secs(config.request_timeout))
            .connect_timeout(Duration::from_secs(config.connect_timeout))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| CloudAnchorError::http(format!("Failed to build HTTP client: {}", e)))?;

        Ok(Self {
            config,
            http_client,
            token: AsyncRwLock::new(None),
            node_id: AsyncRwLock::new(None),
            is_registered: AtomicBool::new(false),
            last_heartbeat: AtomicU64::new(0),
            heartbeat_handle: AsyncRwLock::new(None),
            quota_used: AtomicU64::new(0),
            stats: Arc::new(RwLock::new(ClientStats::default())),
            quota_cache: AsyncRwLock::new(None),
            quota_cache_ttl: Duration::from_secs(60),
        })
    }

    /// 从配置创建客户端（便捷方法）
    pub fn with_url(server_url: impl Into<String>) -> CloudAnchorResult<Self> {
        let config = CloudAnchorConfig::new(server_url);
        Self::new(config)
    }

    /// 检查是否已注册
    pub fn is_registered(&self) -> bool {
        self.is_registered.load(Ordering::SeqCst)
    }

    /// 获取当前节点 ID
    pub async fn current_node_id(&self) -> Option<String> {
        self.node_id.read().await.clone()
    }

    /// 获取配置
    pub fn config(&self) -> &CloudAnchorConfig {
        &self.config
    }

    /// 更新配置
    pub fn update_config(&mut self, config: CloudAnchorConfig) {
        self.config = config;
    }

    /// 获取统计信息
    pub fn stats(&self) -> ClientStats {
        self.stats.read().unwrap().clone()
    }

    // ==================== 注册 API ====================

    /// 注册节点到 Cloud Anchor
    pub async fn register(&self, reg: NodeRegistration) -> CloudAnchorResult<RegistrationResponse> {
        info!("Registering node {} to Cloud Anchor", reg.node_id);

        let url = format!("{}/nodes/register", self.config.api_base_url());
        
        let response = self
            .http_client
            .post(&url)
            .json(&reg)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let result = self.handle_response::<RegistrationResponse>(response).await;
        
        match &result {
            Ok(reg_resp) if reg_resp.success => {
                info!("Node {} registered successfully", reg.node_id);
                
                // 保存令牌和节点 ID
                if let Some(token) = &reg_resp.token {
                    *self.token.write().await = Some(token.clone());
                }
                *self.node_id.write().await = Some(reg.node_id.clone());
                self.is_registered.store(true, Ordering::SeqCst);
                
                // 更新统计
                self.stats.write().unwrap().register_count += 1;

                // 如果配置了自动心跳，启动心跳任务
                if self.config.auto_register {
                    self.start_heartbeat(reg.node_id.clone()).await?;
                }
                
                Ok(reg_resp.clone())
            }
            Ok(reg_resp) => {
                error!("Registration failed: {:?}", reg_resp.error);
                Err(CloudAnchorError::registration(
                    reg_resp.error.clone().unwrap_or_else(|| "Unknown error".to_string())
                ))
            }
            Err(e) => {
                self.stats.write().unwrap().error_count += 1;
                Err(e.clone())
            }
        }
    }

    /// 注销节点
    pub async fn unregister(&self) -> CloudAnchorResult<()> {
        let node_id = self.node_id.read().await.clone();
        let token = self.token.read().await.clone();

        if let (Some(node_id), Some(token)) = (node_id, token) {
            info!("Unregistering node {} from Cloud Anchor", node_id);

            // 停止心跳
            self.stop_heartbeat().await;

            let url = format!("{}/nodes/{}/unregister", self.config.api_base_url(), node_id);
            
            let response = self
                .http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .map_err(CloudAnchorError::from)?;

            self.handle_empty_response(response).await?;

            // 清除状态
            self.is_registered.store(false, Ordering::SeqCst);
            *self.token.write().await = None;
            *self.node_id.write().await = None;

            info!("Node {} unregistered successfully", node_id);
        }

        Ok(())
    }

    // ==================== 心跳 API ====================

    /// 发送心跳
    pub async fn heartbeat(&self, node_id: &str) -> CloudAnchorResult<()> {
        let token = self
            .token
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!("{}/nodes/{}/heartbeat", self.config.api_base_url(), node_id);

        let request = HeartbeatRequest {
            node_id: node_id.to_string(),
            token,
            current_public_addr: None,
            active_connections: 0,
            system_load: None,
        };

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let resp = self.handle_response::<HeartbeatResponse>(response).await?;

        if resp.success {
            self.last_heartbeat.store(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                Ordering::SeqCst,
            );
            self.stats.write().unwrap().heartbeat_count += 1;

            // 如果服务器返回了新令牌，更新它
            if let Some(new_token) = resp.new_token {
                *self.token.write().await = Some(new_token);
            }

            debug!("Heartbeat sent successfully for node {}", node_id);
            Ok(())
        } else {
            Err(CloudAnchorError::authentication(
                resp.error.unwrap_or_else(|| "Heartbeat failed".to_string())
            ))
        }
    }

    /// 启动自动心跳
    async fn start_heartbeat(&self, node_id: String) -> CloudAnchorResult<()> {
        let mut handle = self.heartbeat_handle.write().await;
        
        // 如果已经有心跳任务，先停止
        if let Some(h) = handle.take() {
            h.abort();
        }

        let interval_secs = self.config.heartbeat_interval;
        let node_id_clone = node_id.clone();
        
        // 创建客户端的克隆用于在任务中使用
        let http_client = self.http_client.clone();
        let api_base_url = self.config.api_base_url();
        let token = self.token.read().await.clone();
        let stats = Arc::clone(&self.stats);
        let last_heartbeat = Arc::new(AtomicU64::new(0));
        
        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(interval_secs));
            
            loop {
                interval.tick().await;
                
                let url = format!("{}/nodes/{}/heartbeat", api_base_url, node_id_clone);
                
                if let Some(ref token) = token {
                    let request = HeartbeatRequest {
                        node_id: node_id_clone.clone(),
                        token: token.clone(),
                        current_public_addr: None,
                        active_connections: 0,
                        system_load: None,
                    };

                    match http_client.post(&url).json(&request).send().await {
                        Ok(response) if response.status().is_success() => {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            last_heartbeat.store(now, Ordering::SeqCst);
                            stats.write().unwrap().heartbeat_count += 1;
                            debug!("Heartbeat sent successfully for node {}", node_id_clone);
                        }
                        Ok(response) => {
                            warn!("Heartbeat failed with status: {}", response.status());
                        }
                        Err(e) => {
                            warn!("Heartbeat request failed: {}", e);
                        }
                    }
                }
            }
        });

        *handle = Some(task);
        info!("Heartbeat task started for node {} (interval: {}s)", node_id, interval_secs);
        
        Ok(())
    }

    /// 停止心跳
    async fn stop_heartbeat(&self) {
        let mut handle = self.heartbeat_handle.write().await;
        if let Some(h) = handle.take() {
            h.abort();
            info!("Heartbeat task stopped");
        }
    }

    // ==================== 发现 API ====================

    /// 查询特定节点
    pub async fn query_peer(&self, node_id: &str) -> CloudAnchorResult<Option<DiscoveredPeer>> {
        let url = format!("{}/nodes/{}", self.config.api_base_url(), node_id);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        match response.status() {
            StatusCode::OK => {
                let peer = response
                    .json::<DiscoveredPeer>()
                    .await
                    .map_err(|e| CloudAnchorError::serialization(e.to_string()))?;
                self.stats.write().unwrap().query_count += 1;
                Ok(Some(peer))
            }
            StatusCode::NOT_FOUND => Ok(None),
            _ => Err(self.parse_error_response(response).await),
        }
    }

    /// 列出可连接的节点
    pub async fn list_peers(&self, room_id: Option<&str>) -> CloudAnchorResult<Vec<DiscoveredPeer>> {
        let mut url = format!("{}/nodes", self.config.api_base_url());
        
        if let Some(room) = room_id {
            url.push_str(&format!("?room_id={}", room));
        } else if let Some(ref room) = self.config.room_id {
            url.push_str(&format!("?room_id={}", room));
        }

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let peers = self.handle_response::<Vec<DiscoveredPeer>>(response).await?;
        self.stats.write().unwrap().query_count += 1;
        
        debug!("Discovered {} peers", peers.len());
        Ok(peers)
    }

    /// 按标签搜索节点
    pub async fn search_peers_by_tags(&self, tags: &[String]) -> CloudAnchorResult<Vec<DiscoveredPeer>> {
        let url = format!("{}/nodes/search", self.config.api_base_url());

        let response = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({ "tags": tags }))
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        self.handle_response::<Vec<DiscoveredPeer>>(response).await
    }

    // ==================== NAT 穿透 API ====================

    /// 请求打洞协调
    pub async fn request_hole_punch(
        &self,
        target_node: &str,
    ) -> CloudAnchorResult<PunchCoordination> {
        let node_id = self
            .node_id
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!("{}/hole-punch/request", self.config.api_base_url());

        let session_id = uuid::Uuid::new_v4().to_string();
        let request = HolePunchRequest::new(&node_id, target_node, 
            "0.0.0.0:0".parse().unwrap(), // 服务器会通过连接获取实际地址
            session_id
        );

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let coordination = self.handle_response::<PunchCoordination>(response).await?;
        self.stats.write().unwrap().hole_punch_count += 1;
        
        info!(
            "Hole punch coordination received for session {} with {}",
            coordination.session_id, target_node
        );
        
        Ok(coordination)
    }

    /// 查询打洞响应（作为目标节点）
    pub async fn poll_hole_punch_requests(
        &self,
    ) -> CloudAnchorResult<Vec<HolePunchRequest>> {
        let node_id = self
            .node_id
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!(
            "{}/hole-punch/requests/{}",
            self.config.api_base_url(),
            node_id
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        self.handle_response::<Vec<HolePunchRequest>>(response).await
    }

    /// 接受打洞请求（作为目标节点）
    pub async fn accept_hole_punch(
        &self,
        session_id: &str,
        my_public_addr: SocketAddr,
        my_local_addr: Option<SocketAddr>,
        nat_type: NatType,
    ) -> CloudAnchorResult<HolePunchResponse> {
        let url = format!("{}/hole-punch/accept/{}", self.config.api_base_url(), session_id);

        let response = self
            .http_client
            .post(&url)
            .json(&serde_json::json!({
                "public_addr": my_public_addr,
                "local_addr": my_local_addr,
                "nat_type": nat_type,
            }))
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        self.handle_response::<HolePunchResponse>(response).await
    }

    /// 报告打洞结果
    pub async fn report_hole_punch_result(
        &self,
        info: &HolePunchInfo,
    ) -> CloudAnchorResult<()> {
        let url = format!("{}/hole-punch/report", self.config.api_base_url());

        let response = self
            .http_client
            .post(&url)
            .json(info)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        self.handle_empty_response(response).await
    }

    // ==================== 中继 API ====================

    /// 通过 Cloud Anchor 中继消息
    pub async fn relay_message(
        &self,
        target_node: &str,
        message: &[u8],
        msg_type: &str,
    ) -> CloudAnchorResult<()> {
        if !self.config.enable_relay {
            return Err(CloudAnchorError::relay("Relay is disabled"));
        }

        // 检查配额
        let msg_size = message.len() as u64;
        let current_usage = self.quota_used.load(Ordering::SeqCst);
        
        if let Some(quota) = self.config.relay_quota_bytes {
            if current_usage + msg_size > quota {
                return Err(CloudAnchorError::quota_exceeded(
                    "Relay quota would be exceeded"
                ));
            }
        }

        let node_id = self
            .node_id
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!("{}/relay", self.config.api_base_url());

        // Base64 编码
        let payload = base64::encode(message);
        let relay_msg = RelayMessage::new(&node_id, target_node, msg_type, payload);

        let response = self
            .http_client
            .post(&url)
            .json(&relay_msg)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        self.handle_empty_response(response).await?;

        // 更新配额使用
        self.quota_used.fetch_add(msg_size, Ordering::SeqCst);
        self.stats.write().unwrap().relay_count += 1;
        self.stats.write().unwrap().relay_bytes_sent += msg_size;

        debug!("Message relayed to {} ({} bytes)", target_node, msg_size);
        Ok(())
    }

    /// 接收中继消息（轮询）
    pub async fn poll_relay_messages(&self) -> CloudAnchorResult<Vec<RelayMessage>> {
        let node_id = self
            .node_id
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!("{}/relay/{}", self.config.api_base_url(), node_id);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let messages = self.handle_response::<Vec<RelayMessage>>(response).await?;
        
        // 更新接收字节数统计
        let total_bytes: u64 = messages.iter().map(|m| m.size as u64).sum();
        self.stats.write().unwrap().relay_bytes_received += total_bytes;
        
        Ok(messages)
    }

    /// 获取配额信息（带缓存）
    pub async fn get_quota(&self) -> CloudAnchorResult<QuotaInfo> {
        // 检查缓存是否有效
        {
            let cache = self.quota_cache.read().await;
            if let Some((quota_info, timestamp)) = cache.as_ref() {
                if timestamp.elapsed() < self.quota_cache_ttl {
                    return Ok(quota_info.clone());
                }
            }
        }

        // 缓存无效或过期，从 API 获取
        self.fetch_quota_from_api().await
    }

    /// 强制刷新配额信息
    pub async fn refresh_quota(&self) -> CloudAnchorResult<QuotaInfo> {
        self.fetch_quota_from_api().await
    }

    /// 从 API 获取配额信息
    async fn fetch_quota_from_api(&self) -> CloudAnchorResult<QuotaInfo> {
        let node_id = self
            .node_id
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let token = self
            .token
            .read()
            .await
            .clone()
            .ok_or(CloudAnchorError::NotInitialized)?;

        let url = format!("{}/nodes/{}/quota", self.config.api_base_url(), node_id);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(CloudAnchorError::from)?;

        let quota_response: QuotaResponse = self.handle_response(response).await?;
        
        let quota_info = QuotaInfo {
            total_bytes: quota_response.total_bytes,
            used_bytes: quota_response.used_bytes,
            remaining_bytes: quota_response.remaining_bytes,
            reset_at: quota_response.reset_at,
        };

        // 更新缓存
        *self.quota_cache.write().await = Some((quota_info.clone(), Instant::now()));

        Ok(quota_info)
    }

    /// 获取配额使用情况（实时查询）
    pub async fn get_quota_usage(&self) -> CloudAnchorResult<QuotaInfo> {
        self.fetch_quota_from_api().await
    }

    /// 清除配额缓存
    pub async fn clear_quota_cache(&self) {
        *self.quota_cache.write().await = None;
    }

    // ==================== 辅助方法 ====================

    /// 处理 HTTP 响应
    async fn handle_response<T: for<'de> Deserialize<'de>>(&self, response: Response) -> CloudAnchorResult<T> {
        let status = response.status();
        
        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|e| CloudAnchorError::serialization(e.to_string()))
        } else {
            Err(self.parse_error_response(response).await)
        }
    }

    /// 处理空响应
    async fn handle_empty_response(&self, response: Response) -> CloudAnchorResult<()> {
        let status = response.status();
        
        if status.is_success() {
            Ok(())
        } else {
            Err(self.parse_error_response(response).await)
        }
    }

    /// 解析错误响应
    async fn parse_error_response(&self, response: Response) -> CloudAnchorError {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());

        match status {
            StatusCode::UNAUTHORIZED => CloudAnchorError::authentication(text),
            StatusCode::NOT_FOUND => CloudAnchorError::NodeNotFound(text),
            StatusCode::TOO_MANY_REQUESTS => CloudAnchorError::quota_exceeded(text),
            StatusCode::REQUEST_TIMEOUT => CloudAnchorError::timeout(text),
            _ => CloudAnchorError::ServerError {
                status: status.as_u16(),
                message: text,
            },
        }
    }

    /// 获取最后一次心跳时间
    pub fn last_heartbeat_time(&self) -> Option<u64> {
        let ts = self.last_heartbeat.load(Ordering::SeqCst);
        if ts > 0 {
            Some(ts)
        } else {
            None
        }
    }

    /// 获取当前配额使用量
    pub fn quota_used(&self) -> u64 {
        self.quota_used.load(Ordering::SeqCst)
    }

    /// 重置配额计数
    pub fn reset_quota(&self) {
        self.quota_used.store(0, Ordering::SeqCst);
    }

    /// 关闭客户端
    pub async fn shutdown(&self) -> CloudAnchorResult<()> {
        info!("Shutting down Cloud Anchor client");
        
        // 停止心跳
        self.stop_heartbeat().await;
        
        // 注销节点
        if self.is_registered() {
            self.unregister().await?;
        }
        
        Ok(())
    }
}

/// 配额响应
#[derive(Debug, Clone, Deserialize)]
pub struct QuotaResponse {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub remaining_bytes: u64,
    pub reset_at: u64,
}

/// 便捷的 base64 编码模块
mod base64 {
    pub fn encode(input: &[u8]) -> String {
        const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        
        let mut result = String::with_capacity((input.len() + 2) / 3 * 4);
        
        for chunk in input.chunks(3) {
            let b = match chunk.len() {
                1 => [chunk[0], 0, 0],
                2 => [chunk[0], chunk[1], 0],
                3 => [chunk[0], chunk[1], chunk[2]],
                _ => unreachable!(),
            };
            
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
            
            result.push(BASE64_CHARS[((n >> 18) & 0x3F) as usize] as char);
            result.push(BASE64_CHARS[((n >> 12) & 0x3F) as usize] as char);
            
            if chunk.len() > 1 {
                result.push(BASE64_CHARS[((n >> 6) & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
            
            if chunk.len() > 2 {
                result.push(BASE64_CHARS[(n & 0x3F) as usize] as char);
            } else {
                result.push('=');
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64::encode(b"hello"), "aGVsbG8=");
        assert_eq!(base64::encode(b"a"), "YQ==");
        assert_eq!(base64::encode(b"ab"), "YWI=");
        assert_eq!(base64::encode(b"abc"), "YWJj");
    }

    #[test]
    fn test_client_creation() {
        let config = CloudAnchorConfig::new("https://anchor.example.com");
        let client = CloudAnchorClient::new(config).unwrap();
        
        assert!(!client.is_registered());
        assert_eq!(client.config().server_url, "https://anchor.example.com");
    }

    #[test]
    fn test_client_stats() {
        let config = CloudAnchorConfig::new("https://anchor.example.com");
        let client = CloudAnchorClient::new(config).unwrap();
        
        let stats = client.stats();
        assert_eq!(stats.register_count, 0);
        assert_eq!(stats.heartbeat_count, 0);
    }

    #[test]
    fn test_quota_tracking() {
        let config = CloudAnchorConfig::new("https://anchor.example.com");
        let client = CloudAnchorClient::new(config).unwrap();
        
        assert_eq!(client.quota_used(), 0);
        
        // 模拟配额使用（实际使用需要异步环境）
        client.quota_used.fetch_add(1000, Ordering::SeqCst);
        assert_eq!(client.quota_used(), 1000);
        
        client.reset_quota();
        assert_eq!(client.quota_used(), 0);
    }
}
