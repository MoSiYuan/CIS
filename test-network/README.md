# CIS Docker 组网测试

极简 3 节点组网测试环境。

## 快速测试

```bash
# 一键测试（自动构建或启动）
./test.sh

# 或使用构建版本
./test.sh build
```

## 文件说明

```
test-network/
├── docker-compose.yml          # 预编译二进制版本
├── docker-compose.build.yml    # 从源码构建版本
├── Dockerfile.test             # 预编译镜像
├── Dockerfile.build            # 构建镜像
├── agent-pair.sh              # AI Agent 组网脚本 ⭐
├── auto-pair.sh               # 完整组网工具
├── pair.sh                    # 基础组网脚本
├── test.sh                    # 一键测试脚本 ⭐
└── README.md
```

## 使用方式

### 方式1: 一键测试 (推荐)

```bash
./test.sh
```

### 方式2: 分步操作

```bash
# 1. 启动环境
docker-compose up -d

# 2. 等待启动
sleep 10

# 3. 组网测试
./agent-pair.sh mesh

# 4. 查看状态
docker-compose ps
```

### 方式3: 从源码构建

```bash
./test.sh build
```

## 组网脚本

```bash
# 星型组网 (node1 <- node2, node3)
./agent-pair.sh mesh

# 链式组网 (node1 <-> node2 <-> node3)
./agent-pair.sh chain

# 指定配对
./agent-pair.sh node1 node2

# 查看状态
./agent-pair.sh status
```

## 网络拓扑

```
星型 (mesh):
    node1 (coordinator)
    /    \
 node2    node3

链式 (chain):
node1 <-> node2 <-> node3
```

## 常用命令

```bash
# 查看日志
docker-compose logs -f node1

# 进入容器
docker exec -it cis-node1 sh

# 手动组网
docker exec cis-node1 cis-node pair generate
docker exec cis-node2 cis-node pair join <CODE>

# 清理环境
docker-compose down
```

## 环境变量

```bash
export GLM_API_KEY='your-key'
export GLM_MODEL='code-plan-glm4.7'
```
