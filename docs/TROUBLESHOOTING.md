# CIS 故障排除指南

本文档帮助您解决使用 CIS 时可能遇到的常见问题。

## 快速诊断

运行诊断命令获取系统状态：

```bash
cis doctor           # 完整环境检查
cis doctor --fix     # 自动修复可修复的问题
cis status --paths   # 查看路径配置
```

---

## 安装问题

### 问题：下载安装脚本失败

**症状**：
```
curl: (6) Could not resolve host: raw.githubusercontent.com
```

**解决方案**：

1. **使用代理**（如果需要）：
   ```bash
   export https_proxy=http://proxy.example.com:8080
   curl -fsSL ...
   ```

2. **使用镜像**：
   ```bash
   # 使用 jsDelivr 镜像
   curl -fsSL https://cdn.jsdelivr.net/gh/MoSiYuan/CIS@main/scripts/install/install.sh | bash
   ```

3. **手动下载**：
   ```bash
   # 从 GitHub Releases 手动下载
   wget https://github.com/MoSiYuan/CIS/releases/latest/download/cis-linux-x86_64.tar.gz
   tar xzf cis-linux-x86_64.tar.gz
   sudo mv cis-node /usr/local/bin/cis
   ```

### 问题：macOS "无法验证开发者"

**症状**：
```
"cis" cannot be opened because the developer cannot be verified.
```

**解决方案**：

1. **临时允许**（推荐用于测试）：
   ```bash
   # 右键点击 cis 二进制文件，选择"打开"
   # 或在终端执行：
   xattr -d com.apple.quarantine $(which cis)
   ```

2. **系统设置**（永久）：
   - 打开 "系统设置" → "隐私与安全性"
   - 找到关于 cis 的安全提示，点击"仍要打开"

### 问题：权限被拒绝

**症状**：
```
bash: /usr/local/bin/cis: Permission denied
```

**解决方案**：

```bash
# 添加执行权限
chmod +x /usr/local/bin/cis

# 或重新安装到用户目录
curl -fsSL ... | bash -s -- --install-dir "$HOME/.local/bin"
```

---

## 初始化问题

### 问题：CIS 找不到配置文件

**症状**：
```
Error: Configuration not found at ~/.cis/config.toml
```

**解决方案**：

1. **检查路径**：
   ```bash
   cis status --paths
   ```

2. **手动指定数据目录**：
   ```bash
   export CIS_DATA_DIR=/path/to/data
   cis init
   ```

3. **强制重新初始化**：
   ```bash
   cis init --force
   ```

### 问题：初始化时数据库锁定

**症状**：
```
Error: database is locked
```

**解决方案**：

1. **检查是否有其他 CIS 进程在运行**：
   ```bash
   ps aux | grep cis
   # 如果有，等待其完成或终止它
   kill <pid>
   ```

2. **删除锁定文件**（如果进程已崩溃）：
   ```bash
   rm ~/.cis/data/*.lock
   ```

3. **使用 WAL 模式检查**：
   ```bash
   # 检查 SQLite 数据库
   sqlite3 ~/.cis/data/core.db "PRAGMA journal_mode;"
   # 应该返回 "wal"
   ```

### 问题：Git 项目模式不生效

**症状**：在 Git 项目中运行 CIS，但使用了全局配置

**解决方案**：

1. **确认当前目录**：
   ```bash
   pwd
   git rev-parse --show-toplevel
   ```

2. **检查 .cis 目录**：
   ```bash
   ls -la .cis/
   # 如果不存在，初始化项目级配置
   cis init --project
   ```

3. **设置环境变量强制项目模式**：
   ```bash
   export CIS_PROJECT_MODE=1
   cis status --paths
   ```

---

## 网络问题

### 问题：P2P 连接失败

**症状**：
```
Error: Failed to connect to peer: connection refused
```

**解决方案**：

1. **检查端口**：
   ```bash
   # 检查端口是否被占用
   lsof -i :7677  # QUIC 端口
   lsof -i :6767  # WebSocket 端口
   ```

2. **防火墙设置**：
   ```bash
   # macOS
   sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add $(which cis)
   
   # Linux (ufw)
   sudo ufw allow 7677/tcp
   sudo ufw allow 7677/udp
   sudo ufw allow 6767/tcp
   ```

3. **NAT 穿透**：
   ```bash
   # 检查 UPnP 是否可用
   cis p2p status
   
   # 手动配置端口转发
   # 在路由器上转发 7677 端口到本机
   ```

### 问题：DID 验证失败

**症状**：
```
Error: DID verification failed: invalid signature
```

**解决方案**：

1. **检查节点密钥**：
   ```bash
   # 确认密钥存在且权限正确
   ls -la ~/.cis/node_key
   # 权限应为 600 (-rw-------)
   ```

2. **重新生成密钥**：
   ```bash
   # 备份后删除密钥文件
   mv ~/.cis/node_key ~/.cis/node_key.bak
   cis init --force
   ```

3. **检查系统时间**：
   ```bash
   # DID 验证对时间敏感
   date
   # 如果时间不正确，同步系统时间
   sudo ntpdate -s time.apple.com  # macOS
   sudo timedatectl set-ntp true    # Linux
   ```

---

## 存储问题

### 问题：磁盘空间不足

**症状**：
```
Error: database or disk is full
```

**解决方案**：

1. **检查空间使用**：
   ```bash
   du -sh ~/.cis/
   cis system cleanup --dry-run  # 查看可清理的内容
   ```

2. **清理旧数据**：
   ```bash
   # 清理临时文件
   cis system cleanup
   
   # 清理旧日志
   rm ~/.cis/logs/*.log.old
   
   # 压缩旧记忆
   cis memory vacuum
   ```

