# CIS v1.1.0 - Production Ready 🚀

**发布日期**: 2026-02-08

CIS 首个生产就绪版本，经过完整的安全审计和性能优化。

---

## ✨ 主要新特性

### 四级决策系统
- **Mechanical**: 全自动执行
- **Recommended**: 倒计时自动降级（可配置超时）
- **Confirmed**: 用户确认后执行
- **Arbitrated**: 多方仲裁投票

### GUI 数据连接
- 实时连接 Service API
- 节点列表自动刷新（5秒间隔）
- DAG 状态实时显示
- 终端命令集成

### P2P 网络
- DHT/Kademlia 节点发现
- NAT 穿透（STUN/TURN/UPnP）
- 连接质量监测
- 自动路由优化

### ACL 规则引擎
- 四种网络模式（Whitelist/Solitary/Open/Quarantine）
- 复杂规则条件（IP/时间/速率限制）
- ACL 同步传播

### 远程会话管理
- 多会话支持
- 会话持久化（SQLite）
- Agent 多路复用（Claude/Kimi/Aider/OpenCode）

---

## 🔒 安全

### 安全审计结果
- **依赖漏洞**: 0 关键漏洞 ✅
- **不安全代码**: 6 个 unsafe 块（全部审查通过）✅
- **输入验证**: 全面审查 ⚠️ 3 个中危待修复
- **模糊测试**: 3 个目标已设置 ✅
- **配置审查**: 完成 ✅

### 修复的漏洞
- RUSTSEC-2023-0071 (rsa crate) - 已移除

---

## ⚡ 性能优化

| 指标 | 目标 | 实际 |
|------|------|------|
| DAG P50/P95 | <50/<100ms | ✅ 达标 |
| 向量检索 P50/P95 | <20/<50ms | ✅ 达标 |
| DAG 吞吐量 | ≥100/秒 | ✅ 达标 |
| 内存使用 | <200MB | ✅ 达标 |

---

## 📦 生态集成

- **Homebrew**: `brew install cis` (待发布到 Tap)
- **VS Code**: 插件已开发，待提交市场
- **Docker**: 完整支持（Dockerfile + Compose）
- **Shell**: Bash/Zsh/Fish 补全和别名

---

## 📊 项目统计

- **代码行数**: 80,000+
- **测试用例**: 600+（全部通过）
- **文档**: 50+
- **unsafe 块**: 6 个（全部审查）
- **依赖**: 903 个 crate

---

## 🚀 快速开始

```bash
# 安装
git clone https://github.com/MoSiYuan/CIS.git
cd CIS
cargo build --release

# 初始化
./target/release/cis init

# 查看版本
./target/release/cis --version
# cis 1.1.0
```

---

## 📋 变更日志

详见 [CHANGELOG.md](https://github.com/MoSiYuan/CIS/blob/main/CHANGELOG.md)

---

## ⚠️ 已知限制

1. **WebSocket 默认无 TLS** - 建议生产环境启用 wss://
2. **节点密钥明文存储** - 建议文件权限 0o600
3. **测试并行隔离** - 使用 `--test-threads=1` 运行测试

---

## 📚 文档

- [架构设计](https://github.com/MoSiYuan/CIS/blob/main/plan/ARCHITECTURE_DESIGN.md)
- [安全审计报告](https://github.com/MoSiYuan/CIS/blob/main/reports/security/SECURITY_AUDIT_SUMMARY.md)
- [使用指南](https://github.com/MoSiYuan/CIS/blob/main/plan/user.md)

---

## 🙏 感谢

感谢所有为 CIS 做出贡献的开发者！

**完整变更**: [v1.0.0...v1.1.0](https://github.com/MoSiYuan/CIS/compare/v1.0.0...v1.1.0)
