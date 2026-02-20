# TASK 7.6: P2P 跨设备 Agent 调用

> **Phase**: 7 - 多 Agent 架构 (P3 可选)
> **状态**: 🔄 进行中 (占位符已创建)
> **负责人**: TBD
> **周期**: Week 17

---

## 任务概述

实现基于 P2P 的跨设备 Agent 调用，使 Agent 可在网络中的任何设备上执行。

## 工作内容

### 1. Remote Agent 定义

```rust
// crates/cis-p2p/src/remote_agent.rs
#[cfg(feature = "remote-agent")]
pub struct RemoteAgentProxy {
    peer_id: PeerId,
    agent_type: AgentType,
    network: Arc<dyn Network>,
    timeout: Duration,
}

#[cfg(feature = "remote-agent")]
#[async_trait]
impl Agent for RemoteAgentProxy {
    fn agent_type(&self) -> AgentType {
        self.agent_type.clone()
    }
    
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::Remote
    }
    
    async fn turn(&mut self, user_message: &str) -> anyhow::Result<String> {
        // 构建 RPC 请求
        let request = AgentRequest {
            agent_type: self.agent_type.clone(),
            message: user_message.to_string(),
            timestamp: SystemTime::now(),
        };
        
        // 序列化
        let request_bytes = bincode::serialize(&request)?;
        
        // 发送请求并等待响应
        let response_bytes = self.network
            .request(&self.peer_id, AGENT_PROTOCOL, request_bytes, self.timeout)
            .await?;
        
        // 解析响应
        let response: AgentResponse = bincode::deserialize(&response_bytes)?;
        
        match response.status {
            ResponseStatus::Success => Ok(response.content),
            ResponseStatus::Error => Err(anyhow!("Remote agent error: {}", response.content)),
            ResponseStatus::Timeout => Err(anyhow!("Remote agent timeout")),
        }
    }
    
    fn can_handle(&self, task_type: &TaskType) -> bool {
        // Remote agent 声明可处理的任务类型
        true  // 代理到远端决定
    }
}
```

### 2. Agent 服务注册

```rust
// crates/cis-p2p/src/agent_service.rs
#[cfg(feature = "remote-agent")]
pub struct AgentService {
    local_agents: HashMap<AgentType, Box<dyn Agent>>,
    network: Arc<dyn Network>,
}

#[cfg(feature = "remote-agent")]
impl AgentService {
    pub fn new(network: Arc<dyn Network>) -> Self {
        Self {
            local_agents: HashMap::new(),
            network,
        }
    }
    
    pub fn register_agent(&mut self, agent: Box<dyn Agent>) {
        let agent_type = agent.agent_type();
        info!("Registering local agent: {:?}", agent_type);
        self.local_agents.insert(agent_type, agent);
    }
    
    pub async fn start(&self) -> Result<(), NetworkError> {
        // 注册协议处理器
        let protocol = AGENT_PROTOCOL;
        self.network.register_handler(protocol, |peer_id, request| {
            Box::pin(self.handle_request(peer_id, request))
        }).await?;
        
        // 广播 Agent 能力
        self.announce_capabilities().await?;
        
        Ok(())
    }
    
    async fn handle_request(
        &self,
        peer_id: PeerId,
        request_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, NetworkError> {
        let request: AgentRequest = match bincode::deserialize(&request_bytes) {
            Ok(r) => r,
            Err(e) => {
                let response = AgentResponse::error(format!("Invalid request: {}", e));
                return Ok(bincode::serialize(&response).unwrap());
            }
        };
        
        // 查找 Agent
        let mut agent = match self.local_agents.get(&request.agent_type) {
            Some(a) => a,
            None => {
                let response = AgentResponse::error(format!(
                    "Agent {:?} not available", 
                    request.agent_type
                ));
                return Ok(bincode::serialize(&response).unwrap());
            }
        };
        
        // 执行
        let response = match agent.turn(&request.message).await {
            Ok(content) => AgentResponse::success(content),
            Err(e) => AgentResponse::error(e.to_string()),
        };
        
        Ok(bincode::serialize(&response).unwrap())
    }
    
    async fn announce_capabilities(&self) -> Result<(), NetworkError> {
        let capabilities: Vec<AgentType> = self.local_agents.keys().cloned().collect();
        
        let announcement = AgentAnnouncement {
            peer_id: self.network.local_peer_id().clone(),
            available_agents: capabilities,
            timestamp: SystemTime::now(),
        };
        
        self.network.broadcast(
            AGENT_ANNOUNCE_TOPIC,
            bincode::serialize(&announcement).unwrap(),
        ).await?;
        
        Ok(())
    }
}
```

