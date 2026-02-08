# CIS v1.0.0 变更日志

**版本**: 1.0.0 (Foundation)  
**日期**: 2026-02-08  

---

## 🎉 首个正式版本

CIS v1.0.0 是项目的首个正式版本，标志着核心架构的完成。

---

## ✨ 新增功能

### 核心引擎

- **DAG 四级决策机制**
  - 预判阶段 (Pre-execution)
  - 执行阶段 (Execution)
  - 验证阶段 (Validation)
  - 结算阶段 (Settlement)
  - 债务机制和自动回滚

- **异步执行引擎**
  - 非阻塞任务调度
  - 优先级队列
  - 资源管理

### Matrix 联邦

- **Matrix Client-Server API** (端口 6767)
  - 用户注册/登录
  - Room 创建/加入
  - 消息发送/接收
  - 同步 API

- **Matrix Federation** (端口 7676)
  - 节点间通信
  - Room 状态同步
  - 事件广播

- **WebSocket 联邦** (端口 6768)
  - 实时事件推送
  - 持久连接

- **分离存储设计**
  - `matrix-social.db` - 用户数据
  - `matrix-events.db` - 协议事件

### 安全加密

- **ChaCha20-Poly1305** - 对称加密
- **Ed25519** - 数字签名
- **Noise Protocol** - 密钥交换
- **DID** - 去中心化身份

### 向量智能 (CVI)

- **sqlite-vec** 向量存储
- **语义相似度检索**
- **嵌入服务接口**

### 存储系统

- **多数据库架构**
  - `node.db` - 核心节点数据
  - `memory.db` - 记忆向量数据
  - `federation.db` - 联邦信任网络
  - `matrix-events.db` - Matrix 协议事件
  - `matrix-social.db` - 人类用户数据 (新增)

### Skill 框架

- **Skill SDK**
  - Native 模式
  - WASM 模式 (基础框架)
  - Host API 定义

- **内置 Skills**
  - init-wizard
  - push-client
  - memory-organizer
  - ai-executor
  - dag-executor
  - im (基础框架)
  - matrix-register-skill (新增)

### 用户界面

- **GUI (egui)**
  - 节点管理界面
  - 记忆浏览器
  - GLM 任务面板
  - 演示数据展示

- **CLI (cis-node)**
  - matrix 命令
  - node 命令
  - worker 命令
  - glm 命令

---

## 🔧 改进

### 架构改进

- **端口分工明确化**
  - 6767: 人机交互 (对外暴露)
  - 7676: 节点通信 (集群内部)
  - 6768: WebSocket 联邦

- **数据库分离设计**
  - 人类数据与协议事件分离
  - 支持独立备份
  - Skill 化注册基础

### 性能优化

- SQLite WAL 模式启用
- 异步 I/O 优化
- 向量检索索引

### 开发体验

- 改进的 CLI 输出
- 详细的日志记录
- 开发模式热重载

---

## 🐛 修复

- 端口配置标准化
- 数据库连接池管理
- Matrix 认证流程

---

## ⚠️ 已知问题

### P0 - 阻塞
- SIGBUS 内存错误 (测试失败)
- GUI 使用演示数据

### P1 - 高优先级
- WASM Skill 执行未完成
- IM 集成不完整

### P2 - 中优先级
- P2P 网络部分实现
- 测试覆盖率偏低 (~40%)

---

## 📝 文档

- 580+ 页 API 文档
- 系统架构文档
- 数据库分离方案
- 端口分工说明
- Element 集成指南

---

## 🔒 安全

- 默认启用加密
- 私钥本地存储
- DID 身份验证
- 硬件绑定支持

---

## 📊 统计

| 指标 | 值 |
|------|-----|
| 代码行数 | ~85,913 |
| 文件数量 | 500+ |
| 测试用例 | 200+ |
| 文档页数 | 580+ |
| 编译警告 | 4 |

---

## 🙏 致谢

感谢所有贡献者和社区支持！

---

**完整变更**: 查看 [Git 历史](https://github.com/opencode/CIS/commits/v1.0.0)  
**问题追踪**: [GitHub Issues](https://github.com/opencode/CIS/issues)
