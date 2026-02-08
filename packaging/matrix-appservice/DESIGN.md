# CIS Matrix AppService 设计文档

## 概述

CIS Matrix AppService 将 CIS 集成到 Matrix 生态中，作为 Element 客户端的扩展，实现：

- DAG 状态实时广播到 Matrix Room
- 通过 Matrix 命令控制 CIS (!cis run)
- 双向消息同步
- E2EE 端到端加密支持

## 架构设计

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Matrix 生态                                  │
│                                                                      │
│   ┌─────────────┐     ┌─────────────┐     ┌─────────────────────┐   │
│   │   Element   │     │   Element   │     │  Other Matrix       │   │
│   │   (Web)     │◄───►│  (Mobile)   │◄───►│  Clients            │   │
│   └──────┬──────┘     └──────┬──────┘     └──────────┬──────────┘   │
│          │                   │                       │              │
│          └───────────────────┼───────────────────────┘              │
│                              │                                      │
│                    ┌─────────▼──────────┐                          │
│                    │  Matrix Homeserver │                          │
│                    │  (Synapse/Dendrite)│                          │
│                    └─────────┬──────────┘                          │
│                              │                                      │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
                    ┌──────────▼──────────┐
                    │  CIS AppService     │
                    │  (Matrix Bridge)    │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │     CIS Node        │
                    │  (HTTP/WebSocket)   │
                    └─────────────────────┘
```

## 核心功能

### 1. Room 自动创建

当 DAG 运行时，自动创建/关联 Matrix Room：

```rust
pub struct DagRoomManager {
    /// DAG ID -> Room ID 映射
    dag_room_map: HashMap<String, String>,
    /// 自动创建 Room 的配置
    auto_create: bool,
    /// Room 命名模板
    room_name_template: String,
}

impl DagRoomManager {
    /// 为 DAG 创建 Room
    async fn create_room_for_dag(&self, dag_id: &str) -> Result<String> {
        let room_name = self.room_name_template.replace("{dag_id}", dag_id);
        
        let request = CreateRoomRequest {
            name: room_name,
            topic: format!("CIS DAG: {}", dag_id),
            preset: RoomPreset::PublicChat,
            initial_state: vec![
                RoomStateEvent::Encryption, // 启用 E2EE
            ],
        };
        
        self.matrix_client.create_room(request).await
    }
}
```

### 2. DAG 状态广播

将 DAG 执行状态实时广播到 Matrix Room：

```rust
pub enum DagStatusEvent {
    Started {
        dag_id: String,
        task_id: String,
        timestamp: DateTime<Utc>,
    },
    StepCompleted {
        dag_id: String,
        step_name: String,
        duration: Duration,
        output: Option<String>,
    },
    Failed {
        dag_id: String,
        step_name: Option<String>,
        error: String,
    },
    Completed {
        dag_id: String,
        duration: Duration,
        summary: String,
    },
}

impl DagStatusEvent {
    /// 转换为 Matrix 消息格式
    fn to_matrix_message(&self) -> RoomMessage {
        match self {
            Self::Started { dag_id, task_id, .. } => RoomMessage::text(format!(
                "🚀 **DAG 开始运行**\n\n" +
                "• ID: `{}`\n" +
                "• Task: `{}`",
                dag_id, task_id
            )),
            Self::StepCompleted { step_name, duration, output, .. } => {
                let mut msg = format!(
                    "✅ **步骤完成**: `{}` ({:?})",
                    step_name, duration
                );
                if let Some(out) = output {
                    msg.push_str(&format!("\n```\n{}\n```", out));
                }
                RoomMessage::text(msg)
            }
            Self::Failed { step_name, error, .. } => RoomMessage::text(format!(
                "❌ **执行失败**\n\n" +
                "• 步骤: `{}`\n" +
                "• 错误: ```{}```",
                step_name.as_deref().unwrap_or("N/A"), error
            )),
            Self::Completed { duration, summary, .. } => RoomMessage::text(format!(
                "🎉 **DAG 执行完成** ({:?})\n\n{}",
                duration, summary
            )),
        }
    }
}
```

### 3. 双向命令 (!cis)

在 Matrix Room 中使用 `!cis` 命令控制 CIS：

```
!cis run <dag-name> [args...]     # 运行 DAG
!cis status [dag-name]            # 查看状态
!cis logs <task-id>               # 查看日志
!cis list                         # 列出 DAG
!cis search <query>               # 搜索记忆
!cis help                         # 显示帮助
```

实现代码：

```rust
pub struct CisCommandHandler {
    cis_api: CisApiClient,
    matrix_client: MatrixClient,
}

