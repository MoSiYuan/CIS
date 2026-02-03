# CIS 发布配置指南

## 跨平台目录结构

CIS 遵循各平台标准，自动检测并使用正确的目录。

### 数据目录

| 平台 | 默认路径 | 环境变量覆盖 |
|------|---------|-------------|
| **macOS** | `~/Library/Application Support/CIS` | `CIS_DATA_DIR` |
| **Linux** | `~/.local/share/cis` | `CIS_DATA_DIR` / `XDG_DATA_HOME` |
| **Windows** | `%LOCALAPPDATA%\CIS` | `CIS_DATA_DIR` |

### 目录树

```
$CIS_DATA_DIR/
├── config.toml              # 主配置
├── node.key                 # 节点私钥
├── core/
│   ├── core.db              # 核心数据库（任务、配置、节点信息）
│   └── backup/              # 自动备份
├── skills/
│   ├── registry.json        # Skill 注册表
│   ├── installed/           # Skill 代码
│   │   ├── native/          # Native Skills
│   │   │   ├── ai-executor/
│   │   │   ├── im/          # Claude 开发的 IM
│   │   │   └── ...
│   │   └── wasm/            # WASM Skills
│   │       ├── memory-organizer.wasm
│   │       └── ...
│   └── data/                # Skill 独立数据库
│       ├── ai-executor/data.db
│       ├── im/data.db       # IM 数据
│       └── ...
├── logs/
│   ├── cis.log
│   └── skills/
├── cache/                   # 可安全删除
└── runtime/                 # 重启清空
```

## 构建配置

### macOS

```bash
# 安装目标
target add aarch64-apple-darwin
target add x86_64-apple-darwin

# 通用二进制 (Universal Binary)
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# 合并
lipo -create \
  target/aarch64-apple-darwin/release/cis \
  target/x86_64-apple-darwin/release/cis \
  -output target/release/cis-macos-universal
```

#### .app Bundle 结构

```
CIS.app/
├── Contents/
│   ├── Info.plist
│   ├── PkgInfo
│   ├── MacOS/
│   │   └── cis                    # 主二进制
│   ├── Resources/
│   │   ├── icon.icns
│   │   └── builtin-skills/        # 内置 Skill
│   └── Library/
│       └── LoginItems/            # 开机启动 (可选)
```

### Linux

```bash
# x86_64
cargo build --release --target x86_64-unknown-linux-gnu

# ARM64
cargo build --release --target aarch64-unknown-linux-gnu

# musl (静态链接)
cargo build --release --target x86_64-unknown-linux-musl
```

#### AppImage

```yaml
# appimage-builder.yml
version: 1
script:
  - rm -rf AppDir | true
  - mkdir -p AppDir/usr/bin
  - cp target/release/cis AppDir/usr/bin/
AppDir:
  path: ./AppDir
  app_info:
    id: com.cis.app
    name: CIS
    icon: cis
    version: 0.4.0
    exec: usr/bin/cis
    exec_args: $@
```

#### deb 包

```bash
# cargo-deb
cargo deb
```

### Windows

```bash
# MSVC
target add x86_64-pc-windows-msvc

# GNU
target add x86_64-pc-windows-gnu

cargo build --release --target x86_64-pc-windows-msvc
```

#### 安装程序 (WiX)

```xml
<!-- cis.wxs -->
<Product>
  <Package InstallerVersion="200" Compressed="yes" />
  <Directory Id="TARGETDIR" Name="SourceDir">
    <Directory Id="ProgramFiles64Folder">
      <Directory Id="INSTALLFOLDER" Name="CIS">
        <Component>
          <File Source="target\release\cis.exe" />
        </Component>
      </Directory>
    </Directory>
  </Directory>
</Product>
```

## 内置 Skill

发布时可以选择内置 Skill（随应用分发）：

```toml
# cis-core/Cargo.toml
[features]
default = ["builtin-ai-executor", "builtin-init-wizard"]
builtin-ai-executor = []
builtin-init-wizard = []
builtin-im = []  # Claude 开发的 IM
```

