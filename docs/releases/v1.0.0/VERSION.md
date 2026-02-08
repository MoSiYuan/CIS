# CIS v1.0.0 版本详细说明

**文档版本**: 1.0.0  
**更新日期**: 2026-02-08  

---

## 📌 版本标识

| 属性 | 值 |
|------|-----|
| 版本号 | 1.0.0 |
| 版本名称 | Foundation |
| 发布类型 | 正式版 (GA) |
| 支持周期 | 长期支持 (LTS) |
| 兼容性 | 不兼容 0.x 版本 |

### 版本号规则

```
1.0.0
│ │ │
│ │ └── Patch: Bug 修复
│ └──── Minor: 新功能 (向后兼容)
└────── Major: 重大变更 (可能不兼容)
```

---

## 📋 变更日志

### 新增功能

#### 核心引擎
- ✅ DAG 四级决策机制 (预判 → 执行 → 验证 → 结算)
- ✅ 债务机制和自动回滚
- ✅ 异步执行引擎

#### Matrix 联邦
- ✅ Matrix Client-Server API (端口 6767)
- ✅ Matrix Federation API (端口 7676)
- ✅ WebSocket 联邦 (端口 6768)
- ✅ Room 创建和管理
- ✅ 用户注册/登录 API
- ✅ 分离存储 (matrix-social.db + matrix-events.db)

#### 安全加密
- ✅ ChaCha20-Poly1305 对称加密
- ✅ Ed25519 数字签名
- ✅ Noise Protocol 密钥交换
- ✅ DID 去中心化身份

#### 向量智能
- ✅ sqlite-vec 向量存储
- ✅ 语义相似度检索
- ✅ 嵌入服务接口

#### 存储系统
- ✅ 多数据库架构
  - `node.db` - 核心节点数据
  - `memory.db` - 记忆向量数据
  - `federation.db` - 联邦信任网络
  - `matrix-events.db` - Matrix 协议事件
  - `matrix-social.db` - 人类用户数据

### 改进

- 端口分工明确 (6767 人机交互 / 7676 节点通信)
- 数据库分离设计
- CLI 命令重构

### 修复

- 端口配置标准化
- 数据库 WAL 模式启用

---

## 🐛 已知问题

### P0 - 阻塞问题 (影响生产使用)

| # | 问题 | 影响 | 临时解决方案 |
|---|------|------|--------------|
| 1 | SIGBUS 内存错误 | 测试失败 | 使用 `--release` 模式运行 |
| 2 | GUI 演示数据 | 无法生产使用 | 使用 CLI 代替 GUI |

### P1 - 高优先级 (影响体验)

| # | 问题 | 影响 |
|---|------|------|
| 3 | WASM Skill 执行 | 生态无法扩展 |
| 4 | IM 集成不完整 | 协作功能缺失 |

### P2 - 中优先级 (增值功能)

| # | 问题 | 影响 |
|---|------|------|
| 5 | P2P 网络不完整 | 组网受限 |
| 6 | 测试覆盖率偏低 | 质量风险 |

---

## 🗺️ 路线图

### v1.0.1 (2026-02 末)
- 修复 SIGBUS 内存错误
- 提升测试覆盖率到 60%+

### v1.1.0 (2026 Q2)
- GUI 连接真实数据库
- WASM Skill 完整执行
- IM 双向消息同步
- P2P 自动组网

### v1.2.0 (2026 Q3)
- 移动端支持
- 插件市场
- 高级安全特性

### v2.0.0 (2026 Q4)
- 完整去中心化
- 跨链互操作
- AI 自治节点

---

## ⚠️ 不兼容性说明

### 数据库
- v1.0.0 使用新的分离存储设计
- 0.x 版本数据库需要手动迁移

### API
- Matrix Federation API 路径变更
- CLI 命令参数调整

### 配置
- 端口配置方式更新
- 新增 `matrix-social.db` 配置

---

## 🔧 配置变更

### 新增配置项

```toml
# cis-config.toml
[matrix]
human_port = 6767      # 人机交互端口 (对外暴露)
node_port = 7676       # 节点通信端口 (集群内部)

[storage]
node_db = "node.db"
memory_db = "memory.db"
federation_db = "federation.db"
matrix_events_db = "matrix-events.db"
matrix_social_db = "matrix-social.db"  # 新增
```

### 废弃配置项

```toml
# 废弃 - v1.0.0 之前
[matrix]
port = 7676            # 已废弃，改用 human_port/node_port
single_db = true       # 已废弃，改为分离存储
```

---

## 📊 性能基准

### 测试环境
- **CPU**: Apple M2
- **内存**: 16GB
- **存储**: SSD

### 基准结果

| 操作 | 延迟 | 吞吐量 |
|------|------|--------|
| DAG 执行 | < 10ms | 1000+ TPS |
| 向量检索 | < 50ms | 100 QPS |
| Matrix 消息 | < 20ms | 500 MSG/s |
| 节点发现 | < 1s | - |

---

## 🔐 安全通告

### CVE 状态
- 暂无已知 CVE

### 安全建议
1. 使用强密码保护私钥
2. 定期备份 `node.key`
3. 限制 7676/6768 端口仅内部访问
4. 启用防火墙规则

---

## 📞 支持

### 社区
- GitHub Issues: https://github.com/opencode/CIS/issues
- Discussions: https://github.com/opencode/CIS/discussions

### 商业支持
- 邮箱: support@cis.dev
- 企业版: https://cis.dev/enterprise

---

## 📝 附录

### A. 术语表

| 术语 | 说明 |
|------|------|
| DAG | 有向无环图 (Directed Acyclic Graph) |
| BMI | 机器间接口 (Between Machine Interface) |
| DID | 去中心化身份 (Decentralized Identifier) |
| CVI | CIS 向量智能 (CIS Vector Intelligence) |

### B. 参考文档

- [原始评估报告](archives/kimi_agent.md)
- [开发路线图](../ROADMAP_INDEX.md)
- [执行计划](../EXECUTION_PLAN.md)

### C. 版本历史

| 版本 | 日期 | 说明 |
|------|------|------|
| 1.0.0 | 2026-02-08 | 首个正式版 |

---

**文档维护**: CIS Core Team  
**最后更新**: 2026-02-08
