# GitHub Release 指南

CIS 使用自动化 CI/CD 流程发布版本。

## 快速发布

```bash
# 1. 更新版本号 (示例: 0.2.0)
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
