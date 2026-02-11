# Alpine 测试镜像更新

## 更新内容（2026-02-11）

Dockerfile.test 已从精简版升级为完整版，包含以下组件：

### 1. CIS 内核
- `cis-node` 二进制
- 路径: `/usr/local/bin/cis-node`

### 2. Claude CLI
- 通过 npm 全局安装
- 包名: `@anthropic-ai/claude-cli`

### 3. 向量引擎 (fastembed)
- Python3 + pip
- fastembed 库 (用于 nomic-embed-text-v1.5)
- 模型缓存路径: `/root/.cache/huggingface/`
- 模型大小: ~130MB (首次运行时下载)

## 构建方法

```bash
cd test-network

# 确保 Linux 版本的 cis-node 存在
# 需要交叉编译或从其他来源获取

# 构建镜像
docker-compose build

# 或单独构建
docker build -f Dockerfile.test -t cis-network-test:latest .
```

## 使用方式

```bash
# 启动 3 节点组网
docker-compose up -d

# 进入节点使用 Claude CLI
docker exec -it cis-node1 sh
claude --version

# 使用向量引擎 (Python)
docker exec -it cis-node1 python3
>>> from fastembed import TextEmbedding
>>> model = TextEmbedding()
```

## 镜像信息

| 组件 | 版本 |
|------|------|
| Alpine | 3.19 |
| Python | 3.x |
| Node.js | LTS |
| CIS | v1.1.5 |
