# CIS v1.1.5 核心任务拆分

> 日期: 2026-02-10  
> 目标: WASM 运行时 + 完整联邦协议 + 安全基线

---

## 任务总览

```
P0: WASM 运行时集成 (必须)
├── W1: WASM 模块加载器
├── W2: Bridge-WASM 集成
├── W3: Host 函数完整实现
└── W4: 测试验证

P1: Matrix 联邦协议完整实现
├── M1: 联邦握手协议
├── M2: 密钥交换 (ED25519 + X25519)
├── M3: 完整 Sync 协议
├── M4: 房间状态管理
└── M5: 端到端加密 (Olm/Megolm)

P2: 安全基线 (行业标准)
├── S1: Agent 命令白名单
├── S2: WASI 沙箱限制
├── S3: 密钥加密存储 (SSH Key)
├── S4: 证书固定 (Certificate Pinning)
├── S5: 速率限制 (Rate Limiting)
└── S6: 输入验证框架
```

---

## P0: WASM 运行时集成

### W1: WASM 模块加载器
**目标**: 实现安全、高效的 WASM 模块加载

**子任务**:
- [ ] W1.1: 模块验证 (wasm-parser)
  - 检查内存限制
  - 禁用危险指令
  - 验证导入/导出
- [ ] W1.2: 模块缓存
  - 编译缓存避免重复编译
  - 缓存失效策略
- [ ] W1.3: 内存管理
  - 固定内存限制 (默认 128MB)
  - 内存增长监控

**验收标准**:
```rust
let module = WasmModule::load(&bytes)
    .with_memory_limit(128 * 1024 * 1024)
    .validate()?;
```

### W2: Bridge-WASM 集成
**目标**: 将 WASM 运行时集成到 Bridge

**子任务**:
- [ ] W2.1: Bridge 初始化 WASM 运行时
  - 创建 WasmRuntime 实例
  - 注册 Host 函数
- [ ] W2.2: Skill 调用 WASM
  - bridge.rs:684 集成点
  - 参数序列化/反序列化
- [ ] W2.3: 错误处理
  - WASM 陷阱捕获
  - 优雅降级

**验收标准**:
```rust
// bridge.rs
let result = wasm_runtime.execute_skill(&skill_name, &params).await?;
```

### W3: Host 函数完整实现
**目标**: 实现所有 Host 函数

**子任务**:
- [ ] W3.1: AI 调用函数
  - `host_ai_chat(prompt)` → 调用 AI Provider
  - `host_ai_embedding(text)` → 调用 Embedding
- [ ] W3.2: 存储函数
  - `host_memory_get(key)`
  - `host_memory_set(key, value)`
  - `host_memory_search(query)`
- [ ] W3.3: 网络函数
  - `host_http_request(method, url, body)`
  - 受限域名白名单

**验收标准**:
所有 Host 函数有完整实现，非 placeholder

### W4: 测试验证
**目标**: 端到端测试 WASM Skill 执行

**子任务**:
- [ ] W4.1: 单元测试
  - 模块加载测试
  - Host 函数测试
- [ ] W4.2: 集成测试
  - Bridge → WASM → Host → AI
- [ ] W4.3: 性能测试
  - 启动时间 < 100ms
  - 内存使用 < 128MB

---

## P1: Matrix 联邦协议完整实现

### M1: 联邦握手协议
**目标**: 实现完整的 Server-Server 握手

**子任务**:
- [ ] M1.1: SRV 记录解析
  - 解析 `_matrix-fed._tcp` SRV
  - 回退到 .well-known
- [ ] M1.2: 版本协商
  - GET /_matrix/federation/v1/version
  - 支持版本 1.11
- [ ] M1.3: 挑战-响应
  - 实现挑战生成
  - 签名验证

**验收标准**:
```rust
let remote_server = federation_client.discover_server("example.com").await?;
let versions = remote_server.get_versions().await?;
assert!(versions.contains("v1.11"));
```

### M2: 密钥交换
**目标**: 实现 ED25519 + X25519 密钥交换

**子任务**:
- [ ] M2.1: 密钥对管理
  - ED25519 签名密钥
  - X25519 加密密钥
- [ ] M2.2: 密钥上传
  - POST /_matrix/key/v2/upload
- [ ] M2.3: 密钥查询
  - GET /_matrix/key/v2/query
  - 缓存和验证

**验收标准**:
```rust
let keys = federation_client.query_keys("@user:example.com").await?;
assert!(keys.verify_signature(&message, &signature)?);
```

### M3: 完整 Sync 协议
**目标**: 实现完整的 /sync 端点

**子任务**:
- [ ] M3.1: 状态增量计算
  - 从 last_sync_token 计算 delta
  - 房间状态变化检测
- [ ] M3.2: 事件排序
  - 拓扑排序保证因果性
  - 处理乱序事件
- [ ] M3.3: 流式响应
  - Server-Sent Events
  - 长轮询支持

**验收标准**:
```rust
let response = client.sync(SyncRequest::new(since)).await?;
assert!(!response.rooms.is_empty());
```

### M4: 房间状态管理
**目标**: 完整的房间状态机

**子任务**:
- [ ] M4.1: 状态解析
  - m.room.create, m.room.join_rules
  - m.room.power_levels
- [ ] M4.2: 成员管理
  - 邀请、加入、离开、踢出
  - 成员状态追踪
- [ ] M4.3: 权限检查
  - 根据 power_levels 验证操作
  - 管理员权限

**验收标准**:
```rust
let can_send = room.can_send_message(&user_id, &event_type)?;
assert!(room.apply_state_event(event).is_ok());
```

### M5: 端到端加密 (Olm/Megolm)
**目标**: 实现 E2EE

