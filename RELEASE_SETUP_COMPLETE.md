# GitHub Release 设置完成报告

**日期**: 2026-02-03  
**状态**: ✅ **已完成**

---

## 📦 创建的文件

### GitHub Actions 工作流 (3个)

| 文件 | 用途 |
|------|------|
| `.github/workflows/ci.yml` | PR 和推送时的持续集成 |
| `.github/workflows/release.yml` | 版本发布自动构建 |
| `.github/workflows/nightly.yml` | 每日夜间构建 |

### 构建脚本 (7个)

#### macOS
| 文件 | 用途 |
|------|------|
| `scripts/build/macos/create-app.sh` | 创建 `.app` bundle |
| `scripts/build/macos/create-dmg.sh` | 创建 `.dmg` 安装包 |

#### Linux
| 文件 | 用途 |
|------|------|
| `scripts/build/linux/create-appimage.sh` | 创建 AppImage |
| `scripts/build/linux/create-deb.sh` | 创建 `.deb` 包 |

#### Windows
| 文件 | 用途 |
|------|------|
| `scripts/build/windows/build.bat` | 批处理构建 |
| `scripts/build/windows/create-msi.ps1` | 创建 MSI 安装包 |

### Issue/PR 模板 (4个)

| 文件 | 用途 |
|------|------|
| `.github/ISSUE_TEMPLATE/bug_report.md` | Bug 报告模板 |
| `.github/ISSUE_TEMPLATE/feature_request.md` | 功能请求模板 |
| `.github/ISSUE_TEMPLATE/config.yml` | Issue 配置 |
| `.github/pull_request_template.md` | PR 模板 |

### 文档 (4个)

| 文件 | 用途 |
|------|------|
| `CHANGELOG.md` | 版本变更日志 |
| `RELEASE_CHECKLIST.md` | 发布检查清单 |
| `scripts/build/README.md` | 构建脚本文档 |
| `.github/README.md` | GitHub 配置文档 |

---

## 🚀 使用方法

### 创建新版本发布

```bash
# 1. 更新版本号 (Cargo.toml)
# 2. 更新 CHANGELOG.md
# 3. 提交更改
git add .
git commit -m "Prepare v1.0.0 release"

# 4. 创建标签
git tag -a v1.0.0 -m "Release v1.0.0"

# 5. 推送标签 (触发自动构建)
git push origin v1.0.0
```

GitHub Actions 将自动：
1. 在 macOS/Linux/Windows 上并行构建
2. 创建各平台的安装包
3. 创建 GitHub Release
4. 上传所有构建产物
5. 更新 `latest` 标签

### 下载最新版本

**macOS:**
```bash
brew install cis
```

**Linux:**
```bash
curl -fsSL https://github.com/your-org/cis/releases/latest/download/install.sh | bash
```

**Windows:**
```powershell
irm https://github.com/your-org/cis/releases/latest/download/install.ps1 | iex
```

---

## 📋 Release 产物

每个 Release 包含以下文件：

| 平台 | 文件 | 说明 |
|------|------|------|
| **macOS** | `CIS-{version}-macos.dmg` | 标准安装包 |
| | `CIS-{version}-macos.app.tar.gz` | 便携版 |
| **Linux** | `CIS-{version}-x86_64.AppImage` | 便携版 (无需安装) |
| | `cis_{version}_amd64.deb` | Debian/Ubuntu 安装包 |
| | `cis-{version}-linux-x86_64.tar.gz` | 通用二进制 |
| **Windows** | `CIS-{version}-x86_64.msi` | 标准安装程序 |
| | `CIS-{version}-windows-x86_64.zip` | 便携版 |

---

## ⚙️ 代码签名配置 (可选)

### macOS
1. 获取 Apple Developer ID 证书
2. 导出为 `.p12` 格式
3. 在 GitHub Settings > Secrets 中添加：
   - `MACOS_CERTIFICATE`: Base64 编码的证书
   - `MACOS_CERTIFICATE_PWD`: 证书密码

### Windows
1. 获取代码签名证书
2. 导出为 `.pfx` 格式
3. 在 GitHub Settings > Secrets 中添加：
   - `WINDOWS_CERTIFICATE`: Base64 编码的证书
   - `WINDOWS_CERTIFICATE_PWD`: 证书密码

---

## 🔍 本地测试构建

### macOS
```bash
./scripts/build/macos/create-app.sh 1.0.0
./scripts/build/macos/create-dmg.sh 1.0.0
```

### Linux
```bash
./scripts/build/linux/create-appimage.sh 1.0.0
./scripts/build/linux/create-deb.sh 1.0.0
```

### Windows
```powershell
.\scripts\build\windows\create-msi.ps1 -Version "1.0.0"
# 或
.\scripts\build\windows\build.bat 1.0.0
```

---

## 📊 构建状态

| 平台 | 状态 | 产物 |
|------|------|------|
| macOS | ✅ 配置完成 | .app, .dmg |
| Linux | ✅ 配置完成 | .AppImage, .deb, .tar.gz |
| Windows | ✅ 配置完成 | .msi, .zip |

---

## 📝 下一步

1. **配置代码签名证书** (可选但推荐)
   - 提升用户信任度
   - 避免安全警告

2. **测试发布流程**
   - 创建测试标签 `v0.9.9-rc1`
   - 验证所有产物生成
   - 在各平台测试安装

3. **准备正式发布**
   - 更新版本号到 `1.0.0`
   - 完善 CHANGELOG
   - 执行 `RELEASE_CHECKLIST.md`

4. **发布 v1.0.0**
   ```bash
   git tag -a v1.0.0 -m "First stable release"
   git push origin v1.0.0
   ```

---

## ✅ 检查清单

- [x] CI/CD 工作流配置
- [x] 跨平台构建脚本
- [x] Issue/PR 模板
- [x] CHANGELOG.md
- [x] 发布检查清单
- [ ] 代码签名证书 (用户配置)
- [ ] 首次发布测试

---

**CIS 已准备好发布到 GitHub Releases！**
