# Matrix 数据库分离架构方案

> ✅ **实施状态**: 已完成（2026-02-07）
> - Phase 1: ✅ 添加存储路径
> - Phase 2: ✅ 创建 MatrixSocialStore
> - Phase 3: ✅ 集成到 MatrixServer
> - Phase 4: ✅ 创建 matrix-register-skill
> - Phase 5: ✅ 迁移注册 API

## 当前问题

目前所有 Matrix 相关表都在单一 SQLite 数据库中 (`MatrixStore`)，导致：
1. **难以扩展**：人类社交功能和协议核心耦合
2. **难以卸载**：Matrix 社交功能无法独立移除
3. **备份不便**：无法单独备份用户数据而不备份事件日志
4. **扩展受限**：无法支持更复杂的注册逻辑（邀请码、付费等）

## 提议架构

将 Matrix 存储分离为三个独立的存储：

```
┌─────────────────────────────────────────────────────────────┐
│                    Matrix Federation Layer                   │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ matrix-events   │  │ matrix-social   │  │ federation  │ │
│  │     .db         │  │     .db         │  │    .db      │ │
│  └────────┬────────┘  └────────┬────────┘  └──────┬──────┘ │
│           │                    │                   │       │
│           ▼                    ▼                   ▼       │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────┐ │
│  │ MatrixProtocol  │  │ MatrixRegister  │  │  Federation │ │
│  │     Store       │  │     Skill       │  │    Core     │ │
│  │                 │  │                 │  │             │ │
│  │ • Events        │  │ • User profiles │  │ • Peers     │ │
│  │ • Room state    │  │ • Devices       │  │ • Trust     │ │
│  │ • Sync tokens   │  │ • Access tokens │  │ • Consensus │ │
│  └─────────────────┘  └─────────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## 数据分类

### 1. matrix-events.db (协议核心存储)

**职责**: Matrix 协议事件存储，与 CIS 核心 Federation 紧密关联

**数据表**:
```sql
-- 核心事件存储（必须保留）
matrix_events          -- Matrix 协议事件
matrix_rooms           -- Room 元数据（公共房间信息）
matrix_room_members    -- Room 成员关系（公开成员列表）

-- 同步相关（可选移动）
matrix_sync_tokens     -- 增量同步令牌

-- CIS 扩展
cis_event_meta         -- CIS 事件元数据
pending_sync           -- 断线同步队列
```

**特点**:
- 与 `federation.db` 紧密关联
- 包含公共/共享数据
- 可以跨节点同步/共享
- 卸载 Matrix = 丢失联邦能力

### 2. matrix-social.db (人类社交存储)  ← 需要新建

**职责**: 人类用户相关的本地社交数据

**数据表**:
```sql
-- 用户身份（从 matrix-events 迁移）
matrix_users           -- 本地用户账户
matrix_profiles        -- 用户详细资料（扩展）
matrix_devices         -- 设备注册
matrix_tokens          -- 访问令牌
matrix_sessions        -- 活跃会话

-- 用户设置（可扩展）
matrix_user_settings   -- 用户偏好设置
matrix_push_rules      -- 推送规则

-- 社交关系（可扩展）
matrix_contacts        -- 联系人列表
matrix_direct_rooms    -- DM Room 映射
```

**特点**:
- 纯本地数据，不涉及联邦
- 可独立备份/恢复
- 可支持复杂注册逻辑（邀请码、付费等）
- 可卸载而不影响联邦功能

### 3. federation.db (已存在)

保持不变，存储邦联相关的信任网络和节点状态。

## 实施方案

### Phase 1: 添加存储路径

```rust
// cis-core/src/storage/paths.rs
impl StoragePaths {
    /// Matrix 事件数据库路径（协议核心）
    pub fn matrix_events_db() -> PathBuf {
        Self::data_dir().join("matrix-events.db")
    }
    
    /// Matrix 社交数据库路径（人类用户数据）
    pub fn matrix_social_db() -> PathBuf {
        Self::data_dir().join("matrix-social.db")
    }
}
```

### Phase 2: 创建 MatrixSocialStore

```rust
// cis-core/src/matrix/store_social.rs
pub struct MatrixSocialStore {
    db: Arc<Mutex<Connection>>,
}

impl MatrixSocialStore {
    pub fn open(path: &str) -> MatrixResult<Self>;
    
    // User management
    pub fn create_user(&self, user_id: &str, profile: UserProfile) -> MatrixResult<()>;
    pub fn get_user(&self, user_id: &str) -> MatrixResult<Option<UserRecord>>;
    pub fn update_profile(&self, user_id: &str, profile: UserProfile) -> MatrixResult<()>;
    pub fn delete_user(&self, user_id: &str) -> MatrixResult<()>;
    
    // Device management
    pub fn register_device(&self, user_id: &str, device_id: &str) -> MatrixResult<()>;
    pub fn get_user_devices(&self, user_id: &str) -> MatrixResult<Vec<DeviceRecord>>;
    
