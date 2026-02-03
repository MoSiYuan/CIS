# 剩余核心功能清单

## 1. 私域/公域记忆系统 (Private/Public Memory)

### 当前状态
- ✅ `MemoryDomain` 枚举已定义 (Private/Public)
- ✅ `memory_index` 表有 `domain` 字段
- ❌ 完整的记忆服务未实现
- ❌ 加密/解密逻辑未实现
- ❌ 访问控制未实现

### 需要实现

```rust
// cis-core/src/memory/mod.rs

pub struct MemoryService {
    core_db: Arc<Mutex<CoreDb>>,
    encryption_key: Option<Vec<u8>>, // 私域记忆加密密钥
}

impl MemoryService {
    /// 存储记忆
    pub fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        match domain {
            MemoryDomain::Private => self.set_private(key, value, category),
            MemoryDomain::Public => self.set_public(key, value, category),
        }
    }
    
    /// 读取记忆
    pub fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    
    /// 搜索记忆
    pub fn search(&self, query: &str, domain: Option<MemoryDomain>) -> Result<Vec<MemoryEntry>>;
    
    /// 同步公域记忆到 P2P 网络
    pub async fn sync_public(&self, peer_id: &str) -> Result<()>;
}
```

### 数据流向

```
┌─────────────────────────────────────────────────────────────┐
│                     记忆写入                                  │
│                                                              │
│  ┌─────────────┐         ┌─────────────┐                    │
│  │   Private   │────────▶│  加密存储    │                    │
│  │   私域记忆   │         │  (本地 only) │                    │
│  └─────────────┘         └─────────────┘                    │
│         │                                                    │
│         ▼                                                    │
│  ┌─────────────┐         ┌─────────────┐                    │
│  │   Public    │────────▶│  明文存储    │────────▶ P2P 同步  │
│  │   公域记忆   │         │  (可共享)    │                    │
│  └─────────────┘         └─────────────┘                    │
└─────────────────────────────────────────────────────────────┘
```

## 2. Skill 标准化接口

### 当前状态
- ✅ Skill trait 定义
- ✅ WASM 导出函数骨架
- ❌ 完整的 WASM Runtime 集成
- ❌ Host API 完整实现
- ❌ Skill Manifest 标准

### 需要实现

#### Skill Manifest (skill.toml)

```toml
[skill]
name = "memory-organizer"
version = "1.0.0"
description = "自动整理和增强记忆"
author = "CIS Team"
type = "wasm"  # native | wasm

[skill.permissions]
memory_read = true
memory_write = true
ai_call = true
network = false

[skill.exports]
functions = [
    "skill_init",
    "skill_on_memory_write",
    "skill_parse_keywords",
]

[skill.schema]
input = "json"
output = "json"
```

#### Host API 完整实现

```rust
// cis-core/src/wasm/host.rs

pub struct WasmHost {
    memory: Arc<MemoryService>,
    ai: Arc<AiService>,
}

impl WasmHost {
    /// host_memory_get - WASM 调用 Host 读取记忆
    pub extern "C" fn host_memory_get(
        &self,
        key_ptr: *const u8,
        key_len: usize,
        out_ptr: *mut u8,
        out_len: usize,
    ) -> i32;
    
    /// host_memory_set - WASM 调用 Host 写入记忆
    pub extern "C" fn host_memory_set(
        &self,
        key_ptr: *const u8,
        key_len: usize,
        value_ptr: *const u8,
        value_len: usize,
    ) -> i32;
    
    /// host_ai_chat - WASM 调用 Host AI
    pub extern "C" fn host_ai_chat(
        &self,
        prompt_ptr: *const u8,
        prompt_len: usize,
        out_ptr: *mut u8,
        out_len: usize,
    ) -> i32;
}
```

## 3. 项目用户引导系统

### 当前状态
- ✅ `init-wizard` Skill 骨架
- ✅ 项目配置结构
- ❌ 完整的引导流程
- ❌ 配置文件生成
- ❌ AI 环境检测与配置

### 需要实现

#### 引导流程

```
┌─────────────────────────────────────────────────────────────┐
│                    CIS 初始化向导                            │
├─────────────────────────────────────────────────────────────┤
│ 1. 环境检查                                                  │
│    ├── 检查 AI Agent 安装 (Claude/Kimi/Aider)               │
│    ├── 检查 Git 配置                                         │
│    └── 检查目录权限                                          │
│                                                              │
│ 2. 配置生成                                                  │
│    ├── 创建 ~/.cis/config.toml                              │
│    ├── 生成节点密钥                                          │
│    └── 选择默认 AI Provider                                  │
│                                                              │
│ 3. 项目初始化 (可选)                                         │
│    ├── 创建 .cis/project.toml                               │
│    ├── 设置项目记忆命名空间                                   │
│    └── 配置本地 Skill                                        │
│                                                              │
│ 4. 验证                                                      │
│    ├── 测试 AI 连接                                          │
│    ├── 测试记忆存储                                          │
│    └── 测试 Skill 加载                                       │
└─────────────────────────────────────────────────────────────┘
```

#### CLI 命令

```bash
# 全局初始化
$ cis init

# 项目初始化
$ cis init --project

# 检查环境
$ cis doctor

# 配置管理
$ cis config get <key>
$ cis config set <key> <value>
```

## 4. 记忆服务实现细节

### 私域记忆 (Private)

```rust
impl MemoryService {
    fn set_private(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 加密数据
        let encrypted = self.encrypt(value)?;
        
        // 2. 存储到 core.db
        self.core_db.set_config(
            &format!("private/memory/{}", key),
            &encrypted,
            true, // encrypted = true
        )?;
        
        // 3. 更新索引
        self.update_index(key, "core", MemoryDomain::Private, category)?;
        
        Ok(())
    }
    
    fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>> {
        // 使用节点密钥加密
        // 实现: chacha20poly1305 或 aes-gcm
        todo!()
    }
}
```

### 公域记忆 (Public)

```rust
impl MemoryService {
    fn set_public(
        &self,
        key: &str,
        value: &[u8],
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 明文存储
        self.core_db.set_config(
            &format!("public/memory/{}", key),
            value,
            false, // encrypted = false
        )?;
        
        // 2. 更新索引
        self.update_index(key, "core", MemoryDomain::Public, category)?;
        
        // 3. 触发同步 (如果是 P2P 节点)
        // self.trigger_sync(key).await?;
        
        Ok(())
    }
}
```

## 5. 依赖关系

```
记忆服务
    ├── 依赖: CoreDb (已完成)
    ├── 依赖: 加密模块 (待实现)
    └── 被依赖: WASM Host API

WASM Runtime
    ├── 依赖: wasmer/wasmtime (待集成)
    ├── 依赖: MemoryService
    └── 被依赖: Skill Manager

项目引导
    ├── 依赖: Agent Provider (已完成)
    ├── 依赖: Config 生成 (待实现)
    └── 被依赖: CLI 工具
```

## 6. 优先级建议

### P0 (最高优先级)
1. **记忆服务完整实现** - 私域/公域读写
2. **Skill Manifest 标准** - 定义标准格式

### P1 (高优先级)
3. **项目引导流程** - 让用户体验完整流程
4. **WASM Host API** - 支持 WASM Skill 调用

### P2 (中优先级)
5. **WASM Runtime 集成** - 完整 WASM 支持
6. **记忆加密** - 私域安全存储
7. **P2P 同步** - 公域记忆同步
