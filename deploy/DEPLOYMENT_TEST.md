# CIS v1.1.0 部署测试验证步骤

## 概述

本文档提供 CIS v1.1.0 版本部署后的完整测试验证步骤，确保 Docker Compose 和 Kubernetes 部署正常工作。

## 前置条件

- Docker >= 20.10
- Docker Compose >= 2.0
- Kubernetes >= 1.24 (如使用 K8s 部署)
- kubectl >= 1.24 (如使用 K8s 部署)

---

## 一、Docker Compose 部署测试

### 1.1 基础部署测试

```bash
# 进入部署目录
cd deploy

# 启动服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 预期输出:
# NAME              COMMAND                  SERVICE       STATUS          PORTS
# cis-coordinator   "cis-node daemon --c…"   coordinator   running (healthy)   0.0.0.0:7676->7676/tcp, 0.0.0.0:7677->7677/udp
# cis-worker        "cis-node daemon --c…"   worker        running (healthy)   0.0.0.0:7678->7677/udp
```

### 1.2 健康检查测试

```bash
# 检查协调器健康状态
curl -f http://localhost:7676/health

# 预期输出:
# {"status":"healthy","version":"1.1.0","timestamp":"2026-02-07T..."}

# 查看协调器日志
docker-compose logs -f coordinator

# 查看 Worker 日志
docker-compose logs -f worker
```

### 1.3 网络连通性测试

```bash
# 进入协调器容器
docker-compose exec coordinator sh

# 检查端口监听
netstat -tunlp | grep cis

# 预期输出包含:
# 0.0.0.0:7676 (Federation API)
# 0.0.0.0:7677 (P2P QUIC)
# 0.0.0.0:6767 (Matrix WebSocket)

# 退出容器
exit
```

### 1.4 节点状态测试

```bash
# 查看协调器状态
docker-compose exec coordinator cis-node status

# 预期输出:
# Node ID: coordinator
# Role: coordinator
# Status: running
# Version: 1.1.0
# Uptime: ...
# Connections: ...

# 查看 Worker 状态
docker-compose exec worker cis-node status

# 预期输出:
# Node ID: worker-1
# Role: worker
# Status: running
# Version: 1.1.0
# Coordinator: connected
```

### 1.5 v1.1.0 新功能测试

```bash
# 测试网络模式配置
docker-compose exec coordinator cis-node network mode

# 预期输出当前网络模式: whitelist

# 测试节点发现
docker-compose exec coordinator cis-node peer list

# 预期输出已连接的 Worker 节点信息

# 测试联邦同步状态
docker-compose exec coordinator cis-node sync status
```

### 1.6 资源限制验证

```bash
# 检查容器资源限制
docker stats --no-stream cis-coordinator cis-worker

# 验证资源限制是否生效:
# - coordinator: 内存限制 4G，CPU 限制 2
# - worker: 内存限制 8G，CPU 限制 4
```

### 1.7 环境变量验证

```bash
# 验证 v1.1.0 新增环境变量
docker-compose exec coordinator env | grep CIS_

# 预期输出包含:
# CIS_VERSION=1.1.0
# CIS_NETWORK_MODE=whitelist
# CIS_P2P_ENABLED=true
# CIS_FEDERATION_ENABLED=true
# CIS_ENABLE_TLS=true
# CIS_TELEMETRY_ENABLED=true
```

### 1.8 清理测试

```bash
# 停止服务
docker-compose down

# 验证容器已停止
docker-compose ps

# 清理数据卷（可选）
docker-compose down -v
```

---

## 二、Kubernetes 部署测试

### 2.1 部署前检查

```bash
# 检查集群状态
kubectl cluster-info

# 检查节点状态
kubectl get nodes

# 预期输出: 所有节点 Ready
```

### 2.2 基础部署测试

```bash
# 进入 Kubernetes 配置目录
cd deploy/kubernetes

# 应用所有配置
kubectl apply -f .

# 或使用 kustomize
kubectl apply -k .

# 查看部署状态
kubectl get deployments -l app=cis

# 预期输出:
# NAME               READY   UP-TO-DATE   AVAILABLE   AGE
# cis-coordinator    1/1     1            1           ...
# cis-worker         2/2     2            2           ...
```

### 2.3 Pod 状态验证

```bash
# 查看 Pod 状态
kubectl get pods -l app=cis

# 预期输出:
# NAME                               READY   STATUS    RESTARTS   AGE
# cis-coordinator-xxx                1/1     Running   0          ...
# cis-worker-xxx                     1/1     Running   0          ...
# cis-worker-yyy                     1/1     Running   0          ...

# 查看 Pod 详情
kubectl describe pod -l component=coordinator

# 验证事件中没有错误
```

### 2.4 Service 验证

```bash
# 查看 Service
kubectl get svc -l app=cis

# 预期输出:
# NAME                       TYPE           CLUSTER-IP      EXTERNAL-IP   PORT(S)
# cis-coordinator            ClusterIP      ...             <none>        7676/TCP,7677/UDP,6767/TCP
# cis-coordinator-external   LoadBalancer   ...             <pending>     7676/TCP,7677/UDP,6767/TCP
# cis-worker                 ClusterIP      None            <none>        7677/UDP,6767/TCP
# cis-worker-lb              ClusterIP      ...             <none>        7677/UDP,6767/TCP
```

### 2.5 ConfigMap 验证

```bash
# 查看 ConfigMap
kubectl get configmap -l app=cis

# 验证配置内容
kubectl get configmap cis-coordinator-config -o yaml | grep -A 5 "CIS_VERSION"

# 预期输出包含 v1.1.0 配置项
```

