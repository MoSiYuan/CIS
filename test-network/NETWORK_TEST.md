# CIS Docker 网络测试

在 Docker 环境中运行 CIS 网络测试。

## 快速开始

```bash
cd test-network

# 运行完整的网络测试
./run-network-tests.sh
```

## 手动操作

### 1. 构建镜像

```bash
docker-compose -f docker-compose.test-network.yml build
```

### 2. 启动 3 节点组网

```bash
docker-compose -f docker-compose.test-network.yml up -d node1 node2 node3
```

### 3. 查看节点状态

```bash
docker-compose -f docker-compose.test-network.yml ps
```

### 4. 测试网络连通性

```bash
# 进入测试容器
docker-compose -f docker-compose.test-network.yml --profile test run --rm test-runner

# 或进入特定节点
docker-compose -f docker-compose.test-network.yml exec node1 sh
```

### 5. 停止网络

```bash
docker-compose -f docker-compose.test-network.yml down
```

## 网络拓扑

```
┌─────────────────────────────────────────────────────────────┐
│                      Docker Network                         │
│                      172.30.0.0/16                          │
│                                                             │
│  ┌──────────┐      ┌──────────┐      ┌──────────┐          │
│  │  node1   │◄────►│  node2   │◄────►│  node3   │          │
│  │Coordinator│     │  Worker  │      │  Worker  │          │
│  │172.30.1.11│     │172.30.1.12│     │172.30.1.13│          │
│  └──────────┘      └──────────┘      └──────────┘          │
│        ▲                                                    │
│        │                                                    │
│  ┌──────────┐                                              │
│  │test-runner│                                              │
│  │172.30.1.10│                                              │
│  └──────────┘                                              │
└─────────────────────────────────────────────────────────────┘
```

## 端口映射

| 服务 | 端口 | 说明 |
|------|------|------|
| CIS Core | 7676 | HTTP API |
| CIS P2P | 7677/udp | QUIC + Noise XX |
| Matrix | 6767 | HTTP API + WebSocket |
| Matrix | 6768 | Pairing |

## 包含的工具

- **网络工具**: ping, nc (netcat), dig, iperf3
- **CIS 组件**: cis-node, claude-cli, fastembed
- **系统工具**: curl, sqlite3

## 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `GLM_API_KEY` | GLM API 密钥 | - |
| `GLM_MODEL` | GLM 模型 | code-plan-glm4.7 |
| `ANTHROPIC_API_KEY` | Claude API 密钥 | - |
| `HF_HOME` | 模型缓存路径 | /root/.cache/huggingface |
| `RUST_LOG` | 日志级别 | info |

## 测试用例

网络测试包括：

1. **连通性测试**: ping 所有节点
2. **端口测试**: nc 检查服务端口
3. **DNS 测试**: dig 解析节点名
4. **带宽测试**: iperf3 测量网络性能
5. **CIS 组网**: 节点间配对和通信

## 故障排除

### 端口冲突

如果端口被占用，修改 `docker-compose.test-network.yml` 中的端口映射：

```yaml
ports:
  - "7676:7676"  # 改为 "17676:7676"
```

### 镜像构建失败

```bash
# 清理缓存重新构建
docker-compose -f docker-compose.test-network.yml build --no-cache
```

### 网络隔离问题

确保 Docker 网络创建成功：

```bash
docker network ls | grep cis-net
docker network inspect test-network_cis-net
```

## 高级用法

### 运行 CIS 命令

```bash
# 查看节点状态
docker-compose -f docker-compose.test-network.yml exec node1 cis-node status

# 查看帮助
docker-compose -f docker-compose.test-network.yml exec node1 cis-node --help
```

### 使用 Claude CLI

```bash
docker-compose -f docker-compose.test-network.yml exec node1 claude --version
```

### 使用向量引擎

```bash
docker-compose -f docker-compose.test-network.yml exec node1 python3 << 'EOF'
from fastembed import TextEmbedding
model = TextEmbedding()
embedding = model.embed("Hello World")
print(f"Embedding dimension: {len(embedding[0])}")
EOF
```

## 清理

```bash
# 停止并删除容器
docker-compose -f docker-compose.test-network.yml down

# 删除镜像
docker-compose -f docker-compose.test-network.yml down --rmi all

# 删除卷
docker-compose -f docker-compose.test-network.yml down -v
```
