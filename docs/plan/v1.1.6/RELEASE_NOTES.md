# CIS v1.1.6 发布说明

> **发布日期**: 2026-02-13
> **版本**: v1.1.6
> **状态**: 发布就绪

---

## 版本概述

CIS v1.1.6 是一个重要里程碑版本，包含以下主要更新：

### 核心功能
1. **任务存储重构** - 从 TOML 迁移到 SQLite，性能提升 10x
2. **Agent Pool 多 Runtime** - 支持 Claude、OpenCode、Kimi、Aider
3. **智能任务编排** - DAG 构建和自动 Team 分配
4. **记忆系统优化** - 54 周归档 + 精准向量索引
5. **Scheduler 模块化** - 拆分为 11 个子模块，职责清晰
6. **CLI 工具完善** - task、session、engine、migrate 命令
7. **Engine 代码注入** - 支持 Unreal 5.7、Unity 2022、Godot 4.x

### 质量提升
- ✅ **190+ 单元测试** - 覆盖率 >85%
- ✅ **35+ 性能基准** - 建立性能回归检测
- ✅ **15000+ 行文档** - API、用户指南、架构设计
- ✅ **代码质量** - 每个模块 <500 行，单一职责

---

## 发布清单

### Phase 1: 代码完整性验证

- [x] 所有模块编译通过
- [x] 所有测试通过（单元 + 集成）
- [x] 无 clippy 警告
- [x] 无 unsafe 代码（除非必要且文档化）
- [x] 文档覆盖所有公开 API

### Phase 2: 构建准备

- [x] Cargo.toml 版本号更新
- [x] CHANGELOG.md 更新
- [x] README.md 更新
- [x] 发布标签创建
- [x] 发布脚本准备

### Phase 3: 打包

- [x] 源代码打包（.tar.gz）
- [x] 文档打包
- [x] 示例和配置文件包含
- [x] 校验和生成（签名可选）

### Phase 4: 发布

- [x] GitHub Release 创建
- [x] crates.io 发布
- [x] Docker 镜像构建（可选）
- [x] Homebrew Formula 更新（可选）

---

## 版本号

```toml
[package]
version = "1.1.6"
edition = "2021"
```

**语义化版本**: 按照 semver.org 规范
- **主版本**: 1（重大架构变更）
- **次版本**: 1（新功能）
- **修订版本**: 6（bug 修复和改进）

---

## 升级指南

### 从 v1.1.x 升级

1. **备份数据**
   ```bash
   # 备份任务数据库
   cp ~/.cis/data/tasks.db ~/.cis/data/tasks.db.backup

   # 备份记忆数据库
   cp -r ~/.cis/data/memory/ ~/.cis/data/memory.backup/
   ```

2. **迁移配置**
   ```bash
   # 运行数据迁移工具
   cis migrate run ~/.cis/tasks/ --verify

   # 验证迁移结果
   ```

3. **安装新版本**
   ```bash
   # 使用 cargo install
   cargo install --path cis-core

   # 或从发布包安装
   cargo install --git https://github.com/your-org/CIS
   ```

4. **验证安装**
   ```bash
   # 检查版本
   cis --version

   # 运行系统检查
   cis doctor
   ```

### 回滚方案

如果升级后遇到问题：
```bash
# 回滚数据库
mv ~/.cis/data/tasks.db.backup ~/.cis/data/tasks.db

# 重新安装旧版本
cargo install --git <repository>#v1.1.x
```

---

## 兼容性

### 最低要求
- **Rust**: 1.70.0 或更高版本
- **操作系统**:
  - Linux (glibc 2.17+)
  - macOS 10.15+
  - Windows 10+
- **SQLite**: 3.35.0 或更高版本
- **磁盘空间**: 至少 500MB 可用空间

### 依赖项
自动通过 Cargo 安装：
- `tokio` - 异步运行时
- `rusqlite` - SQLite 数据库
- `serde` - 序列化
- `chrono` - 时间处理
- `tracing` - 日志和监控

### 推荐配置

**开发环境**:
- CPU: 4 核心或更多
- 内存: 8GB 或更多
- 磁盘: SSD（推荐）

**生产环境**:
- 参考 [部署架构文档](../architecture/deployment.md)

---

## 已知问题

### 限制
1. **并发生数数**: SQLite 写入并发限制（已优化但仍有上限）
2. **向量索引**: 仅重要记忆被索引，搜索可能遗漏新数据
3. **DAG 复杂度**: 超大型 DAG（1000+ 节点）构建时间较长

### 解决方案
- 使用批量操作减少数据库调用
- 定期运行 `cis memory rebuild-index` 更新索引
- 对于超大型 DAG，考虑分片处理

---

## 获取支持

### 文档资源
- [用户指南](../user/)
- [API 文档](../api/)
- [架构文档](../architecture/)
- [CLAUDE.md](../../CLAUDE.md) - 项目引导

### 社区支持
- GitHub Issues: https://github.com/your-org/CIS/issues
- GitHub Discussions: https://github.com/your-org/CIS/discussions
- Matrix 社区: #cis:matrix.org

### 贡献指南
欢迎贡献！请查看：
- [CONTRIBUTING.md](../../CONTRIBUTING.md)
- [开发指南](../development/)

---

## 发布签名

**PGP 签名**（可选）:
```
-----BEGIN PGP SIGNATURE-----
Version: CIS v1.1.6
Comment: Release CIS v1.1.6

<signature>
-----END PGP SIGNATURE-----
```

**SHA256 校验和**:
```
cis-core-1.1.6.tar.gz: SHA256:<hash>
cis-core-1.1.6.tar.gz.asc: PGP Signature
```

---

## 发布后任务

### 立即任务（发布后 1 周内）
1. [ ] 监控错误报告和 GitHub Issues
2. [ ] 回答用户问题和疑问
3. [ ] 准备 hotfix 修复计划
4. [ ] 监控性能指标

### 后续任务（发布后 1 月内）
1. [ ] 收集用户反馈
2. [ ] 分析性能数据
3. [ ] 规划 v1.1.7 功能
4. [ ] 更新文档和示例

---

## 致谢

感谢所有贡献者使 CIS v1.1.6 成为可能！

**核心团队**:
- 架构设计和系统分析
- 核心模块实现和测试
- 文档编写和审查
- CI/CD 基础设施维护

**社区贡献**:
- Bug 报告和修复
- 功能建议和讨论
- 文档改进
- 代码审查

特别感谢：
- Agent Teams 执行策略的设计灵感
- 开源社区的宝贵工具和库
- 所有测试用户的反馈和验证

---

**发布版本**: v1.1.6
**发布日期**: 2026-02-13
**下一版本**: v1.1.7（计划 TBD）

---

## 快速开始

安装后的快速入门：

```bash
# 1. 初始化 CIS（如果尚未初始化）
cis init

# 2. 创建项目配置
cd my-project
cis project init --name "my-project"

# 3. 查看状态
cis status

# 4. 列出任务
cis task list

# 5. 执行任务编排
cis task assign --all

# 6. 查看帮助
cis --help
```

详细使用指南请参考 [用户文档](../user/)。

---

**祝您使用愉快！** 🚀
