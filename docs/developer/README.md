# CIS 开发者文档

欢迎来到 CIS 开发者文档！这里提供架构设计、API 文档和贡献指南。

## 目录

### 架构设计
- [系统架构](./architecture.md) - 整体架构设计
- [模块说明](./modules.md) - 核心模块详解
- [数据流](./data-flow.md) - 数据流转过程
- [网络协议](./network-protocol.md) - P2P 和联邦协议

### API 文档
- [HTTP API](./http-api.md) - RESTful API 参考
- [WebSocket API](./websocket-api.md) - 实时通信 API
- [Rust SDK](./rust-sdk.md) - 开发自定义 Skill
- [CLI 参考](./cli-reference.md) - 命令行接口

### 开发指南
- [环境搭建](./setup.md) - 开发环境配置
- [代码规范](./coding-standards.md) - Rust 编码规范
- [测试指南](./testing.md) - 编写和运行测试
- [调试技巧](./debugging.md) - 调试方法和工具

### 贡献指南
- [如何贡献](./contributing.md) - 贡献流程
- [提交规范](./commit-conventions.md) - Commit 消息规范
- [PR 模板](./pr-template.md) - Pull Request 模板
- [发布流程](./release-process.md) - 版本发布步骤

## 快速开始

### 克隆仓库

```bash
git clone https://github.com/MoSiYuan/CIS.git
cd CIS
```

### 开发环境

#### 使用 Dev Container（推荐）

VS Code 用户可以直接使用 Dev Container：

1. 安装 "Dev Containers" 扩展
2. 按 `F1` -> "Dev Containers: Reopen in Container"
3. 等待环境构建完成

#### 本地开发

```bash
# 安装依赖
# macOS
brew install rust openssl sqlite pkg-config

# Ubuntu/Debian
sudo apt-get install -y rustc libssl-dev libsqlite3-dev pkg-config

# 构建
cargo build

# 运行测试
cargo test

# 启动开发节点
cargo run --bin cis-node -- daemon
```

### 项目结构

```
CIS/
├── cis-core/           # 核心库
│   ├── src/
│   │   ├── did/       # DID 身份系统
│   │   ├── matrix/    # Matrix 协议
│   │   ├── network/   # P2P 网络
│   │   ├── storage/   # 存储层
│   │   └── ...
│   └── Cargo.toml
├── cis-node/           # 节点可执行文件
│   └── src/
│       ├── commands/  # CLI 命令
│       └── main.rs
├── cis-gui/            # GUI 应用
├── skills/             # 内置 Skills
│   ├── init-wizard/
│   ├── dag-executor/
│   └── ...
├── packaging/          # 打包配置
│   ├── homebrew/
│   ├── vscode-cis/
│   └── ...
└── docs/               # 文档
```

## 核心概念

### DID (去中心化身份)

```rust
pub struct Did {
    /// 助记词派生的密钥对
    keypair: Ed25519KeyPair,
    /// 硬件指纹
    hardware_id: String,
}

impl Did {
    /// 生成新 DID
    pub fn generate() -> (Self, Mnemonic) { ... }
    
    /// 从助记词恢复
    pub fn from_mnemonic(mnemonic: &Mnemonic) -> Self { ... }
    
    /// 签名
    pub fn sign(&self, message: &[u8]) -> Signature { ... }
}
```

### DAG (有向无环图)

```rust
pub struct Dag {
    pub id: String,
    pub name: String,
    pub steps: Vec<Step>,
    pub dependencies: Graph<String, ()>,
}

pub struct Step {
    pub name: String,
    pub command: String,
    pub depends_on: Vec<String>,
    pub condition: Option<String>,
}
```

### P2P 网络

```rust
pub struct P2PNetwork {
    /// QUIC 传输
    transport: QuicTransport,
    /// mDNS 发现
    discovery: MdnsDiscovery,
    /// 对等节点管理
    peer_manager: PeerManager,
}
```

## 开发 Skill

### 基本结构

