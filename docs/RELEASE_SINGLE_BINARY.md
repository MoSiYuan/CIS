# Release 单文件分发解决方案

**问题**: CIS 项目基于 TOML 配置，但 Release 版本是单可执行文件。如何解决这个问题？

**解决方案**: Release 模式下自动初始化

---

## 核心机制

### 1. 自动检测 Release 模式

```rust
pub fn run_mode() -> RunMode {
    // 如果可执行文件在 target/release 中，认为是 Release 模式
    if let Ok(exe_path) = std::env::current_exe() {
        let exe_str = exe_path.to_string_lossy();
        if exe_str.contains("target/release/") || exe_str.contains("target\\release\\") {
            return RunMode::Release;
        }
    }
    
    // 检查环境变量 CIS_PORTABLE=1 强制使用便携模式
    if std::env::var("CIS_PORTABLE").unwrap_or_default() == "1" {
        return RunMode::Release;
    }
    
    RunMode::Development
}
```

### 2. Release 模式自动初始化

当检测到 Release 模式且配置文件不存在时，**自动创建默认配置**：

```rust
async fn check_first_run(command: &Commands) -> anyhow::Result<()> {
    if needs_init && !Paths::config_file().exists() {
        // Release 模式下自动初始化
        if Paths::run_mode() == RunMode::Release {
            eprintln!("📦 Release 模式：自动初始化 CIS...");
            
            // 1. 生成节点密钥
            let node_key = generate_node_key();
            save_node_key(&node_key)?;
            
            // 2. 创建默认配置
            let config = create_default_config(&node_key).await?;
            save_config(&config)?;
            
            // 3. 创建数据目录结构
            Paths::ensure_dirs()?;
            
            eprintln!("✅ CIS 自动初始化完成");
            return Ok(());
        }
        
        // 开发模式：提示用户初始化
        // ...
    }
}
```

### 3. 生成的默认配置

```toml
# CIS Global Configuration
# Generated at: 2026-02-03 10:30:00

[node]
id = "550e8400-e29b-41d4-a716-446655440000"
name = "username"
key = "a1b2c3d4e5f6..."  # 自动生成的 32 字节密钥

[ai]
default_provider = "claude"

[ai.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[storage]
max_backups = 10
backup_interval_days = 7

[p2p]
enabled = true
listen_port = 7677
enable_dht = true
enable_nat_traversal = true

[p2p.bootstrap]
nodes = []
```

---

## 文件布局

### Release 模式（单文件 + 自动创建的数据）

```
/path/to/cis/                    # 可执行文件所在目录
├── cis                          # 单可执行文件（Release 产物）
└── .cis/                        # 自动创建的目录
    ├── config.toml              # 自动生成的配置
    ├── node.key                 # 自动生成的节点密钥
    ├── node.db                  # 核心数据库
    ├── memory.db                # 记忆数据库
    ├── federation.db            # 联邦数据库
    ├── vector.db                # 向量数据库
    ├── skills/                  # Skill 目录
    ├── logs/                    # 日志目录
    └── cache/                   # 缓存目录
```

### 便携模式（USB/移动硬盘）

```
E:/CIS/                          # USB 驱动器
├── cis.exe                      # Windows 可执行文件
├── .cis/                        # 完整数据目录
│   ├── config.toml
│   ├── node.key
│   └── ...
└── data/                        # 用户数据
    └── ...
```

---

## 分发方式

### 方式 1: 单文件 + 自动初始化（推荐）

**适用场景**: 普通用户下载安装

**流程**:
1. 用户下载 `cis`（或 `cis.exe`）
2. 首次运行自动创建 `.cis/` 目录和默认配置
3. 无需手动初始化

**优点**:
- 极简安装体验
- 开箱即用
- 无依赖

**缺点**:
- 首次启动稍慢（需要生成密钥和配置）
- 无法预设配置

### 方式 2: 打包为安装包（.dmg/.msi/.deb）

**适用场景**: 正式软件分发

**流程**:
1. 安装包包含预生成的默认配置
2. 安装时复制到标准目录
3. 用户首次运行时只需确认

**安装包内容**:
```
CIS-1.0.0.dmg
├── CIS.app/
│   └── Contents/MacOS/cis
└── .cis/                    # 预生成的配置模板
    └── config.toml
```

**优点**:
- 专业安装体验
- 可以预设配置
- 支持卸载

### 方式 3: 便携压缩包

**适用场景**: 技术用户、离线环境