内置 Skill 目录：
- macOS: `CIS.app/Contents/Resources/builtin-skills/`
- Linux: `/opt/cis/lib/builtin-skills/`
- Windows: `C:\Program Files\CIS\builtin-skills\`

## 热插拔与数据隔离

### 核心数据库 (`core/core.db`)

```rust
// 只存储核心数据
pub struct CoreDb;
impl CoreDb {
    pub fn open() -> Result<Self>;
    pub fn set_config(&self, key: &str, value: &[u8]) -> Result<()>;
    pub fn register_memory_index(&self, key: &str, skill: Option<&str>) -> Result<()>;
}
```

### Skill 数据库 (`skills/data/{name}/data.db`)

```rust
// 每个 Skill 独立数据库
pub struct SkillDb {
    name: String,
    conn: Connection,
}

// 热插拔：独立加载/卸载，不影响核心
impl SkillDb {
    pub fn open(name: &str) -> Result<Self>;
    pub fn close(self) -> Result<()>;
}
```

### 使用示例

```rust
use cis_core::storage::{Paths, DbManager, CoreDb, SkillDb};
use cis_core::skill::{SkillManager, SkillMeta, SkillType};

fn main() -> Result<()> {
    // 1. 初始化目录
    Paths::ensure_dirs()?;
    Paths::cleanup_runtime()?; // 清理上次运行残留
    
    // 2. 创建数据库管理器
    let db_manager = Arc::new(DbManager::new()?);
    
    // 3. 创建 Skill 管理器
    let skill_manager = SkillManager::new(db_manager.clone())?;
    
    // 4. 加载已注册 Skills
    for info in skill_manager.list_registered()? {
        if info.auto_load {
            skill_manager.load(&info.name, LoadOptions::default())?;
        }
    }
    
    // 5. 热插拔示例
    skill_manager.load("im", LoadOptions::default())?;
    skill_manager.activate("im")?;
    
    // ... 运行中 ...
    
    // 6. 卸载 IM（热插拔）
    skill_manager.unload("im")?;
    
    // 7. 关闭
    db_manager.shutdown()?;
    Ok(())
}
```

## CI/CD 配置

### GitHub Actions

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      - run: cargo build --release
      - run: cargo bundle --release  # macOS app bundle
      - uses: actions/upload-artifact@v4
        with:
          name: cis-macos
          path: target/release/bundle/osx/CIS.app

  build-linux:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      - run: cargo build --release
      - run: cargo deb
      - run: cargo appimage
      - uses: actions/upload-artifact@v4
        with:
          name: cis-linux
          path: |
            target/debian/*.deb
            target/appimage/*.AppImage

  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-action@stable
      - run: cargo build --release
      - run: cargo wix
      - uses: actions/upload-artifact@v4
        with:
          name: cis-windows
          path: target/wix/*.msi
```

## 版本策略

| 版本 | 说明 |
|-----|------|
| `0.4.0` | 当前开发版本 - 热插拔 + 数据隔离 |
| `0.5.0` | WASM Runtime + Host API |
| `0.6.0` | P2P 网络 + 节点发现 |
| `1.0.0` | 稳定版 - 完整功能 |

## 迁移指南

### 从旧版本迁移

```rust
// cis-core/src/storage/migration.rs
pub fn migrate_from_v03_to_v04() -> Result<()> {
    // 1. 备份旧数据
    let backup = BackupManager::new();
    backup.backup_core()?;
    
    // 2. 创建新目录结构
    Paths::ensure_dirs()?;
    
    // 3. 迁移核心数据到新数据库
    let core_db = CoreDb::open()?;
    // ... 数据迁移逻辑 ...
    
    // 4. 移动 Skill 数据到独立数据库
    for skill in list_installed_skills()? {
        let skill_db = SkillDb::open(&skill.name)?;
        // ... 数据迁移逻辑 ...
    }
    
    Ok(())
}
```

## 调试与诊断

```bash
# 查看目录结构
cis debug paths

# 检查数据库状态
cis debug db-status

# 列出已加载 Skills
cis skill list

# 手动加载/卸载 Skill
cis skill load im
cis skill unload im

# 备份数据
cis backup create
cis backup list
cis backup restore <timestamp>
```

## 安全注意事项

1. **node.key**: 节点私钥，使用操作系统密钥链存储（macOS Keychain, Windows DPAPI, Linux Secret Service）
2. **Skill 隔离**: 每个 Skill 独立数据库，无法直接访问核心数据
3. **权限**: Skill 注册时声明权限，运行时检查
4. **备份加密**: 敏感备份文件应加密存储
