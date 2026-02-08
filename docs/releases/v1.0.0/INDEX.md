# CIS v1.0.0 文档索引

**版本**: 1.0.0 Foundation  
**发布日期**: 2026-02-08  
**文档版本**: 1.0.0  

---

## 📑 快速导航

### 入门必读
1. [README.md](README.md) - 版本发布说明 (开始 here)
2. [VERSION.md](VERSION.md) - 详细版本信息
3. [CHANGELOG.md](CHANGELOG.md) - 变更日志

### 核心机制
4. [core/ARCHITECTURE.md](core/ARCHITECTURE.md) - 系统架构
5. [core/STORAGE.md](core/STORAGE.md) - 存储设计 (数据库分离)
6. [core/MATRIX.md](core/MATRIX.md) - Matrix 联邦 (端口分工)
7. [core/SECURITY.md](core/SECURITY.md) - 安全机制

### 归档文档
8. [archives/kimi_agent.md](archives/kimi_agent.md) - 评估报告 (75% 完成度)
9. [archives/COMPLETION_ROADMAP.md](archives/COMPLETION_ROADMAP.md) - 完善路线图
10. [archives/EXECUTION_PLAN.md](archives/EXECUTION_PLAN.md) - 执行计划

---

## 📊 阅读路径

### 如果你是新用户
```
README.md → VERSION.md (已知问题) → core/SECURITY.md
```

### 如果你是开发者
```
CHANGELOG.md → core/ARCHITECTURE.md → core/STORAGE.md
```

### 如果你是贡献者
```
archives/kimi_agent.md → archives/EXECUTION_PLAN.md → 认领任务
```

### 如果你是审计人员
```
core/SECURITY.md → core/ARCHITECTURE.md → core/STORAGE.md
```

---

## 🗂️ 文档地图

```
v1.0.0/
├── 📄 README.md              ⭐ 版本发布说明 (Start Here)
├── 📄 VERSION.md             版本详细信息
├── 📄 CHANGELOG.md           变更日志
├── 📄 INDEX.md               本文档 (导航)
│
├── 📁 core/                  核心机制文档
│   ├── ARCHITECTURE.md       系统架构设计
│   ├── STORAGE.md            存储设计 (Matrix 数据库分离)
│   ├── MATRIX.md             Matrix 联邦 (端口 6767/7676/6768)
│   └── SECURITY.md           安全机制 (ChaCha20/Ed25519/Noise)
│
└── 📁 archives/              归档文档
    ├── kimi_agent.md         评估报告 (75% 完成度)
    ├── COMPLETION_ROADMAP.md 完善路线图 (v1.1.0)
    └── EXECUTION_PLAN.md     执行计划表
```

---

## 🏷️ 文档标签

| 标签 | 含义 | 文档 |
|------|------|------|
| ⭐ | 必读 | README.md |
| 🔴 | 阻塞问题 | VERSION.md#known-issues |
| 🟡 | 高优先级 | archives/EXECUTION_PLAN.md |
| 🟢 | 增值功能 | archives/COMPLETION_ROADMAP.md |

---

## 📈 版本状态

```
完成度: 80% (来自 kimi_agent.md 评估)

核心引擎:   ██████████ 90% ✅
Matrix 联邦: █████████░ 85% ✅
存储系统:   █████████░ 90% ✅
安全机制:   █████████░ 90% ✅

GUI 界面:   █████░░░░░ 50% ⚠️
WASM Skill: ████░░░░░░ 40% ⚠️
P2P 网络:   ██████░░░░ 60% ⚠️
```

---

## 🎯 关键信息

### 端口分工 (重要!)
- **6767**: 人机交互 (对外暴露, Element 客户端)
- **7676**: 节点通信 (集群内部, 节点间同步)
- **6768**: WebSocket 联邦 (内部, 实时推送)

### 数据库分离
- `matrix-social.db` - 用户/设备/令牌 (人类数据)
- `matrix-events.db` - 事件/房间/同步 (协议数据)

### 已知问题 (P0)
1. SIGBUS 内存错误 (测试失败)
2. GUI 使用演示数据

---

## 🔗 外部链接

- **项目主页**: https://cis.dev
- **GitHub**: https://github.com/opencode/CIS
- **Issues**: https://github.com/opencode/CIS/issues
- **讨论区**: https://github.com/opencode/CIS/discussions

---

**文档维护**: CIS Core Team  
**最后更新**: 2026-02-08  
**版本**: 1.0.0
