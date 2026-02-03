# CIS Skill 集成方案

**作者**: Claude (Sonnet 4.5)
**日期**: 2026-02-02
**状态**: 设计方案

---

## 执行摘要

本文档详细说明如何在 CIS 系统中实现 Skill 集成，包括 Skill 接口、生命周期管理、Matrix 集成和事件处理机制。

---

## 目录

1. [Skill 架构概览](#1-skill-架构概览)
2. [Skill 接口定义](#2-skill-接口定义)
3. [Skill 生命周期管理](#3-skill-生命周期管理)
4. [Matrix 集成](#4-matrix-集成)
5. [Skill 配置系统](#5-skill-配置系统)
6. [Skill 实现示例](#6-skill-实现示例)
7. [WASM Skill 支持](#7-wasm-skill-支持)
8. [事件联邦同步](#8-事件联邦同步)
9. [实现检查清单](#9-实现检查清单)
10. [开发指南](#10-开发指南)
11. [最佳实践](#11-最佳实践)
12. [故障排查](#12-故障排查)

---

## 1. Skill 架构概览

### 1.1 设计理念

CIS Skill 系统遵循以下核心原则：

- **热插拔**: 运行时加载/卸载 Skills，无需重启节点
- **Matrix Room 视图**: 每个 Skill 对应一个 Matrix Room，作为消息总线
- **联邦同步**: 支持跨节点 Skill 事件同步
- **沙箱隔离**: WASM Skills 提供安全隔离
- **本地主权**: Skill 数据库独立，数据完全本地控制

### 1.2 技术栈

| 组件 | 技术 | 说明 |
|------|------|------|
| **Skill 框架** | Rust async trait | 统一接口 |
| **事件总线** | Matrix Protocol | 去中心化消息传递 |
| **存储** | SQLite 多库分离 | 每个 Skill 独立数据库 |
| **WASM 运行时** | wasmtime | 动态加载 Skills |
| **配置** | TOML + JSON | Manifest + 运行时配置 |

---

## 2. Skill 接口定义

### 2.1 核心 Trait

**文件**: `cis-core/src/skill/mod.rs`

```rust
#[async_trait]
pub trait Skill: Send + Sync {
    // ========== 基本信息 ==========

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

    // ========== Matrix 集成 ==========

    /// Skill 对应的 Matrix Room ID
    /// 默认格式: !{skill_name}:cis.local
    fn room_id(&self) -> Option<String> {
        Some(format!("!{}:cis.local", self.name()))
    }

    /// 是否联邦同步（默认 false）
    fn federate(&self) -> bool {
        false
    }

    // ========== 生命周期 ==========

    /// 初始化（加载时调用）
    async fn init(&mut self, config: SkillConfig) -> Result<()>;

    /// 初始化 Matrix Room（创建/加入 Room，注册事件处理器）
    async fn init_room(&self, nucleus: Arc<MatrixNucleus>) -> Result<()>;

    /// 处理 Matrix Event
    async fn on_matrix_event(&self, event: MatrixEvent) -> Result<()>;

    /// 处理 CIS 事件
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()>;

    /// 关闭（卸载时调用）
    async fn shutdown(&self) -> Result<()>;
}
```

### 2.2 SkillContext 接口

Skill 通过 SkillContext 访问系统资源：

```rust
pub trait SkillContext: Send + Sync {
    // ========== 日志 ==========
    fn log_info(&self, message: &str);
    fn log_debug(&self, message: &str);
    fn log_warn(&self, message: &str);
    fn log_error(&self, message: &str);

    // ========== 记忆系统 ==========
    fn memory_get(&self, key: &str) -> Option<Vec<u8>>;
    fn memory_set(&self, key: &str, value: &[u8]) -> Result<()>;
    fn memory_delete(&self, key: &str) -> Result<()>;

    // ========== 配置 ==========
    fn config(&self) -> &SkillConfig;
}
```

### 2.3 事件类型

```rust
pub enum Event {
    /// 初始化
    Init { config: serde_json::Value },

    /// 关闭
    Shutdown,

    /// 定时触发
    Tick,

    /// 记忆变更
    MemoryChange {
        key: String,
        value: Vec<u8>,
        operation: MemoryOp,
    },

    /// 自定义事件
    Custom {
        name: String,
        data: serde_json::Value,
    },

    /// Agent 调用
    AgentCall {
        prompt: String,
        callback: String,
    },
}
```

---

## 3. Skill 生命周期管理

### 3.1 状态机

```
Installed → Registered → Loaded → Active → Unloading → Unloaded → Removed
                 ↑_________|___________|       |
                          Pause      Resume     |
                                       ↑_________|
```

**状态说明**:

| 状态 | 描述 | 可转换状态 |
|------|------|-----------|
| **Installed** | Skill 文件已安装 | Registered, Removed |
| **Registered** | 已注册到系统 | Loaded, Unregistered |
| **Loaded** | 已加载到内存，分配数据库 | Active, Paused, Unloading |
| **Active** | 正在运行，处理事件 | Paused, Unloading |
| **Paused** | 已暂停 | Active, Unloading |
| **Unloading** | 正在卸载中 | Unloaded, Loaded (失败时) |
| **Unloaded** | 已卸载，释放资源 | Loaded, Removed |
| **Removed** | 已从系统移除 | - |

### 3.2 核心操作

**文件**: `cis-core/src/skill/manager.rs`

```rust
pub struct SkillManager {
    db_manager: Arc<DbManager>,
    registry: Arc<Mutex<SkillRegistry>>,
    active_skills: Arc<Mutex<HashMap<String, ActiveSkill>>>,
    #[cfg(feature = "wasm")]
    wasm_runtime: Arc<Mutex<WasmRuntime>>,
}

impl SkillManager {
    /// 加载 Skill
    pub async fn load(&self, name: &str, options: LoadOptions) -> Result<()> {
        // 1. 检查状态
        let info = self.registry.get(name)?;
        if !info.runtime.state.can_load() {
            return Err(SkillError::InvalidState(info.runtime.state));
        }

        // 2. 加载数据库
        let skill_db = self.db_manager.attach_skill_db(name)?;

        // 3. 创建数据目录
        let skill_data_dir = Paths::skill_data_dir(name);
        std::fs::create_dir_all(&skill_data_dir)?;

        // 4. 加载 Skill 代码
        let skill = self.load_skill_code(name, &info)?;

        // 5. 初始化 Skill
        let config = options.config.unwrap_or_default();
        skill.init(config.clone()).await?;

        // 6. 创建 ActiveSkill 记录
        let active_skill = ActiveSkill {
            info: info.clone(),
            db: skill_db,
            config,
            skill: Arc::new(skill),
        };

        // 7. 添加到活跃列表
        let mut skills = self.active_skills.lock().await;
        skills.insert(name.to_string(), active_skill);

        // 8. 更新状态
        self.registry.update_state(name, SkillState::Loaded).await?;

        Ok(())
    }

    /// 卸载 Skill
    pub async fn unload(&self, name: &str) -> Result<()> {
        // 1. 检查状态
        if !self.is_loaded(name)? {
            return Ok(()); // 已卸载
        }

        let mut skills = self.active_skills.lock().await;
        if let Some(mut active_skill) = skills.remove(name) {
            // 2. 关闭 Skill
            active_skill.skill.shutdown().await?;

            // 3. 关闭数据库（执行 checkpoint）
            active_skill.db.close()?;

            // 4. DETACH 数据库
            self.db_manager.detach_skill_db(name)?;
        }

        // 5. 更新状态
        self.registry.update_state(name, SkillState::Unloaded).await?;

        Ok(())
    }

    /// 激活 Skill（启动事件循环）
    pub async fn activate(&self, name: &str) -> Result<()> {
        if !self.is_loaded(name)? {
            return Err(SkillError::NotLoaded(name.to_string()));
        }

        let skills = self.active_skills.lock().await;
        if let Some(active_skill) = skills.get(name) {
            // 启动 Skill 的事件循环
            self.registry.update_state(name, SkillState::Active).await?;
        }

        Ok(())
    }

    /// 停用 Skill
    pub async fn deactivate(&self, name: &str) -> Result<()> {
        if !self.is_active(name)? {
            return Ok(());
        }

        // 停止事件循环
        self.registry.update_state(name, SkillState::Paused).await?;

        Ok(())
    }
}
```

### 3.3 热插拔实现

```rust
// 加载 Skill 的完整流程
pub async fn hotplug_load(&self, name: &str, manifest_path: &Path) -> Result<()> {
    // 1. 读取 Manifest
    let manifest = std::fs::read_to_string(manifest_path)?;
    let meta: SkillMeta = toml::from_str(&manifest)?;

    // 2. 注册 Skill
    self.register(meta)?;

    // 3. 加载 Skill
    self.load(name, LoadOptions::default()).await?;

    // 4. 激活 Skill
    self.activate(name).await?;

    tracing::info!("✅ Skill '{}' loaded and activated", name);

    Ok(())
}

// 卸载 Skill 的完整流程
pub async fn hotplug_unload(&self, name: &str) -> Result<()> {
    // 1. 停用 Skill
    self.deactivate(name).await?;

    // 2. 卸载 Skill
    self.unload(name).await?;

    // 3. 注销 Skill
    self.unregister(name)?;

    tracing::info!("✅ Skill '{}' unloaded and unregistered", name);

    Ok(())
}
```

---

## 4. Matrix 集成

### 4.1 Skill = Matrix Room 视图

每个 Skill 对应一个 Matrix Room，作为其消息总线和状态同步通道：

```rust
impl Skill for MySkill {
    fn room_id(&self) -> Option<String> {
        // 格式: !{skill_name}:cis.local
        Some(format!("!{}:cis.local", self.name()))
    }

    fn federate(&self) -> bool {
        // 控制是否联邦同步
        // 私有 Skill: false
        // 公共 Skill: true
        false
    }

    async fn init_room(&self, nucleus: Arc<MatrixNucleus>) -> Result<()> {
        if let Some(room_id_str) = self.room_id() {
            let room_id = RoomId::parse(&room_id_str)?;

            // 创建 Room
            let opts = RoomOptions::new(self.name().to_string())
                .with_federate(self.federate());

            nucleus.create_room(opts).await?;

            // 注册事件处理器
            nucleus.register_handler("*", {
                let skill = self.clone();
                move |event| {
                    // 处理所有事件类型
                    skill.on_matrix_event(event)
                }
            }).await;

            tracing::info!("✅ Skill '{}' initialized Room: {}", self.name(), room_id);
        }
        Ok(())
    }
}
```

### 4.2 事件处理流程

**完整的事件流路径**：

```
Matrix Client (Element Web)
    ↓ 1. 发送消息
Matrix Server (port 7676)
    ↓ 2. 路由到 Bridge
MatrixBridge::on_matrix_message()
    ↓ 3. 解析命令
parse_skill_command()
    ↓ 4. 构造 SkillTask
SkillTask { skill, action, params }
    ↓ 5. 调用 Skill
invoke_skill()
    ↓ 6. SkillManager 查找/加载/激活
SkillManager.ensure_active()
    ↓ 7. 执行 Skill
skill.handle_event()
    ↓ 8. 格式化结果
format_result()
    ↓ 9. 发送回 Matrix
send_to_room()
    ↓ 10. 更新 Room 状态
MatrixNucleus.send_event()
```

### 4.3 命令解析

**文件**: `cis-core/src/matrix/bridge.rs`

```rust
impl MatrixBridge {
    /// 解析 Skill 命令
    ///
    /// 格式: !skill <name> [action] [key=value ...]
    ///
    /// 示例:
    /// - !skill nav target=sofa speed=fast
    /// - !skill git pull origin=origin branch=main
    pub fn parse_skill_command(&self, content: &str) -> Option<SkillTask> {
        let content = content.trim();

        // 检查是否为 Skill 命令
        if !content.starts_with("!skill ") {
            return None;
        }

        let parts: Vec<&str> = content.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        // 解析 Skill 名称
        let skill = parts[1].to_string();

        // 解析动作（默认 "default"）
        let action = if parts.len() > 2 && !parts[2].contains('=') {
            parts[2].to_string()
        } else {
            "default".to_string()
        };

        // 解析参数
        let mut params = HashMap::new();
        for part in parts.iter().skip(2) {
            if let Some((key, value)) = part.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }

        Some(SkillTask {
            skill,
            action,
            params,
            raw: content.to_string(),
        })
    }

    /// 调用 Skill
    pub async fn invoke_skill(&self, task: SkillTask) -> Result<SkillResult> {
        // 1. 检查 Skill 是否存在
        if !self.skill_manager.is_registered(&task.skill)? {
            return Ok(SkillResult::error(format!("Skill '{}' not found", task.skill)));
        }

        // 2. 确保 Skill 已加载
        if !self.skill_manager.is_loaded(&task.skill)? {
            self.skill_manager.load(&task.skill, LoadOptions::default()).await?;
        }

        // 3. 确保 Skill 已激活
        if !self.skill_manager.is_active(&task.skill)? {
            self.skill_manager.activate(&task.skill).await?;
        }

        // 4. 构造事件
        let event = Event::Custom {
            name: task.action.clone(),
            data: serde_json::json!({
                "params": task.params,
            }),
        };

        // 5. 获取 Skill 并执行
        let skill = self.skill_manager.get_active_skill(&task.skill)?;
        let ctx = self.create_context();

        // 6. 执行 Skill
        match skill.handle_event(&ctx, event).await {
            Ok(()) => Ok(SkillResult::success("Command executed")),
            Err(e) => Ok(SkillResult::error(e.to_string())),
        }
    }
}
```

---

## 5. Skill 配置系统

### 5.1 skill.toml Manifest

标准 Skill 配置文件：

```toml
[skill]
name = "my-skill"
version = "1.0.0"
description = "My awesome skill"
author = "Your Name"
type = "native"           # native 或 wasm
entry = "lib.rs"           # 入口文件

[permissions]
memory_read = true         # 读取记忆
memory_write = true        # 写入记忆
ai_call = true            # 调用 AI
network = true            # 网络访问
p2p = true               # P2P 通信

[exports]
functions = [
    { name = "my_function", description = "My function" }
]

commands = [
    {
        name = "cmd",
        description = "Execute command",
        args = [
            {
                name = "arg",
                description = "Argument",
                type = "string",
                required = true
            }
        ]
    }
]

[config.schema]
api_key = { description = "API key", type = "string", required = true }
webhook_url = { description = "Webhook URL", type = "string", required = false }

[config.defaults]
api_key = ""
webhook_url = "http://localhost:8080/webhook"
```

### 5.2 运行时配置

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillConfig {
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

    /// 合并配置
    pub fn merge(&mut self, other: SkillConfig) {
        for (key, value) in other.values {
            self.values.insert(key, value);
        }
    }
}

impl Default for SkillConfig {
    fn default() -> Self {
        Self {
            values: HashMap::new(),
        }
    }
}
```

---

## 6. Skill 实现示例

### 6.1 最小 Skill 示例

```rust
use cis_core::skill::{Skill, SkillContext, SkillConfig, Event};
use cis_core::error::Result;
use async_trait::async_trait;

pub struct MinimalSkill;

#[async_trait]
impl Skill for MinimalSkill {
    fn name(&self) -> &str {
        "minimal"
    }

    fn description(&self) -> &str {
        "A minimal skill example"
    }

    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        tracing::info!("Initializing minimal skill");
        Ok(())
    }

    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Init { config } => {
                ctx.log_info("Skill initialized");
            }
            Event::Tick => {
                ctx.log_debug("Tick event received");
            }
            Event::Custom { name, data } => {
                ctx.log_info(&format!("Custom event: {} - {}", name, data));
            }
            _ => {}
        }
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down minimal skill");
        Ok(())
    }
}
```

---

## 7. WASM Skill 支持

### 7.1 WASM Skill Manifest

```toml
[skill]
name = "wasm-example"
version = "1.0.0"
type = "wasm"
entry = "skill.wasm"

[permissions]
memory_read = true
memory_write = false    # 只读访问
ai_call = false         # 不允许调用 AI
network = false         # 不允许网络访问
```

### 7.2 加载 WASM Skill

```rust
impl SkillManager {
    #[cfg(feature = "wasm")]
    pub async fn load_wasm(
        &self,
        name: &str,
        wasm_bytes: &[u8],
        options: LoadOptions,
    ) -> Result<()> {
        // 1. 创建 WASM 运行时
        let mut wasm_skill = WasmSkillBuilder::new()
            .name(name)
            .version("1.0.0")
            .wasm_bytes(wasm_bytes.to_vec())
            .memory_service(self.create_memory_service())
            .build()?;

        // 2. 实例化 WASM 模块
        wasm_skill.instantiate()?;

        // 3. 初始化 Skill
        wasm_skill.call_init(&options.config)?;

        // 4. 创建 ActiveSkill 包装器
        let active_skill = ActiveSkill {
            info: /* ... */,
            db: self.db_manager.attach_skill_db(name)?,
            config: options.config,
            skill: Arc::new(wasm_skill),
        };

        // 5. 注册为活跃 Skill
        self.active_skills.lock().await
            .insert(name.to_string(), active_skill);

        Ok(())
    }
}
```

---

## 8. 事件联邦同步

### 8.1 跨节点事件广播

当 Skill 执行产生需要联邦同步的事件时：

```rust
impl MySkill {
    pub async fn do_something(&self) -> Result<()> {
        // 1. 执行操作
        let result = self.perform_operation().await?;

        // 2. 发送 Matrix 事件到本地 Room
        let event = MatrixEvent::new(
            self.room_id().unwrap().into(),
            EventId::new(format!("${}", Uuid::new_v4())),
            UserId::new("@me:cis.local"),
            "io.cis.my_skill.result",
            serde_json::to_value(&result)?,
        );

        if let Some(nucleus) = &self.nucleus {
            // 3. 发送到 Matrix Room
            nucleus.send_event(&event.room_id, event.clone()).await?;

            // 4. 如果 federate=true，广播到联邦节点
            if self.federate() {
                let broadcaster = EventBroadcaster::new(
                    /* ... */
                );
                broadcaster.broadcast_event(&event.room_id, &event).await?;
            }
        }

        Ok(())
    }
}
```

### 8.2 处理联邦事件

```rust
impl MySkill {
    pub async fn on_matrix_event(&self, event: MatrixEvent) -> Result<()> {
        match event.event_type.as_str() {
            "io.cis.other_skill.result" => {
                // 处理来自其他节点的 Skill 事件
                let result: OtherSkillResult = serde_json::from_value(event.content.clone())?;
                self.handle_remote_result(result).await?;
            }
            _ => {}
        }
        Ok(())
    }
}
```

---

## 9. 实现检查清单

### 9.1 核心 Skill 框架

- [x] Skill trait 定义
- [x] SkillManager 实现
- [x] SkillContext 接口
- [x] 生命周期状态机
- [x] 热插拔支持
- [ ] WASM 运行时集成
- [ ] 权限沙箱实现

### 9.2 Matrix 集成

- [x] MatrixNucleus 实现
- [x] MatrixBridge 命令解析
- [x] Room 创建和管理
- [x] 事件处理器注册
- [ ] EventBroadcaster 联邦广播
- [ ] Skill 事件类型定义完善

### 9.3 配置系统

- [x] skill.toml Manifest 标准
- [x] SkillConfig 运行时配置
- [x] 配置加载和验证
- [ ] 环境变量集成
- [ ] 配置热更新

### 9.4 存储集成

- [x] 多库分离架构
- [x] ATTACH/DETACH 支持
- [ ] Skill 数据库模板
- [ ] 跨库查询示例

---

## 10. 开发指南

### 10.1 创建新 Skill

**步骤 1**: 创建目录结构

```bash
mkdir -p skills/my-skill/src
cd skills/my-skill
```

**步骤 2**: 创建 `Cargo.toml`

```toml
[package]
name = "my-skill"
version = "0.1.0"
edition = "2021"

[dependencies]
cis-core = { path = "../../cis-core" }
cis-skill-sdk = { path = "../../cis-skill-sdk" }
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"
anyhow = "1.0"
```

**步骤 3**: 创建 `src/lib.rs`

```rust
use cis_core::skill::{Skill, SkillContext, SkillConfig, Event};
use cis_core::error::Result;
use async_trait::async_trait;

pub struct MySkill;

#[async_trait]
impl Skill for MySkill {
    fn name(&self) -> &str {
        "my-skill"
    }

    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        tracing::info!("Initializing MySkill");
        Ok(())
    }

    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => {
                ctx.log_info(&format!("Received event: {}", name));
            }
            _ => {}
        }
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
```

**步骤 4**: 创建 `skill.toml`

```toml
[skill]
name = "my-skill"
version = "0.1.0"
description = "My awesome skill"
author = "Your Name"
type = "native"
entry = "lib.rs"

[permissions]
memory_read = true
memory_write = true
```

**步骤 5**: 构建和加载

```bash
# 构建
cargo build --release

# 加载 Skill
!skill load my-skill

# 激活 Skill
!skill activate my-skill

# 测试 Skill
!skill my-skill action=test
```

---

## 11. 最佳实践

### 11.1 错误处理

```rust
impl Skill for MySkill {
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        // 使用 ? 传播错误，让框架处理
        let result = self.do_something().await
            .map_err(|e| {
                ctx.log_error(&format!("Operation failed: {}", e));
                CisError::skill(format!("Something failed: {}", e))
            })?;

        Ok(())
    }
}
```

### 11.2 资源清理

```rust
impl Skill for MySkill {
    async fn shutdown(&self) -> Result<()> {
        // 1. 停止后台任务
        if let Some(task) = self.background_task.take() {
            task.abort().await;
        }

        // 2. 关闭连接
        if let Some(conn) = self.connection.take() {
            conn.close().await?;
        }

        // 3. 刷新数据
        self.db.flush().await?;

        tracing::info!("✅ Skill shutdown complete");
        Ok(())
    }
}
```

---

## 12. 故障排查

### 12.1 常见问题

**问题**: Skill 无法加载

```
解决方案:
1. 检查 skill.toml 格式是否正确
2. 验证 entry 文件路径是否存在
3. 查看编译错误日志
```

**问题**: Matrix Room 无法创建

```
解决方案:
1. 检查 MatrixNucleus 是否已启动
2. 验证 room_id 格式是否正确
3. 查看数据库连接状态
```

**问题**: 事件未收到

```
解决方案:
1. 确认事件处理器已注册
2. 检查事件类型是否匹配
3. 验证 federate 设置
```

### 12.2 调试命令

```bash
# 列出所有 Skills
!skill list

# 查看 Skill 状态
!skill status my-skill

# 重新加载 Skill
!skill reload my-skill

# 查看 Skill 日志
!skill logs my-skill

# 测试 Skill
!skill test my-skill action=test
```

---

## 附录

### A. 核心优势

- ✅ **热插拔**: 运行时加载/卸载，无需重启
- ✅ **Matrix 集成**: 每个 Skill 一个 Room，天然支持联邦
- ✅ **本地主权**: Skill 数据库独立，完全本地控制
- ✅ **类型安全**: Rust trait 编译时检查
- ✅ **沙箱隔离**: WASM Skills 安全隔离

### B. 下一步

1. **完善 WASM 支持**: 完整实现 WASM 运行时集成
2. **权限系统**: 实现细粒度权限控制
3. **监控工具**: Skill 性能监控和调试工具
4. **Marketplace**: Skill 分发市场
5. **测试框架**: Skill 单元和集成测试

### C. 参考文档

- [Skill trait 定义](cis-core/src/skill/mod.rs)
- [SkillManager 实现](cis-core/src/skill/manager.rs)
- [MatrixNucleus 文档](cis-core/src/matrix/nucleus.rs)
- [Feishu IM Skill 示例](skills/cis-feishu-im/src/lib.rs)

---

**文档版本**: 1.0
**最后更新**: 2026-02-02
