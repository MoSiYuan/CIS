# GitHub Release 指南

> **当前版本**: v1.1.5
> **发布日期**: 2026-02-11
> **状态**: ✅ 已发布

CIS 使用自动化 CI/CD 流程发布版本。

---

## v1.1.5 Release Notes

### 🚀 新功能

#### Matrix 联邦增强
- **Matrix 首次登录验证码**: 6位 OTP 防止暴力破解
- **联邦请求签名**: Ed25519 签名验证
- **完整 Sync 实现**: 支持 joined/invited/left rooms
- **Bridge 真实执行**: 非模拟，真实执行技能

#### WASM Skill 沙箱
- **WASM 运行时**: Wasmer 引擎
- **WASI 沙箱**: 限制系统调用
- **资源限制**: 128MB 内存，30秒超时
- **四种技能类型**: Native/WASM/Remote/DAG

#### DHT 公共记忆
- **Kademlia DHT**: 分布式存储
- **公共记忆 API**: sync/get/list 操作
- **节点发现**: mDNS + DHT

#### Agent → Skill 直接调用
- **AgentCisClient**: 本地直接调用
- **绕过 Matrix**: 更低延迟

### 📊 测试和质量

- **测试通过**: 1104/1135 ✅
- **代码量**: 16.6 万行 Rust
- **测试覆盖**: 65%
- **Docker 环境**: 3 节点组网测试

### 📦 下载

```bash
# macOS/Linux
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash

# 或手动下载
# https://github.com/MoSiYuan/CIS/releases/tag/v1.1.5
```

---

## 快速发布流程

```bash
# 1. 更新版本号
vim Cargo.toml  # 更新所有 workspace 成员版本

# 2. 更新 CHANGELOG.md
vim CHANGELOG.md

# 3. 提交并打标签
git add -A
git commit -m "Release v0.2.0"
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin main --tags
```

GitHub Actions 会自动构建并创建 Release。

## 版本号规范

| 类型 | 格式 | 示例 |
|------|------|------|
| 正式版 | `v{major}.{minor}.{patch}` | `v0.2.0` |
| 预发布 | `v{major}.{minor}.{patch}-{type}.{num}` | `v0.2.0-alpha.1` |

类型: `alpha` (内测) → `beta` (公测) → `rc` (候选)

## 发布前检查清单

- [ ] `cargo test --all` 通过
- [ ] `cargo clippy --all` 无警告
- [ ] `CHANGELOG.md` 已更新
- [ ] 版本号已统一更新

## 手动触发构建

```bash
# 如果 CI 失败，手动触发
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

## Release 说明模板

创建 Release 时填写：

```markdown
## 新增功能
- 功能描述

## 修复
- 修复描述

## 安装
```bash
# macOS/Linux
curl -sSL https://github.com/user/cis/releases/download/v0.2.0/install.sh | bash

# 或手动下载对应平台二进制文件
```

## 完整变更日志
见 [CHANGELOG.md](./CHANGELOG.md)
```

## 故障排除

| 问题 | 解决 |
|------|------|
| CI 构建失败 | 检查 `cargo build` 本地是否通过 |
| 标签已存在 | 先删除远程标签再重新推送 |
| 发布未触发 | 确保标签格式为 `v{x}.{y}.{z}` |