3. **迁移数据目录**：
   ```bash
   # 移动数据到更大的磁盘
   mv ~/.cis /new/disk/cis
   ln -s /new/disk/cis ~/.cis
   ```

### 问题：数据库损坏

**症状**：
```
Error: database disk image is malformed
```

**解决方案**：

1. **备份当前数据**：
   ```bash
   cp -r ~/.cis/data ~/.cis/data.backup.$(date +%Y%m%d)
   ```

2. **尝试修复**：
   ```bash
   # 对每个数据库文件
   sqlite3 ~/.cis/data/core.db ".recover" | sqlite3 ~/.cis/data/core_fixed.db
   mv ~/.cis/data/core.db ~/.cis/data/core.db.corrupt
   mv ~/.cis/data/core_fixed.db ~/.cis/data/core.db
   ```

3. **从备份恢复**：
   ```bash
   # 如果有定期备份
   cis system restore --from /path/to/backup
   ```

---

## 性能问题

### 问题：向量搜索缓慢

**症状**：语义搜索响应时间超过 1 秒

**解决方案**：

1. **检查索引**：
   ```bash
   # 检查向量索引状态
   sqlite3 ~/.cis/data/memory.db "SELECT name FROM sqlite_master WHERE type='index';"
   ```

2. **重建索引**：
   ```bash
   # 重新生成向量索引
   cis memory rebuild-index
   ```

3. **限制结果数量**：
   ```bash
   # 搜索时限制返回数量
   cis memory search "query" --limit 5
   ```

4. **检查内存使用**：
   ```bash
   # macOS
   vm_stat
   
   # Linux
   free -h
   ```

### 问题：CIS 进程占用过多内存

**症状**：RSS 内存超过 1GB

**解决方案**：

1. **检查内存使用**：
   ```bash
   ps aux | grep cis
   ```

2. **限制 WASM 运行时内存**：
   ```toml
   # 在 ~/.cis/config.toml 中添加
   [wasm]
   max_memory_mb = 512
   ```

3. **重启服务**：
   ```bash
   cis node stop
   cis node start
   ```

---

## AI 集成问题

### 问题：AI Provider 连接失败

**症状**：
```
Error: AI provider error: connection timeout
```

**解决方案**：

1. **检查 API 密钥**：
   ```bash
   # 确认配置文件中 API 密钥正确
   cat ~/.cis/config.toml | grep api_key
   ```

2. **测试网络连接**：
   ```bash
   # Claude
   curl https://api.anthropic.com/v1/health
   
   # OpenAI
   curl https://api.openai.com/v1/models -H "Authorization: Bearer $OPENAI_API_KEY"
   ```

3. **切换 Provider**：
   ```bash
   # 使用本地模型
   cis config set ai.default_provider ollama
   ```

### 问题：Skill 调用失败

**症状**：
```
Error: Skill execution failed: timeout
```

**解决方案**：

1. **检查 Skill 状态**：
   ```bash
   cis skill info <skill-name>
   ```

2. **查看日志**：
   ```bash
   tail -f ~/.cis/logs/skill.log
   ```

3. **重新加载 Skill**：
   ```bash
   cis skill unload <skill-name>
   cis skill load <skill-name>
   ```

---

## GUI 问题

### 问题：GUI 无法启动

**症状**：
```
Error: Failed to initialize graphics
```

**解决方案**：

1. **检查显示服务器**：
   ```bash
   # Linux - 确认正在运行 X11 或 Wayland
   echo $DISPLAY
   echo $WAYLAND_DISPLAY
   ```

2. **使用软件渲染**：
   ```bash
   export LIBGL_ALWAYS_SOFTWARE=1
   cis-gui
   ```

3. **更新显卡驱动**

### 问题：远程会话连接失败

**症状**：无法通过 WebSocket 连接到远程 CIS 节点

**解决方案**：

1. **检查远程节点状态**：
   ```bash
   # 在远程节点上
   cis node status
   ```

2. **验证 DID 白名单**：
   ```bash
   # 检查 ACL
   cis network list
   
   # 添加当前节点到白名单
   cis network allow <remote-did> --reason "Trusted node"
   ```

3. **检查端口连通性**：
   ```bash
   # 从本地测试
   nc -zv <remote-ip> 6767
   ```

---

## 日志和调试

### 启用详细日志

```bash
# 设置日志级别
export RUST_LOG=debug

# 特定模块
cis command

# 或永久设置
echo 'RUST_LOG=info' >> ~/.cis/config.env
```

### 查看日志文件

```bash
# 实时查看
tail -f ~/.cis/logs/cis.log

# 查看错误日志
tail -f ~/.cis/logs/cis.error.log

# 查看特定日期
less ~/.cis/logs/cis.log.2026-02-07
```

### 生成诊断报告

```bash
# 生成完整的诊断信息
cis system diagnose --output report.txt

# 包含在 Issue 中
cat report.txt | pbcopy  # macOS
cat report.txt | xclip -selection clipboard  # Linux
```

---

## 获取帮助

如果以上方案无法解决您的问题：

1. **查看文档**：
   - [使用指南](USAGE.md)
   - [架构文档](ARCHITECTURE.md)
   - [API 文档](../cis-core/src/lib.rs)

2. **搜索 Issues**：
   - [GitHub Issues](https://github.com/MoSiYuan/CIS/issues)

3. **提交新 Issue**：
   - 包含 `cis doctor` 输出
   - 包含相关日志片段
   - 描述复现步骤

---

**最后更新**: 2026-02-07