**流程**:
1. 下载 `cis-portable.zip`
2. 解压到任意目录
3. 运行即创建数据

**压缩包内容**:
```
cis-portable/
├── cis                       # 可执行文件
├── README.md                 # 说明文档
└── .cis/                     # 可选：预配置
    └── config.toml
```

---

## 配置优先级

当配置存在冲突时，优先级从高到低：

1. **环境变量** `CIS_DATA_DIR=/path`
   - 完全自定义数据位置
   
2. **Release 自动创建**（可执行文件旁 `.cis/`）
   - 单文件分发场景
   
3. **Git 项目模式**（项目根目录 `.cis/`）
   - 开发场景
   
4. **系统默认**（`~/.cis/` 或 `%USERPROFILE%\.cis\`）
   - 传统安装模式

---

## 首次运行流程

### Release 模式

```bash
$ ./cis status
📦 Release 模式：自动初始化 CIS...
✅ CIS 自动初始化完成
   配置: /path/to/cis/.cis/config.toml
   数据: /path/to/cis/.cis

📡 P2P Network Status
Node ID:    550e8400-e29b-41d4-a716-446655440000
Node Name:  username

P2P Configuration:
  Enabled:  ✅
  Port:     7677
  DHT:      ✅
  NAT:      ✅

Network Status:
  State:    🟡 Not connected
```

### 开发模式

```bash
$ cargo run --bin cis-node -- status
⚠️  CIS 尚未初始化

📁 CIS 路径信息:
  运行模式: Development (开发模式)
  数据目录:   /Users/username/.cis
  配置目录:   /Users/username/.cis
  配置文件:   /Users/username/.cis/config.toml

💡 请先初始化 CIS:
   cis init           # 交互式初始化
   cis init --non-interactive --provider claude
```

---

## 构建配置

### GitHub Actions 构建

```yaml
- name: Build Release
  run: cargo build --release --bin cis-node

- name: Package
  run: |
    mkdir -p cis-${VERSION}
    cp target/release/cis-node cis-${VERSION}/cis
    cp README.md cis-${VERSION}/
    tar czf cis-${VERSION}-${TARGET}.tar.gz cis-${VERSION}/
```

### 本地构建

```bash
# Release 构建
cargo build --release --bin cis-node

# 测试 Release 模式（模拟）
CIS_PORTABLE=1 ./target/release/cis-node status
```

---

## 常见问题

### Q: 如何迁移配置？

**A**: 复制 `.cis/` 目录到新位置：

```bash
# 备份
cp -r /old/path/.cis /backup/

# 恢复
cp -r /backup/.cis /new/path/
```

### Q: 如何重置配置？

**A**: 删除 `.cis/` 目录，下次运行自动重新创建：

```bash
rm -rf /path/to/cis/.cis
./cis status  # 自动重新初始化
```

### Q: 如何自定义配置？

**A**: 编辑自动生成的 `config.toml`：

```bash
# 编辑配置
vim /path/to/cis/.cis/config.toml

# 重启生效
./cis p2p restart
```

### Q: 多个版本共存？

**A**: 使用环境变量隔离：

```bash
# 版本 A
export CIS_DATA_DIR=/path/to/cis-a/.cis
./cis-a/cis status

# 版本 B  
export CIS_DATA_DIR=/path/to/cis-b/.cis
./cis-b/cis status
```

---

## 技术细节

### 节点密钥生成

```rust
fn generate_node_key() -> Vec<u8> {
    use rand::RngCore;
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key.to_vec()
}
```

### 权限设置

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(&key_path)?.permissions();
    permissions.set_mode(0o600);  // 仅所有者可读写
    std::fs::set_permissions(&key_path, permissions)?;
}
```

### 配置原子写入

```rust
// 先写入临时文件，再重命名，避免配置损坏
let temp_path = config_path.with_extension("tmp");
std::fs::write(&temp_path, config)?;
std::fs::rename(&temp_path, config_path)?;
```

---

## 总结

| 场景 | 推荐方式 | 用户体验 |
|------|---------|---------|
| 普通用户 | 单文件 + 自动初始化 | ⭐⭐⭐⭐⭐ |
| 企业部署 | 安装包 (.msi/.dmg/.deb) | ⭐⭐⭐⭐⭐ |
| 技术用户 | 便携压缩包 | ⭐⭐⭐⭐ |
| 开发测试 | cargo run | ⭐⭐⭐ |

**核心优势**: Release 单文件无需预先配置，首次运行自动完成初始化，实现真正的"开箱即用"。