### 3. 发现远程 Agent

```rust
// crates/cis-p2p/src/agent_discovery.rs
#[cfg(feature = "remote-agent")]
pub struct AgentDiscovery {
    network: Arc<dyn Network>,
    known_agents: RwLock<HashMap<AgentType, Vec<RemoteAgentInfo>>>,
}

#[cfg(feature = "remote-agent")]
#[derive(Debug, Clone)]
pub struct RemoteAgentInfo {
    pub peer_id: PeerId,
    pub agent_type: AgentType,
    pub last_seen: SystemTime,
    pub latency: Option<Duration>,
}

#[cfg(feature = "remote-agent")]
impl AgentDiscovery {
    pub async fn find_agent(&self, agent_type: &AgentType) -> Option<RemoteAgentInfo> {
        let agents = self.known_agents.read().await;
        
        agents.get(agent_type)?
            .iter()
            .min_by_key(|a| a.latency.unwrap_or(Duration::MAX))
            .cloned()
    }
    
    pub async fn refresh(&self) -> Result<(), NetworkError> {
        // 发送发现请求
        self.network.broadcast(
            AGENT_DISCOVERY_TOPIC,
            bincode::serialize(&DiscoveryRequest::All).unwrap(),
        ).await?;
        
        Ok(())
    }
}
```

### 4. 在 AgentPool 中集成 Remote Agent

```rust
// crates/cis-core/src/agent/pool.rs
#[cfg(feature = "multi-agent")]
impl AgentPool {
    pub async fn acquire(&self, agent_type: AgentType) -> Result<Box<dyn Agent>, PoolError> {
        // 1. 尝试获取本地 Agent
        if let Some(agent) = self.try_get_local(&agent_type).await {
            return Ok(agent);
        }
        
        // 2. 检查是否可创建新的本地 Agent
        if self.can_create_local(&agent_type).await {
            return self.create_local(&agent_type).await;
        }
        
        // 3. 尝试获取远程 Agent 代理
        #[cfg(feature = "remote-agent")]
        if let Some(remote_info) = self.discovery.find_agent(&agent_type).await {
            info!("Using remote agent at {:?}", remote_info.peer_id);
            return Ok(Box::new(RemoteAgentProxy::new(
                remote_info.peer_id,
                agent_type,
                self.network.clone(),
            )));
        }
        
        Err(PoolError::AgentNotAvailable(agent_type))
    }
}
```

### 5. 配置 Feature Flag

```toml
# crates/cis-p2p/Cargo.toml
[features]
default = []
remote-agent = ["dep:cis-traits/multi-agent"]

# crates/cis-core/Cargo.toml
[features]
multi-agent = [
    # ... 其他特性
    "cis-p2p/remote-agent",
]
```

## 验收标准

- [ ] 本地 Agent 可注册为网络服务
- [ ] 远程 Agent 可通过 P2P 发现
- [ ] Agent 调用可透明路由到远程设备
- [ ] 支持超时和错误处理
- [ ] 设备发现正常工作
- [ ] 端到端测试通过

## 依赖

- Task 7.4 (DAG 编排)
- Task 2.5 (cis-p2p)

## 阻塞

- TASK_7_7 (多 Agent 集成测试)

---