```rust
use cis_skill_sdk::prelude::*;

#[derive(Skill)]
struct MySkill;

#[skill_impl]
impl MySkill {
    #[skill_method]
    async fn execute(&self, input: SkillInput) -> Result<SkillOutput> {
        // Skill 逻辑
        Ok(SkillOutput::success(result))
    }
}
```

### 完整示例

```rust
// skills/hello-world/src/lib.rs
use cis_skill_sdk::prelude::*;

#[derive(Debug, Skill)]
#[skill(name = "hello-world", version = "1.0.0")]
pub struct HelloWorldSkill;

#[skill_impl]
impl HelloWorldSkill {
    #[skill_method(description = "Say hello")]
    pub async fn hello(&self, input: HelloInput) -> Result<HelloOutput> {
        let message = format!("Hello, {}!", input.name);
        
        // 记录到记忆
        self.memory().store(MemoryEntry {
            content: message.clone(),
            tags: vec!["greeting".to_string()],
            ..Default::default()
        }).await?;
        
        Ok(HelloOutput { message })
    }
}

#[derive(SkillInput)]
pub struct HelloInput {
    #[input(required = true)]
    pub name: String,
}

#[derive(SkillOutput)]
pub struct HelloOutput {
    pub message: String,
}
```

### 注册 Skill

在 `Cargo.toml` 中添加：

```toml
[package]
name = "hello-world-skill"
version = "1.0.0"
edition = "2021"

[dependencies]
cis-skill-sdk = { path = "../../cis-skill-sdk" }

[lib]
crate-type = ["cdylib"]
```

## API 示例

### HTTP API

```bash
# 获取节点信息
curl http://localhost:7676/api/v1/node/info

# 运行 DAG
curl -X POST http://localhost:7676/api/v1/dags/my-dag/run \
  -H "Content-Type: application/json" \
  -d '{"arg1": "value1"}'

# 搜索记忆
curl "http://localhost:7676/api/v1/memory/search?q=keyword"
```

### WebSocket API

```javascript
const ws = new WebSocket('ws://localhost:6767/ws');

ws.onopen = () => {
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'dag_events'
  }));
};

ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('DAG Event:', data);
};
```

## 测试

### 单元测试

```bash
# 运行所有测试
cargo test

# 运行特定模块测试
cargo test --package cis-core network::

# 显示输出
cargo test -- --nocapture
```

### 集成测试

```bash
# 启动测试节点
cargo run --bin cis-node -- daemon --config test-config.toml &

# 运行集成测试
cargo test --test cli_integration_test
```

### 基准测试

```bash
cargo bench
```

## 调试

### 日志级别

```bash
# 设置日志级别
RUST_LOG=debug cargo run --bin cis-node -- daemon

# 模块级别
RUST_LOG=cis_core::network=trace cargo run ...
```

### GDB 调试

```bash
# 编译调试版本
cargo build

# 使用 GDB
gdb target/debug/cis-node
(gdb) run daemon
```

### VS Code 调试

```json
// .vscode/launch.json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug CIS Node",
      "cargo": {
        "args": ["build", "--bin=cis-node"]
      },
      "args": ["daemon"]
    }
  ]
}
```

## 性能分析

```bash
# 使用 perf
perf record cargo run --release --bin cis-node -- daemon
perf report

# 使用 flamegraph
cargo flamegraph --bin cis-node -- daemon
```

## 贡献

欢迎贡献！请阅读 [贡献指南](./contributing.md)。

### 提交 Issue

- 使用 Issue 模板
- 提供复现步骤
- 包含环境信息

### 提交 PR

1. Fork 仓库
2. 创建分支 (`git checkout -b feature/xxx`)
3. 提交更改 (`git commit -m 'feat: xxx'`)
4. 推送分支 (`git push origin feature/xxx`)
5. 创建 Pull Request

## 参考

- [Rust 文档](https://doc.rust-lang.org/)
- [Cargo 文档](https://doc.rust-lang.org/cargo/)
- [Matrix 规范](https://spec.matrix.org/)
- [QUIC 协议](https://quicwg.org/)