#[async_trait]
impl CommandHandler for CisCommandHandler {
    async fn handle(&self, room_id: &str, sender: &str, command: &str, args: &[String]) {
        match command {
            "run" => self.handle_run(room_id, args).await,
            "status" => self.handle_status(room_id, args).await,
            "logs" => self.handle_logs(room_id, args).await,
            "list" => self.handle_list(room_id).await,
            "search" => self.handle_search(room_id, args).await,
            "help" => self.handle_help(room_id).await,
            _ => self.send_error(room_id, "未知命令").await,
        }
    }

    async fn handle_run(&self, room_id: &str, args: &[String]) {
        if args.is_empty() {
            self.send_error(room_id, "用法: !cis run <dag-name>").await;
            return;
        }
        
        let dag_name = &args[0];
        
        // 调用 CIS API
        match self.cis_api.run_dag(dag_name, None).await {
            Ok(task_id) => {
                self.matrix_client
                    .send_message(room_id, format!(
                        "✅ DAG `{}` 已启动\nTask ID: `{}`",
                        dag_name, task_id
                    ))
                    .await;
            }
            Err(e) => {
                self.send_error(room_id, &format!("启动失败: {}", e)).await;
            }
        }
    }
}
```

### 4. E2EE 支持

端到端加密确保消息安全：

```rust
pub struct E2EEManager {
    /// 设备密钥
    device_keys: DeviceKeys,
    /// 会话存储
    session_store: SqliteCryptoStore,
    /// Olm 账户
    olm_account: OlmAccount,
}

impl E2EEManager {
    /// 初始化 E2EE
    pub async fn initialize(&mut self) -> Result<()> {
        // 生成或加载设备密钥
        let identity_keys = self.olm_account.identity_keys();
        
        // 上传设备密钥到 Homeserver
        self.matrix_client.upload_device_keys(
            DeviceKeysUploadRequest {
                device_id: self.device_id.clone(),
                identity_key: identity_keys.curve25519,
                signing_key: identity_keys.ed25519,
            }
        ).await?;
        
        Ok(())
    }

    /// 加密消息
    pub async fn encrypt(&self, room_id: &str, plaintext: &str) -> Result<EncryptedEvent> {
        // 获取房间的加密会话
        let session = self.session_store.get_group_session(room_id).await?;
        
        // 加密消息
        let encrypted = session.encrypt(plaintext).await?;
        
        Ok(encrypted)
    }

    /// 解密消息
    pub async fn decrypt(&self, event: &EncryptedEvent) -> Result<String> {
        match event {
            EncryptedEvent::OlmV1Curve25519AesSha2(content) => {
                // 使用 Olm 会话解密
                let session = self.session_store
                    .get_olm_session(&content.sender_key)
                    .await?;
                
                session.decrypt(&content.ciphertext).await
            }
            EncryptedEvent::MegolmV1AesSha2(content) => {
                // 使用 Megolm 会话解密
                let session = self.session_store
                    .get_inbound_group_session(&content.room_id, &content.session_id)
                    .await?;
                
                session.decrypt(&content.ciphertext).await
            }
        }
    }
}
```

## 配置文件

```yaml
# appservice.yaml
id: cis-appservice
hs_token: <homeserver-token>
as_token: <appservice-token>
url: http://localhost:8080
sender_localpart: cis-bot
namespaces:
  users:
    - exclusive: false
      regex: '@cis_.*'
  rooms:
    - exclusive: false
      regex: '#cis-.*'
  aliases:
    - exclusive: false
      regex: '#cis-.*'