### 2.6 持久化存储验证

```bash
# 查看 PVC
kubectl get pvc -l app=cis

# 预期输出:
# NAME                   STATUS   VOLUME                                     CAPACITY   ACCESS MODES
# cis-coordinator-data   Bound    pvc-xxx                                    10Gi       RWO
# cis-worker-data        Bound    pvc-yyy                                    5Gi        RWO
```

### 2.7 健康检查和探针验证

```bash
# 查看 Pod 的就绪状态
kubectl get pods -l app=cis -o wide

# 查看探针状态
kubectl describe pod -l component=coordinator | grep -A 10 "Conditions"

# 预期输出:
# Type              Status
# Initialized       True
# Ready             True
# ContainersReady   True
# PodScheduled      True
```

### 2.8 日志检查

```bash
# 查看协调器日志
kubectl logs -f deployment/cis-coordinator

# 查看 Worker 日志
kubectl logs -f deployment/cis-worker

# 验证 v1.1.0 版本信息
kubectl logs deployment/cis-coordinator | grep "v1.1.0"
```

### 2.9 RBAC 权限验证

```bash
# 查看 ServiceAccount
kubectl get serviceaccount -l app=cis

# 查看 Role
kubectl get role -l app=cis

# 查看 RoleBinding
kubectl get rolebinding -l app=cis
```

### 2.10 扩缩容测试

```bash
# 扩展 Worker 到 5 个副本
kubectl scale deployment cis-worker --replicas=5

# 验证扩展结果
kubectl get pods -l component=worker

# 缩减回 2 个副本
kubectl scale deployment cis-worker --replicas=2
```

### 2.11 滚动更新测试

```bash
# 模拟镜像更新
kubectl set image deployment/cis-coordinator cis-coordinator=cis:v1.1.0-test

# 监控滚动更新状态
kubectl rollout status deployment/cis-coordinator

# 验证更新历史
kubectl rollout history deployment/cis-coordinator

# 如有问题，回滚到上一版本
kubectl rollout undo deployment/cis-coordinator
```

### 2.12 清理测试

```bash
# 删除所有资源
kubectl delete -f .

# 或使用 kustomize
kubectl delete -k .

# 验证资源已删除
kubectl get all -l app=cis
```

---

## 三、集成测试

### 3.1 多节点通信测试

```bash
# Docker Compose 环境
# 在协调器上查看已连接的 Worker
docker-compose exec coordinator cis-node peer list

# 预期输出显示 worker-1 已连接

# Kubernetes 环境
# 进入协调器 Pod
kubectl exec -it deployment/cis-coordinator -- sh

# 查看节点列表
cis-node peer list

# 退出
exit
```

### 3.2 任务分发测试

```bash
# 提交测试任务
curl -X POST http://localhost:7676/api/v1/tasks \
  -H "Content-Type: application/json" \
  -d '{
    "type": "test",
    "payload": {"message": "Hello CIS v1.1.0"}
  }'

# 验证任务状态
curl http://localhost:7676/api/v1/tasks/<task-id>/status
```

### 3.3 联邦同步测试

```bash
# 在协调器上触发联邦同步
docker-compose exec coordinator cis-node sync trigger

# 或使用 Kubernetes
kubectl exec deployment/cis-coordinator -- cis-node sync trigger

# 查看同步状态
kubectl exec deployment/cis-coordinator -- cis-node sync status
```

---

## 四、故障排查

### 4.1 Pod 无法启动

```bash
# 查看 Pod 事件
kubectl describe pod <pod-name>

# 查看容器日志
kubectl logs <pod-name> --previous
```

### 4.2 网络连接问题

```bash
# 测试端口连通性
kubectl run -it --rm debug --image=curlimages/curl --restart=Never -- \
  curl -v telnet://cis-coordinator:7676

# 检查 Service 端点
kubectl get endpoints cis-coordinator
```

### 4.3 存储问题

```bash
# 检查 PVC 状态
kubectl get pvc

# 检查 PV
kubectl get pv

# 查看存储类
kubectl get storageclass
```

---

## 五、性能测试（可选）

### 5.1 负载测试

```bash
# 使用 ab 或 wrk 进行压力测试
ab -n 10000 -c 100 http://localhost:7676/health

# 监控资源使用
kubectl top pods -l app=cis
```

### 5.2 长时间稳定性测试

```bash
# 运行 24 小时稳定性测试
# 监控指标: 内存使用、CPU 使用、连接数、任务成功率
```

---

## 六、测试检查清单

### Docker Compose

- [ ] 服务正常启动
- [ ] 健康检查通过
- [ ] 端口映射正确
- [ ] 日志无错误
- [ ] 节点状态正常
- [ ] 环境变量正确设置
- [ ] 资源限制生效
- [ ] 数据卷持久化正常

### Kubernetes

- [ ] 所有 Pod 运行正常
- [ ] Service 配置正确
- [ ] ConfigMap 挂载正确
- [ ] PVC 绑定成功
- [ ] RBAC 权限正确
- [ ] 探针工作正常
- [ ] 扩缩容功能正常
- [ ] 滚动更新正常

### 集成测试

- [ ] 多节点通信正常
- [ ] 任务分发正常
- [ ] 联邦同步正常
- [ ] v1.1.0 新功能正常

---

## 七、测试完成确认

测试完成后，请确认:

1. 所有测试步骤均已执行
2. 所有检查清单项均已通过
3. 发现的问题已记录并修复
4. 测试结果已存档

如有问题，请提交到 GitHub Issues: https://github.com/MoSiYuan/CIS/issues