    // Token management
    pub fn create_token(&self, user_id: &str, device_id: Option<&str>) -> MatrixResult<String>;
    pub fn validate_token(&self, token: &str) -> MatrixResult<Option<TokenInfo>>;
    pub fn revoke_token(&self, token: &str) -> MatrixResult<()>;
    
    // Registration policies (extensible)
    pub fn check_registration_allowed(&self, options: &RegistrationOptions) -> MatrixResult<bool>;
}
```

### Phase 3: 重构 MatrixStore

将 `MatrixStore` 拆分为两个存储引用：

```rust
// cis-core/src/matrix/store.rs
pub struct MatrixStore {
    // Protocol data (events, rooms, sync)
    protocol_db: Arc<Mutex<Connection>>,
}

pub struct MatrixStores {
    pub protocol: Arc<MatrixStore>,      // matrix-events.db
    pub social: Arc<MatrixSocialStore>,  // matrix-social.db
}
```

### Phase 4: 创建 MatrixRegisterSkill

```rust
// skills/matrix-register-skill/src/lib.rs
pub struct MatrixRegisterSkill {
    social_store: Arc<MatrixSocialStore>,
    config: RegistrationConfig,
}

#[derive(Default)]
pub struct RegistrationConfig {
    /// 是否需要邀请码
    pub require_invite_code: bool,
    /// 是否开放注册
    pub open_registration: bool,
    /// 用户名保留列表
    pub reserved_usernames: Vec<String>,
    /// 注册回调（可扩展）
    pub on_register: Option<Box<dyn Fn(&UserRecord) + Send + Sync>>,
}

#[async_trait]
impl Skill for MatrixRegisterSkill {
    fn name(&self) -> &str { "matrix-register" }
    
    async fn handle_request(&self, req: Request) -> Result<Response> {
        match req.action.as_str() {
            "register" => self.handle_register(req).await,
            "check_username" => self.check_username(req).await,
            "get_profile" => self.get_profile(req).await,
            "update_profile" => self.update_profile(req).await,
            _ => Err(Error::UnknownAction),
        }
    }
}
```

### Phase 5: 迁移注册 API

将注册逻辑从 `cis-core` 路由迁移到 Skill：

```rust
// Before: cis-core/src/matrix/routes/register.rs
pub async fn register(
    State(store): State<Arc<MatrixStore>>,
    Json(req): Json<RegisterRequest>,
) -> Result<...> {
    store.register_user(...).await  // 直接操作数据库
}

// After: Skill-based registration
pub async fn register(
    State(skill_client): State<Arc<SkillClient>>,
    Json(req): Json<RegisterRequest>,
) -> Result<...> {
    skill_client
        .call("matrix-register", "register", req)
        .await
}
```

## 好处

1. **独立扩展**: 可以为社交层添加复杂功能（邀请系统、付费验证等）而不影响协议层
2. **灵活卸载**: 可以禁用 Matrix 人类功能但保留联邦事件处理
3. **独立备份**: 用户数据 (`matrix-social.db`) 可以独立备份/恢复
4. **Skill 化**: 注册逻辑成为可插拔的 Skill，支持自定义策略
5. **数据分离**: 事件日志（大）与用户数据（小）分离，优化性能

## 兼容性

- 现有 API 保持不变（路由层适配）
- 现有数据可以通过迁移脚本分离
- Federation 功能不受影响

## 下一步行动

1. 同意架构设计
2. 创建 `MatrixSocialStore` 和 `StoragePaths::matrix_social_db()`
3. 迁移 `matrix_users`, `matrix_devices`, `matrix_tokens` 表
4. 创建 `matrix-register-skill` 骨架
5. 迁移注册 API 到 Skill

---

- 在 `StoragePaths` 中添加 `matrix_events_db()` 和 `matrix_social_db()` 路径
- 位置: `cis-core/src/storage/paths.rs`

### Phase 2: MatrixSocialStore ✅
- 创建独立的人类用户数据存储
- 支持用户、设备、令牌、资料管理
- 完整的注册流程 (`register_user_complete`)
- 位置: `cis-core/src/matrix/store_social.rs`

### Phase 3: MatrixServer 集成 ✅
- 修改 `MatrixServer` 同时管理两个存储
- 更新 `MatrixServerBuilder` 支持分离的数据库路径
- 更新所有路由以使用 `AppState`（包含两个存储）
- 迁移注册和登录逻辑到使用 `MatrixSocialStore`

### Phase 4: matrix-register-skill ✅
- 创建新的 Skill 项目
- 支持灵活的注册策略（开放/邀请/禁用）
- 完整的用户生命周期管理
- 事件处理器架构
- 位置: `skills/matrix-register-skill/`

### Phase 5: API 迁移 ✅
- 注册 API 使用 `MatrixSocialStore`
- 登录 API 使用 `MatrixSocialStore`
- 认证中间件使用 `MatrixSocialStore`

## 架构变化

```
之前：
┌──────────────────────────────────┐
│          MatrixStore             │
│    (matrix_users, devices, ...)   │
│    (matrix_events, rooms, ...)    │
└──────────────────────────────────┘

