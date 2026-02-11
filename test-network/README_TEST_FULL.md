# CIS 完整测试环境（带向量模型）

这个配置用于在 Docker 中运行完整的 CIS 测试，包括需要预下载模型的 AI 嵌入测试。

## 文件说明

| 文件 | 用途 |
|------|------|
| `Dockerfile.test-full` | 包含预下载 fastembed 向量模型的测试镜像 |
| `docker-compose.test-full.yml` | 3 节点组网 + 测试运行器配置 |

## 前置要求

1. 本地构建 `cis-node` 二进制：
```bash
cd /path/to/CIS
cargo build --release -p cis-node
```

2. 确保可以访问 HuggingFace 或镜像站点（用于下载向量模型）

## 使用方法

### 1. 构建测试镜像

```bash
cd /path/to/CIS
docker build -f test-network/Dockerfile.test-full -t cis-test-full:latest .
```

> **注意**：模型下载 (~130MB) 可能需要几分钟，取决于网络状况。
> 如果下载失败，镜像仍会构建成功，但模型将在运行时下载。

### 2. 运行完整测试

```bash
cd test-network
docker-compose -f docker-compose.test-full.yml up test-runner
```

### 3. 运行 3 节点组网测试（带模型）

```bash
cd test-network
docker-compose -f docker-compose.test-full.yml up -d node1 node2 node3
```

### 4. 在运行中的容器内执行测试

```bash
# 进入 node1 容器
docker exec -it cis-node1 bash

# 手动运行 AI 嵌入测试（需要模型）
cd /build
cargo test --package cis-core test_embedding -- --ignored
```

## 模型信息

- **模型名称**: `nomic-ai/nomic-embed-text-v1.5`
- **大小**: ~130MB
- **维度**: 768
- **下载源**: 
  - 主站: https://huggingface.co/nomic-ai/nomic-embed-text-v1.5
  - 镜像: https://hf-mirror.com/nomic-ai/nomic-embed-text-v1.5
- **缓存路径**: `/root/.cache/huggingface/`

## 测试覆盖

使用此环境可以运行所有测试，包括：
- ✅ 1107+ 个核心单元测试
- ✅ AI 嵌入测试（需要 ONNX 模型）
- ✅ 3 节点组网集成测试

## 故障排除

### 模型下载失败

如果构建时模型下载失败：
1. 检查网络连接
2. 尝试手动下载模型并放入 `test-network/models/` 目录
3. 修改 Dockerfile 从本地复制模型

### 测试超时

AI 嵌入测试首次运行时需要下载模型，可能需要几分钟。如果超时：
```bash
# 手动下载模型到本地缓存
export HF_HOME=$HOME/.cache/huggingface
cargo test --package cis-core test_embedding -- --ignored

# 然后重新构建镜像（模型已在本地）
docker build -f test-network/Dockerfile.test-full -t cis-test-full:latest .
```
