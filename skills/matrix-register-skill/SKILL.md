# Matrix Register Skill

Matrix 用户注册 Skill，提供灵活的用户注册策略和管理功能。

## 功能特性

- **开放注册**: 无需邀请码，任何人均可注册
- **邀请码注册**: 仅持有有效邀请码的用户可注册
- **用户资料管理**: 支持显示名称、头像、状态消息
- **设备管理**: 多设备支持，设备活跃状态跟踪
- **令牌管理**: 访问令牌生成、验证、吊销
- **保留用户名**: 防止注册敏感用户名

## 使用方式

### 直接使用 Skill API

```rust
use matrix_register_skill::{MatrixRegisterSkill, RegistrationConfig, RegistrationPolicy};

// 创建 Skill（使用默认配置：开放注册）
let skill = MatrixRegisterSkill::open(Path::new("matrix-social.db"))?;

// 或使用仅邀请模式
let skill = MatrixRegisterSkill::open(Path::new("matrix-social.db"))?
    .with_config(RegistrationConfig::invite_only(vec![
        "INVITE001".to_string(),
        "INVITE002".to_string(),
    ]));

// 注册用户
let resp = skill.register_user(RegistrationRequest {
    username: Some("@did:cis:node1:user1".to_string()),
    device_id: Some("DEVICE001".to_string()),
    display_name: Some("Alice".to_string()),
    ..Default::default()
})?;

println!("User registered: {}", resp.user_id);
println!("Access token: {}", resp.access_token);
```

### 通过事件处理器

```rust
use matrix_register_skill::handler::handle_action;

// 处理注册事件
let result = handle_action(&skill, "register", json!({
    "username": "@did:cis:node1:user2",
    "device_id": "DEVICE002",
    "display_name": "Bob"
}))?;

// 检查用户名
let result = handle_action(&skill, "check_username", json!({
    "username": "@did:cis:node1:newuser"
}))?;

// 获取用户信息
let result = handle_action(&skill, "get_user_info", json!({
    "user_id": "@did:cis:node1:user1"
}))?;

// 更新资料
let result = handle_action(&skill, "update_profile", json!({
    "user_id": "@did:cis:node1:user1",
    "display_name": "Alice Updated",
    "status_msg": "Hello world!"
}))?;
```

## 配置选项

```rust
RegistrationConfig {
    // 注册策略
    policy: RegistrationPolicy::Open,  // 或 InviteOnly, Disabled
    
    // 有效邀请码列表
    valid_invite_codes: vec!["CODE1".to_string(), "CODE2".to_string()],
    
    // 保留用户名（无法注册）
    reserved_usernames: vec!["admin".to_string(), "root".to_string()],
    
    // Homeserver 名称
    home_server: "localhost".to_string(),
    
    // Token 过期时间（秒）
    token_expiry_secs: Some(30 * 24 * 60 * 60),  // 30天
    
    // 允许多设备
    allow_multiple_devices: true,
    
    // 每用户最大设备数
    max_devices_per_user: 10,
}
```

## 支持的操作

| 操作 | 描述 |
|------|------|
| `register` | 注册新用户 |
| `check_username` | 检查用户名可用性 |
| `get_user_info` | 获取用户信息 |
| `update_profile` | 更新用户资料 |
| `get_flows` | 获取可用注册流程 |
| `get_devices` | 获取用户设备列表 |
| `delete_device` | 删除设备 |
| `revoke_token` | 吊销访问令牌 |
| `delete_user` | 删除用户 |
| `get_config` | 获取当前配置 |
| `cleanup_tokens` | 清理过期令牌 |

## 数据库分离

此 Skill 使用独立的 `matrix-social.db` 存储用户数据：

- **matrix-social.db**: 用户、设备、令牌、资料
- **matrix-events.db**: 房间、事件、同步状态（由 cis-core 管理）

这种分离允许：
1. 独立备份用户数据而不影响事件日志
2. 卸载人类功能而不影响联邦
3. 灵活扩展注册策略

## 集成到 cis-core

```rust
use cis_core::matrix::{MatrixServer, MatrixServerBuilder};
use cis_core::storage::StoragePaths;

// MatrixServerBuilder 自动使用两个数据库
let server = MatrixServerBuilder::new()
    .port(7676)
    .data_dir(StoragePaths::data_dir())
    .build()?;
```

## 测试

```bash
cargo test -p matrix-register-skill
```

## 架构图

```
┌─────────────────────────────────────────────────────────────┐
│                  Matrix Register Skill                       │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐                                        │
│  │  Registration   │  ← 注册策略配置（开放/邀请/禁用）       │
│  │    Config       │                                        │
│  └────────┬────────┘                                        │
│           │                                                  │
│           ▼                                                  │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │  Event Handler  │  │  MatrixSocial   │                   │
│  │    (register)   │◄─┤     Store       │                   │
│  │ (check_username)│  │ (matrix-social) │                   │
│  │  (get_user_info)│  └────────┬────────┘                   │
│  └─────────────────┘           │                            │
│                                ▼                            │
│                      ┌───────────────────┐                  │
│                      │  matrix-social.db │                  │
│                      │  (users, devices, │                  │
│                      │   tokens, profiles)│                 │
│                      └───────────────────┘                  │
└─────────────────────────────────────────────────────────────┘
```