之后：
┌─────────────────────┐  ┌─────────────────────┐
│   MatrixSocialStore │  │    MatrixStore      │
│  (matrix-social.db) │  │  (matrix-events.db) │
├─────────────────────┤  ├─────────────────────┤
│ • matrix_users      │  │ • matrix_events     │
│ • matrix_devices    │  │ • matrix_rooms      │
│ • matrix_tokens     │  │ • matrix_room_members│
│ • matrix_profiles   │  │ • cis_event_meta    │
└─────────────────────┘  └─────────────────────┘
         │                          │
         └──────────┬───────────────┘
                    ▼
         ┌─────────────────────┐
         │  MatrixRegisterSkill │
         │   (可插拔/可扩展)     │
         └─────────────────────┘
```

## 文件变更清单

### 新增文件
- `cis-core/src/matrix/store_social.rs` - 社交数据存储 (636 lines)
- `skills/matrix-register-skill/Cargo.toml` - Skill 配置
- `skills/matrix-register-skill/src/lib.rs` - Skill 主库 (399 lines)
- `skills/matrix-register-skill/src/config.rs` - 注册配置
- `skills/matrix-register-skill/src/types.rs` - 类型定义
- `skills/matrix-register-skill/src/error.rs` - 错误类型
- `skills/matrix-register-skill/src/handler.rs` - 事件处理器
- `skills/matrix-register-skill/SKILL.md` - Skill 文档

### 修改文件
- `Cargo.toml` - 添加 matrix-register-skill 到工作空间
- `cis-core/src/storage/paths.rs` - 添加数据库路径
- `cis-core/src/matrix/mod.rs` - 导出 MatrixSocialStore
- `cis-core/src/matrix/server.rs` - 支持双存储
- `cis-core/src/matrix/routes/mod.rs` - AppState 结构
- `cis-core/src/matrix/routes/register.rs` - 使用 SocialStore
- `cis-core/src/matrix/routes/login.rs` - 使用 SocialStore
- `cis-core/src/matrix/routes/auth.rs` - 使用 SocialStore
- `cis-core/src/matrix/routes/sync.rs` - 使用 AppState
- `cis-core/src/matrix/routes/room.rs` - 使用 AppState
- `cis-node/src/commands/matrix.rs` - 更新 API 调用

## 测试状态

```
✅ MatrixSocialStore: 4/4 测试通过
   - test_user_lifecycle
   - test_device_management
   - test_token_management
   - test_complete_registration

✅ MatrixServer: 2/2 测试通过
   - test_server_creation
   - test_builder_with_data_dir

✅ MatrixRegisterSkill: 5/5 测试通过
   - test_skill_creation
   - test_user_registration
   - test_username_availability
   - test_invite_only_policy
   - test_reserved_usernames
```

## 使用方式

### 启动 Matrix 服务器（自动使用分离存储）

```rust
use cis_core::matrix::MatrixServerBuilder;
use cis_core::storage::StoragePaths;

let server = MatrixServerBuilder::new()
    .port(7676)
    .data_dir(StoragePaths::data_dir())
    .build()?;
```

### 使用 MatrixRegisterSkill

```rust
use matrix_register_skill::{MatrixRegisterSkill, RegistrationConfig};

let skill = MatrixRegisterSkill::open(Path::new("matrix-social.db"))?
    .with_config(RegistrationConfig::invite_only(vec![
        "INVITE001".to_string(),
    ]));

let resp = skill.register_user(RegistrationRequest {
    username: Some("@did:cis:node1:user1".to_string()),
    device_id: Some("DEVICE001".to_string()),
    invite_code: Some("INVITE001".to_string()),
    ..Default::default()
})?;
```

## 数据库文件位置

默认情况下，数据库文件存储在：
- **macOS**: `~/Library/Application Support/CIS/`
- **Linux**: `~/.local/share/cis/` 或 `$XDG_DATA_HOME/cis/`
- **Windows**: `%LOCALAPPDATA%\CIS\`

生成的文件：
- `matrix-events.db` - Matrix 协议事件
- `matrix-social.db` - 人类用户数据
- `federation.db` - 联邦信任网络
- `node.db` - CIS 核心数据

## 下一步建议

1. **数据迁移**: 将现有 `matrix_users`, `matrix_devices`, `matrix_tokens` 从旧数据库迁移到 `matrix-social.db`
2. **Skill 集成**: 将 `MatrixRegisterSkill` 注册到 CIS Skill 管理器
3. **高级策略**: 实现邮箱验证、手机号验证、付费注册等策略
4. **备份工具**: 创建独立备份/恢复 `matrix-social.db` 的工具

## 兼容性说明

- Matrix Client-Server API 保持不变
- 现有的 `MatrixStore` API 保持不变（向后兼容）
- `MatrixServerBuilder` 新增 `data_dir()` 方法用于简化配置
- `MatrixServer::new()` 现在需要 `social_store` 参数
