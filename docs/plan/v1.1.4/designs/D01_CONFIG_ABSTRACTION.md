# D01: 配置抽象设计 (Phase 1)

> 任务: P0-1 架构重构 Phase 1  
> 负责人: 开发 A  
> 工期: Week 1-2 (5天)  
> 状态: 设计中

---

## 目标

消除所有硬编码配置，建立统一的配置中心。

---

## 当前问题

```rust
// ❌ 硬编码端口
let addr = "127.0.0.1:6767";

// ❌ 硬编码域名
let domain = "cis.local";

// ❌ 硬编码路径
let db_path = "/var/lib/cis/data.db";
```

---

## 设计方案

### 配置分层

```
┌─────────────────────────────────────────┐
│         环境变量 (最高优先级)            │
│    CIS_NETWORK_TCP_PORT=6767            │
├─────────────────────────────────────────┤
│         配置文件 (config.toml)           │
│    [network]                            │
│    tcp_port = 6767                      │
├─────────────────────────────────────────┤
│         默认值 (代码中定义)              │
│    impl Default for NetworkConfig {     │
│        fn default() -> Self { ... }     │
│    }                                    │
└─────────────────────────────────────────┘
```

### 配置结构

```rust
// config/mod.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub network: NetworkConfig,
    
    #[serde(default)]
    pub storage: StorageConfig,
    
    #[serde(default)]
    pub security: SecurityConfig,
    
    #[serde(default)]
    pub wasm: WasmConfig,
    
    #[serde(default)]
    pub p2p: P2PConfig,
}

// ===== Network Config =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    #[serde(default = "default_tcp_port")]
    pub tcp_port: u16,
    
    #[serde(default = "default_udp_port")]
    pub udp_port: u16,
    
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    
    #[serde(default)]
    pub tls: TlsConfig,
}

fn default_tcp_port() -> u16 { 6767 }
fn default_udp_port() -> u16 { 7677 }
fn default_bind_address() -> String { "0.0.0.0".to_string() }

// ===== Storage Config =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    
    #[serde(default)]
    pub encryption: EncryptionConfig,
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/var/lib"))
        .join("cis")
}

// ===== Security Config =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub command_whitelist: Vec<String>,
    
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize,
    
    #[serde(default = "default_rate_limit")]
    pub rate_limit: u32,
}

// ===== WASM Config =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    #[serde(default = "default_max_memory")]
    pub max_memory: usize,
    
    #[serde(default = "default_max_execution_time")]
    pub max_execution_time: Duration,
    
    #[serde(default)]
    pub allowed_syscalls: Vec<String>,
}

fn default_max_memory() -> usize { 512 * 1024 * 1024 } // 512MB

// ===== P2P Config =====
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct P2PConfig {
    #[serde(default)]
    pub bootstrap_nodes: Vec<String>,
    
    #[serde(default = "default_discovery_interval")]
    pub discovery_interval: Duration,
    
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: Duration,
}
```

---

## 配置加载流程

```rust
// config/loader.rs

pub struct ConfigLoader {
    config_path: PathBuf,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            config_path: Self::default_config_path(),
        }
    }
    
    /// 加载配置（按优先级合并）
    pub fn load(&self) -> Result<Config> {
        // 1. 从默认值开始
        let mut config = Config::default();
        
        // 2. 合并配置文件
        if self.config_path.exists() {
            let file_config = self.load_from_file()?;
            config = self.merge(config, file_config);
        }
        
        // 3. 合并环境变量
        let env_config = self.load_from_env()?;
        config = self.merge(config, env_config);
        
        // 4. 验证配置
        self.validate(&config)?;
        
        Ok(config)
    }
    
    /// 从文件加载
    fn load_from_file(&self) -> Result<Config> {
        let content = fs::read_to_string(&self.config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
    
    /// 从环境变量加载
    fn load_from_env(&self) -> Result<Config> {
        // 使用 envy crate 或手动解析
        // CIS_NETWORK_TCP_PORT → config.network.tcp_port
        let mut config = Config::default();
        
        if let Ok(port) = env::var("CIS_NETWORK_TCP_PORT") {
            config.network.tcp_port = port.parse()?;
        }
        
        if let Ok(port) = env::var("CIS_NETWORK_UDP_PORT") {
            config.network.udp_port = port.parse()?;
        }
        
        // ... 更多环境变量
        
        Ok(config)
    }
    
    /// 验证配置
    fn validate(&self, config: &Config) -> Result<()> {
        // 验证端口范围
        if config.network.tcp_port < 1024 {
            return Err(Error::validation("tcp_port must be >= 1024"));
        }
        
        // 验证路径存在
        if !config.storage.data_dir.exists() {
            fs::create_dir_all(&config.storage.data_dir)?;
        }
        
        // 更多验证...
        
        Ok(())
    }
}
```

---

## 配置使用示例

```rust
// 修改前 (硬编码)
async fn start_server() -> Result<()> {
    let addr = "0.0.0.0:6767";  // ❌ 硬编码
    let listener = TcpListener::bind(addr).await?;
    // ...
}

// 修改后 (配置注入)
async fn start_server(config: &NetworkConfig) -> Result<()> {
    let addr = format!("{}:{}", config.bind_address, config.tcp_port);  // ✅ 配置驱动
    let listener = TcpListener::bind(&addr).await?;
    // ...
}

// 依赖注入
pub struct Server {
    config: Arc<Config>,
}

impl Server {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }
    
    pub async fn start(&self) -> Result<()> {
        start_server(&self.config.network).await?;
        Ok(())
    }
}
```

---

## 配置文件模板

```toml
# config.toml

[network]
tcp_port = 6767
udp_port = 7677
bind_address = "0.0.0.0"

[network.tls]
enabled = true
cert_path = "/etc/cis/cert.pem"
key_path = "/etc/cis/key.pem"

[storage]
data_dir = "/var/lib/cis"
max_connections = 100

[storage.encryption]
enabled = true
key_derivation = "argon2id"

[security]
max_request_size = 10485760  # 10MB
rate_limit = 100  # requests per minute

[wasm]
max_memory = 536870912  # 512MB
max_execution_time = 30000  # 30 seconds

[p2p]
bootstrap_nodes = [
    "did:cis:abc123@192.168.1.100:7677",
    "did:cis:def456@10.0.0.50:7677",
]
discovery_interval = 60  # seconds
```

---

## 任务清单

- [ ] 创建 `config/` 模块
- [ ] 定义 `Config` 结构体
- [ ] 实现配置加载器
- [ ] 实现环境变量解析
- [ ] 实现配置验证
- [ ] 替换所有硬编码端口
- [ ] 替换所有硬编码路径
- [ ] 替换所有硬编码超时
- [ ] 创建默认配置文件模板
- [ ] 更新文档

---

## 验收标准

```bash
# 测试 1: 默认配置
cis start  # 应使用默认端口 6767

# 测试 2: 配置文件
echo '[network]' > /tmp/cis-config.toml
echo 'tcp_port = 8888' >> /tmp/cis-config.toml
cis start --config /tmp/cis-config.toml  # 应使用端口 8888

# 测试 3: 环境变量
CIS_NETWORK_TCP_PORT=9999 cis start  # 应使用端口 9999

# 测试 4: 优先级
CIS_NETWORK_TCP_PORT=7777 cis start --config /tmp/cis-config.toml
# 环境变量优先级最高，应使用 7777
```

---

## 依赖

- `serde` - 序列化
- `toml` - 配置文件解析
- `dirs` - 默认路径

---

*设计创建日期: 2026-02-10*