# cis-bridge.yaml
bridge:
  # Matrix Homeserver 地址
  homeserver_url: http://localhost:8008
  
  # CIS 节点地址
  cis_node_url: http://localhost:7676
  
  # 存储配置
  database:
    path: ./data/matrix-bridge.db
  
  # E2EE 配置
  encryption:
    enabled: true
    pickle_key: <encryption-key>
  
  # Room 管理
  room_management:
    auto_create: true
    name_template: "CIS: {dag_id}"
    topic_template: "DAG 执行日志: {dag_id}"
  
  # 状态广播
  status_broadcast:
    enabled: true
    
    # 广播级别
    levels:
      - started
      - step_completed
      - failed
      - completed
    
    # 格式化模板
    templates:
      started: "🚀 DAG `{dag_id}` 开始运行"
      step_completed: "✅ 步骤 `{step}` 完成 ({duration})"
      failed: "❌ DAG 执行失败: `{error}`"
      completed: "🎉 DAG 执行完成 ({duration})"
  
  # 命令配置
  commands:
    prefix: "!cis"
    require_admin: false
    allowed_users: []  # 空列表表示允许所有用户
```

## 部署指南

### 1. 注册 AppService

在 Matrix Homeserver 配置中添加：

```yaml
# homeserver.yaml (Synapse)
app_service_config_files:
  - /path/to/cis-appservice.yaml
```

### 2. 生成注册文件

```bash
# 生成 Appservice 注册文件
cis-matrix-bridge --generate-registration > cis-appservice.yaml
```

### 3. 启动 Bridge

```bash
# 启动 CIS Matrix Bridge
cis-matrix-bridge --config cis-bridge.yaml
```

### 4. Docker Compose 部署

```yaml
version: '3.8'
services:
  matrix-bridge:
    image: mosiyuan/cis-matrix-bridge:latest
    container_name: cis-matrix-bridge
    restart: unless-stopped
    volumes:
      - ./data:/data
      - ./cis-bridge.yaml:/app/config.yaml:ro
      - ./cis-appservice.yaml:/app/registration.yaml:ro
    environment:
      - RUST_LOG=info
    ports:
      - "8080:8080"
    networks:
      - cis-network
      - matrix-network

networks:
  cis-network:
    external: true
  matrix-network:
    external: true
```

## 使用示例

### 在 Element 中使用

1. **邀请 bot 加入 Room**
   ```
   /invite @cis-bot:example.com
   ```

2. **运行 DAG**
   ```
   !cis run deployment-pipeline
   ```

3. **查看状态**
   ```
   !cis status deployment-pipeline
   ```

4. **搜索记忆**
   ```
   !cis search "Docker 配置"
   ```

### 自动 Room 创建

DAG 配置中启用自动 Room 创建：

```toml
[dag]
name = "deployment-pipeline"

[matrix]
enabled = true
room_name = "部署流水线"
auto_create = true
invite_users = ["@admin:example.com", "@dev:example.com"]

[matrix.notifications]
on_start = true
on_complete = true
on_failure = true
```

## 安全考虑

1. **身份验证**: AppService 使用 token 验证请求
2. **权限控制**: 支持用户白名单和管理员权限
3. **E2EE**: 消息端到端加密
4. **速率限制**: 防止命令滥用

## 未来扩展

- 支持更多 Matrix 客户端功能（Reaction、Thread 等）
- DAG 可视化（通过 Matrix Widget）
- 语音/视频集成（Jitsi）
- 多 Homeserver 支持
