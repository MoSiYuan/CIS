# CIS Matrix AppService

CIS 的 Matrix 网桥，将 DAG 执行状态广播到 Matrix Room，并支持通过 Matrix 命令控制 CIS。

## 功能

- ✅ **DAG 状态广播** - 实时推送执行状态到 Matrix Room
- ✅ **双向命令** - 通过 `!cis` 命令控制 CIS
- ✅ **Room 自动创建** - DAG 运行时自动创建 Room
- ✅ **E2EE 支持** - 端到端加密（开发中）
- 🔄 **Widget 支持** - DAG 可视化（计划中）

## 快速开始

### 前置要求

- 运行中的 CIS 节点
- Matrix Homeserver (Synapse/Dendrite)
- Rust 1.75+ (从源码构建)

### 安装

#### 方式一：Docker（推荐）

```bash
# 1. 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS/packaging/matrix-appservice

# 2. 配置
cp config.example.yaml config.yaml
# 编辑 config.yaml

# 3. 生成注册文件
docker run --rm -v $(pwd):/data mosiyuan/cis-matrix-bridge \
  --generate-registration > cis-appservice.yaml

# 4. 配置 Homeserver
# 将 cis-appservice.yaml 复制到 Synapse 配置目录
# 并添加到 homeserver.yaml 的 app_service_config_files

# 5. 启动
docker-compose up -d
```

#### 方式二：从源码构建

```bash
# 1. 构建
cargo build --release -p cis-matrix-bridge

# 2. 生成注册文件
./target/release/cis-matrix-bridge --generate-registration > cis-appservice.yaml

# 3. 配置并启动
./target/release/cis-matrix-bridge --config config.yaml
```

## 配置

### config.yaml

```yaml
bridge:
  # Matrix Homeserver
  homeserver_url: http://localhost:8008
  domain: example.com
  
  # CIS 节点
  cis_node_url: http://localhost:7676
  
  # 监听地址
  listen_address: 0.0.0.0:8080
  
  # 数据库
  database:
    path: ./data/matrix-bridge.db
  
  # E2EE（可选）
  encryption:
    enabled: true
    pickle_key: your-secret-key
  
  # Room 管理
  room_management:
    auto_create: true
    name_template: "CIS: {dag_id}"
  
  # 命令配置
  commands:
    prefix: "!cis"
```

### cis-appservice.yaml

由 `--generate-registration` 自动生成，包含：

- `id`: AppService ID
- `hs_token`: Homeserver token
- `as_token`: AppService token
- `namespaces`: 用户/房间命名空间

## 使用方法

### 在 Matrix Room 中使用

```
# 运行 DAG
!cis run my-dag

# 查看 DAG 状态
!cis status my-dag

# 列出所有 DAG
!cis list

# 查看任务日志
!cis logs task-abc123

# 搜索记忆
!cis search "关键词"

# 显示帮助
!cis help
```

### DAG 自动 Room 创建

在 DAG 配置中启用：

```toml
[dag]
name = "my-dag"

[matrix]
enabled = true
auto_create = true
room_name = "My DAG Room"
invite_users = ["@user:example.com"]
```

## 命令参考

| 命令 | 描述 | 示例 |
|------|------|------|
| `!cis run <dag>` | 运行 DAG | `!cis run build` |
| `!cis status [dag]` | 查看状态 | `!cis status build` |
| `!cis logs <task>` | 查看日志 | `!cis logs task-123` |
| `!cis list` | 列出 DAG | `!cis list` |
| `!cis search <query>` | 搜索记忆 | `!cis search config` |
| `!cis help` | 显示帮助 | `!cis help` |

## 架构

```
Matrix Client <-> Homeserver <-> CIS AppService <-> CIS Node
```

## 开发

```bash
# 运行测试
cargo test -p cis-matrix-bridge

# 调试模式
RUST_LOG=debug cargo run -p cis-matrix-bridge

# 格式化代码
cargo fmt -p cis-matrix-bridge

# 检查
 cargo clippy -p cis-matrix-bridge
```

## 故障排除

### Bridge 无法连接 Homeserver

1. 检查 `homeserver_url` 配置
2. 确认 Homeserver 可以访问 Bridge 地址
3. 检查注册文件是否正确加载

### 命令无响应

1. 确认 Bot 已在 Room 中
2. 检查命令前缀配置
3. 查看 Bridge 日志

### E2EE 问题

1. 确保启用了 `encryption.enabled`
2. 确认 `pickle_key` 配置正确
3. 可能需要重新验证设备

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE)
