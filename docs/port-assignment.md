# CIS 端口分工说明

## 概述

CIS 使用两个主要端口进行 Matrix 通信，明确区分人机交互和节点间通信：

| 端口 | 用途 | 暴露范围 | 协议 |
|------|------|----------|------|
| **6767** | 人机交互 | 对外暴露 | HTTP (Matrix C-S API) |
| **7676** | 节点间通信 | 集群内部 | HTTP (BMI Federation) |
| **6768** | WebSocket 联邦 | 集群内部 | WebSocket |

## 端口 6767 - 人机交互（对外暴露）

### 用途
- **Matrix 客户端访问**: Element、Cinny 等客户端连接
- **智能体 API 访问**: Bearer Token 鉴权的 API 调用
- **用户注册/登录**: 人类用户的认证流程
- **消息同步**: 客户端的消息接收和发送

### 典型连接
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Element   │────▶│  :6767      │────▶│  CIS Node   │
│   Client    │◀────│  HTTP API   │◀────│  Human API  │
└─────────────┘     └─────────────┘     └─────────────┘
```

### 代码引用
- `MatrixServer` 默认监听端口
- `cis-core/src/matrix/server.rs`: `MATRIX_HUMAN_PORT = 6767`
- Well-known: `{"m.homeserver": {"base_url": "http://localhost:6767"}}`

## 端口 7676 - 节点间通信（集群内部）

### 用途
- **Matrix 房间同步**: 跨节点房间状态同步
- **DAG 分发**: 跨节点 Skill DAG 任务分发
- **Room 通信**: 节点间的消息转发
- **联邦发现**: 节点发现和心跳

### 典型连接
```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  CIS Node A │◀───▶│  :7676      │◀───▶│  CIS Node B │
│  (本地)     │     │  BMI API    │     │  (远程)     │
└─────────────┘     └─────────────┘     └─────────────┘
```

### 代码引用
- `FederationServer` 默认监听端口
- `cis-core/src/matrix/federation/types.rs`: `FEDERATION_PORT = 7676`
- GLM 跨节点 DAG 分发: `format!("http://{}:7676", target_node)`

## 端口 6768 - WebSocket 联邦（集群内部）

### 用途
- **实时事件推送**: 低延迟的事件广播
- **持久连接**: 长连接替代 HTTP 轮询
- **Noise 协议**: 加密通信握手

## 安全建议

### 6767 端口（对外暴露）
- **防火墙**: 建议仅开放给可信网络
- **TLS**: 生产环境应启用 HTTPS
- **Rate Limit**: 实施请求频率限制
- **CORS**: 配置允许的跨域来源

### 7676/6768 端口（集群内部）
- **防火墙**: 应限制在集群内部网络
- **不暴露公网**: 不应直接暴露到互联网
- **VPC/内网**: 建议部署在私有网络中

## 配置示例

### 单机开发
```bash
# 两个端口都绑定 localhost
cis matrix start --port 6767  # 人机交互
# 内部通信自动使用 7676
```

### 生产集群
```bash
# Node 1 (公网可访问)
cis matrix start --port 6767 --bind 0.0.0.0
# 内部通信绑定内网 IP
cis matrix federation --port 7676 --bind 10.0.0.1

# Node 2 (仅内网）
cis matrix start --port 6767 --bind 127.0.0.1
# 内部通信绑定内网 IP
cis matrix federation --port 7676 --bind 10.0.0.2
```

## 默认常量

```rust
// cis-core/src/matrix/server.rs
pub const MATRIX_HUMAN_PORT: u16 = 6767;

// cis-core/src/matrix/federation/types.rs
pub const FEDERATION_PORT: u16 = 7676;
```

## 常见问题

### Q: 为什么需要两个端口？
A: 人机交互和节点间通信有不同的安全要求和流量模式。分离后：
- 可以独立扩展（如内部通信使用更高带宽）
- 可以独立安全配置（如内部通信使用更简单的认证）
- 可以独立监控和限流

### Q: 可以只用一个端口吗？
A: 技术上可以通过路径区分，但强烈建议保持分离以确保：
- 外部攻击不影响内部通信
- 内部故障不影响外部服务
- 更清晰的安全策略

### Q: 端口冲突怎么办？
A: 可以通过配置更改端口：
```rust
MatrixServerBuilder::new()
    .port(8080)  // 更改人机交互端口
    .build()?;

FederationServerBuilder::new()
    .port(9090)  // 更改节点间通信端口
    .build()?;
```
