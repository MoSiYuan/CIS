# CIS Homebrew Formula

这个目录包含 CIS 的 Homebrew Formula 和相关脚本。

## 使用方法

### 方式一：使用自建 Tap（推荐）

```bash
# 添加 Tap
brew tap mosiyuan/cis

# 安装 CIS
brew install cis

# 升级到最新版本
brew upgrade cis
```

### 方式二：直接安装

```bash
# 下载 Formula 并安装
brew install --formula ./cis.rb
```

### 方式三：从源码构建

```bash
# 使用 --build-from-source 或 --head 选项
brew install --build-from-source ./cis.rb

# 或者安装最新开发版本
brew install --head ./cis.rb
```

## 自动更新脚本

`update-formula.sh` 脚本用于在发布新版本时自动更新 Formula：

```bash
# 更新到指定版本
./update-formula.sh 1.2.0

# 脚本会自动：
# 1. 下载各平台二进制文件
# 2. 计算 SHA256 校验和
# 3. 更新 Formula 中的版本和校验和
# 4. 可选：提交 PR 到 Homebrew Core
```

## 提交到 Homebrew Core

当项目满足以下条件时，可以提交到官方 Homebrew Core：

1. ✅ 使用 GitHub 仓库
2. ✅ 有稳定的 release
3. ✅ 有可下载的二进制文件或从源码构建
4. ✅ 通过 `brew audit --new-formula` 检查
5. ✅ 有合理的 popularity 指标（stars/forks）

提交步骤：

```bash
# 1. Fork homebrew-core
# https://github.com/Homebrew/homebrew-core

# 2. 创建新分支
git checkout -b cis

# 3. 复制 Formula
cp packaging/homebrew/cis.rb Formula/c/cis.rb

# 4. 运行测试
brew install --build-from-source cis
brew test cis
brew audit --new-formula cis

# 5. 提交 PR
```

## Formula 说明

- **版本管理**: 使用 GitHub Releases 的预编译二进制文件
- **多平台支持**: macOS (Intel/Apple Silicon) 和 Linux (x86_64/ARM64)
- **Shell 补全**: 自动安装 Bash/Zsh/Fish 补全脚本
- **服务管理**: 支持 `brew services` 管理后台服务
- **测试**: 包含简单的功能测试

## 注意事项

1. **SHA256 占位符**: 发布新版本前需要更新 SHA256 校验和
2. **二进制文件名**: 确保 release 中的文件名与 Formula 匹配
3. **依赖项**: 更新依赖版本时需要同步修改 Formula