**子任务**:
- [ ] M5.1: Olm 账户
  - 双棘轮算法
  - 密钥上传
- [ ] M5.2: Megolm 会话
  - 群组加密
  - 会话共享
- [ ] M5.3: 设备验证
  - Emoji 验证
  - QR 码验证

**验收标准**:
```rust
let encrypted = olm_account.encrypt(&device_key, &plaintext)?;
let decrypted = olm_account.decrypt(&encrypted)?;
```

---

## P2: 安全基线 (行业标准)

### S1: Agent 命令白名单
**目标**: 限制 Agent 可执行的命令

**子任务**:
- [ ] S1.1: 命令分类
  - 安全命令 (只读): ls, cat, grep
  - 危险命令 (需确认): rm, dd, mkfs
  - 禁止命令: sudo, su, chmod 777
- [ ] S1.2: 配置系统
  - YAML 配置文件
  - 支持通配符
- [ ] S1.3: 运行时检查
  - 命令解析
  - 参数验证

**验收标准**:
```yaml
# security/commands.yaml
allowed:
  - ls *
  - cat *
  - grep *
  - cargo *
  
denied:
  - sudo *
  - rm -rf /
  - chmod 777 *
```

### S2: WASI 沙箱限制
**目标**: 限制 WASM 的文件系统访问

**子任务**:
- [ ] S2.1: 能力模型
  - 只读目录列表
  - 可写目录列表
- [ ] S2.2: 路径验证
  - 禁止路径遍历 (../)
  - 符号链接检查
- [ ] S2.3: 资源限制
  - 文件描述符限制
  - 磁盘配额

**验收标准**:
```rust
let sandbox = WasiSandbox::new()
    .with_readonly_paths(&["/data"])
    .with_writable_paths(&["/tmp"])
    .build()?;
```

### S3: 密钥加密存储 (SSH Key)
**目标**: 使用 SSH Key 加密节点私钥

**子任务**:
- [ ] S3.1: SSH Key 加载
  - 支持 OpenSSH 格式
  - 密码保护
- [ ] S3.2: 密钥派生
  - Argon2id + ChaCha20-Poly1305
- [ ] S3.3: 密钥存储
  - 加密后的密钥文件
  - 权限 0600

**验收标准**:
```rust
let encrypted_key = SshKeyEncryption::encrypt_node_key(
    &ssh_key, &node_private_key
)?;
```

### S4: 证书固定
**目标**: 防止中间人攻击

**子任务**:
- [ ] S4.1: 指纹存储
  - 首次连接信任 (TOFU)
  - 持久化存储
- [ ] S4.2: 变更检测
  - 指纹变更警告
  - 用户确认
- [ ] S4.3: 预共享密钥
  - 支持预配置指纹

**验收标准**:
```rust
let pinning = CertificatePinning::new()
    .pin("example.com", &expected_fingerprint)?;
```

### S5: 速率限制
**目标**: 防止 DoS 攻击

**子任务**:
- [ ] S5.1: 令牌桶算法
  - 全局速率限制
  - 每 IP 限制
- [ ] S5.2: 分级限制
  - API 调用: 100/min
  - 认证: 5/min
  - 连接: 10/min
- [ ] S5.3: 惩罚机制
  - 超限封禁
  - 指数退避

**验收标准**:
```rust
let limiter = RateLimiter::new()
    .with_limit("api", 100, Duration::from_secs(60))
    .with_limit("auth", 5, Duration::from_secs(60));
```

### S6: 输入验证框架
**目标**: 防止注入攻击

**子任务**:
- [ ] S6.1: 验证规则
  - 长度限制
  - 字符白名单
  - 正则匹配
- [ ] S6.2: 自动验证
  - derive 宏
  - 运行时检查
- [ ] S6.3: 错误处理
  - 详细错误信息
  - 不泄露内部细节

**验收标准**:
```rust
#[derive(Validate)]
struct CreateRoomRequest {
    #[validate(length(min = 1, max = 255))]
    #[validate(regex = "^[a-zA-Z0-9_-]+$")]
    room_id: String,
}
```

---

## 执行计划

### 阶段 1: WASM 运行时 (W1-W4)
**时间**: 3-4 天  
**并行**: W1 + W3 并行，W2 依赖 W1，W4 最后

### 阶段 2: 联邦协议 (M1-M5)
**时间**: 5-7 天  
**并行**: M1-M3 并行，M4-M5 依赖 M1

### 阶段 3: 安全基线 (S1-S6)
**时间**: 4-5 天  
**并行**: S1, S3, S5 并行，S2, S4, S6 依赖 S1

### 总时间: 12-16 天

---

## 依赖关系

```
WASM 运行时:
  W1 (加载器) ─┬─→ W2 (Bridge集成)
               └─→ W3 (Host函数) ─→ W4 (测试)

联邦协议:
  M1 (握手) ─┬─→ M3 (Sync)
             ├─→ M2 (密钥) ─→ M5 (E2EE)
             └─→ M4 (房间)

安全基线:
  S1 (白名单) ─→ S2 (WASI沙箱)
  S3 (SSH Key)
  S4 (证书固定)
  S5 (速率限制)
  S6 (输入验证)
```

---

## 验收清单

- [ ] WASM Skill 可以完整执行 (Bridge → WASM → Host → AI)
- [ ] Matrix 联邦可以与 Synapse 互通
- [ ] 所有安全基线通过渗透测试
- [ ] 代码覆盖率 > 80%
- [ ] 性能测试通过 (延迟 < 100ms, 内存 < 256MB)

---

*任务拆分完成: 2026-02-10*  
*准备开始实现*
