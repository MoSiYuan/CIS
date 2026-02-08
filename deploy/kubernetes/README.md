# CIS v1.1.0 Kubernetes 部署指南

## 快速开始

### 使用 kubectl 直接部署

```bash
# 进入 Kubernetes 配置目录
cd deploy/kubernetes

# 应用所有配置
kubectl apply -f .

# 查看部署状态
kubectl get pods -l app=cis

# 查看日志
kubectl logs -f deployment/cis-coordinator
kubectl logs -f deployment/cis-worker
```

### 使用 Kustomize 部署（推荐）

```bash
# 基础部署
kubectl apply -k .

# 查看部署状态
kubectl get all -l app=cis
```

## 配置说明

### 协调器节点

- **Deployment**: `cis-coordinator`
- **Service**: `cis-coordinator` (ClusterIP) + `cis-coordinator-external` (LoadBalancer)
- **ConfigMap**: `cis-coordinator-config`
- **PVC**: `cis-coordinator-data` (10Gi)

### Worker 节点

- **Deployment**: `cis-worker` (默认 2 副本)
- **Service**: `cis-worker` (Headless) + `cis-worker-lb` (ClusterIP)
- **ConfigMap**: `cis-worker-config`
- **PVC**: `cis-worker-data` (5Gi，每个 Pod 独立)

## 端口说明

| 端口  | 协议   | 用途                          |
|-------|--------|-------------------------------|
| 7676  | TCP    | Federation API                |
| 7677  | UDP    | P2P QUIC 传输                 |
| 6767  | TCP    | Matrix Federation (WebSocket) |

## 环境变量

### v1.1.0 新增环境变量

| 变量名                        | 说明                  | 默认值      |
|-------------------------------|-----------------------|-------------|
| `CIS_VERSION`                 | CIS 版本号            | 1.1.0       |
| `CIS_K8S_ENABLED`             | Kubernetes 模式标识   | true        |
| `CIS_POD_NAME`                | Pod 名称              | 自动注入    |
| `CIS_POD_NAMESPACE`           | Pod 命名空间          | 自动注入    |
| `CIS_NETWORK_MODE`            | 网络模式              | whitelist   |
| `CIS_ENABLE_TLS`              | 启用 TLS              | true        |
| `CIS_AUTO_GENERATE_CERTS`     | 自动生成证书          | true        |
| `CIS_TELEMETRY_ENABLED`       | 启用遥测              | true        |
| `CIS_MAX_MEMORY_MB`           | 最大内存限制 (MB)     | 4096        |
| `CIS_MAX_CONNECTIONS`         | 最大连接数            | 1000        |

## 扩展 Worker

```bash
# 横向扩展 Worker 到 5 个副本
kubectl scale deployment cis-worker --replicas=5
```

## 更新部署

```bash
# 更新镜像版本
kubectl set image deployment/cis-coordinator cis-coordinator=cis:v1.1.1
kubectl set image deployment/cis-worker cis-worker=cis:v1.1.1

# 或使用 kustomize
kubectl apply -k .
```

## 清理资源

```bash
kubectl delete -f .
# 或使用 kustomize
kubectl delete -k .
```
