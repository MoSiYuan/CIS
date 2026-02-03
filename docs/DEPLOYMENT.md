# CIS 部署指南

## 系统要求

- **操作系统**: macOS 12+, Ubuntu 20.04+, Windows 10+
- **Rust**: 1.70+
- **SQLite**: 3.40+ (with FTS5, vec0)
- **内存**: 4GB+ (推荐 8GB)
- **磁盘**: 2GB+ 可用空间

## 安装

### 从源码构建

```bash
git clone https://github.com/your-org/cis.git
cd cis
cargo build --release
```

### 安装到系统

```bash
# macOS/Linux
sudo cp target/release/cis /usr/local/bin/

# Windows
# 将 target\release\cis.exe 添加到 PATH
```

## 配置

### 1. 初始化配置

```bash
cis init
```

### 2. 配置 AI Provider

编辑 `~/.cis/config.toml`:

```toml
[ai]
provider = "claude"

[ai.claude]
api_key = "your-api-key"
```

### 3. 配置向量存储

```toml
[vector]
embedding_dim = 768
use_hnsw = true
hnsw_m = 16
hnsw_ef_construction = 100
```

## 升级

```bash
# 备份数据
cp -r ~/.cis ~/.cis.backup

# 拉取新版本
git pull origin main
cargo build --release

# 迁移数据 (如果需要)
cis doctor --migrate
```

## 监控

### 查看遥测

```bash
cis telemetry logs --limit 100
cis telemetry stats
```

### 清理旧数据

```bash
# 清理 30 天前的日志
cis telemetry cleanup --days 30

# 清理旧向量
cis memory cleanup --threshold 0.3
```

## 故障排除

### 数据库损坏

```bash
# 检查数据库完整性
cis doctor --check-db

# 修复数据库
cis doctor --repair-db
```

### 性能问题

1. 检查 HNSW 索引是否启用
2. 监控向量数据库大小
3. 考虑分片存储
