# Memory Conflicts CLI 快速参考

> **版本**: v1.1.6
> **最后更新**: 2026-02-15

---

## 命令概览

```bash
cis memory conflicts <command>
```

### 可用命令

| 命令 | 简写 | 描述 | 示例 |
|-----|------|------|------|
| `list` | - | 列出所有未解决的冲突 | `cis memory conflicts list` |
| `resolve` | - | 解决指定的冲突 | `cis memory conflicts resolve -i <id> -c <choice>` |
| `detect` | - | 检测新的冲突 | `cis memory conflicts detect -k <keys>` |

---

## 详细使用

### 1. 列出冲突 (list)

```bash
cis memory conflicts list
```

**输出**:
```
🔍 检查未解决的冲突...

✅ 没有未解决的冲突

💡 提示:
   冲突检测会在多节点同步时自动触发
   使用 'cis memory conflicts detect <keys>' 手动检测指定键
```

**使用场景**:
- 定期检查系统健康状态
- P2P 同步后验证数据一致性
- 故障排查时查看冲突状态

---

### 2. 解决冲突 (resolve)

```bash
cis memory conflicts resolve --id <conflict-id> --choice <1-4>
```

**参数**:
- `--id` 或 `-i`: 冲突 ID（必需）
- `--choice` 或 `-c`: 解决选择（必需）

**解决选项**:

| 选项 | 名称 | 描述 | 适用场景 |
|-----|------|------|----------|
| `1` | KeepLocal | 保留本地版本 | 本地更改是正确的 |
| `2` | KeepRemote | 保留远程版本 | 远程更改更新/更准确 |
| `3` | KeepBoth | 保留两个版本 | 两个版本都需要保留 |
| `4` | AIMerge | AI 智能合并 | 需要合并两个版本的内容 |

**示例**:

```bash
# 保留本地版本
cis memory conflicts resolve -i conflict-abc-123 -c 1

# 保留远程版本
cis memory conflicts resolve --id conflict-def-456 --choice KeepRemote

# AI 合并（推荐）
cis memory conflicts resolve -i conflict-ghi-789 -c AIMerge
```

**输出**:
```
🔧 解决冲突: conflict-abc-123
✅ 已解决冲突: conflict-abc-123
   选择: 保留本地

⚠️  注意: 当前为演示模式，实际冲突解决需要完整的 ConflictGuard 集成
```

**使用场景**:
- 手动解决已知的冲突
- 选择正确的数据版本
- 合并多个节点的更改

---

### 3. 检测冲突 (detect)

```bash
cis memory conflicts detect --keys <keys>
```

**参数**:
- `--keys` 或 `-k`: 要检测的内存键（逗号分隔，必需）

**示例**:

```bash
# 检测单个键
cis memory conflicts detect -k user/preference/theme

# 检测多个键
cis memory conflicts detect -k key1,key2,key3

# 检测项目相关键
cis memory conflicts detect -k project/config,project/architecture

# 使用长格式
cis memory conflicts detect --keys user/settings,project/config
```

**输出**:
```
🔍 检测冲突: ["user/preference/theme", "project/config"]

✅ 未检测到新冲突

💡 提示:
   检测的键: ["user/preference/theme", "project/config"]
   在多节点环境中，冲突会在以下情况产生:
   - 同一键在不同节点被同时修改
   - 网络分区导致的数据不一致
   - 并发写入冲突
```

**使用场景**:
- 同步后验证关键数据
- 手动检查特定键的状态
- 批量验证多个键

---

## 常见工作流

### 工作流 1: 日常检查

```bash
# 1. 检查是否有冲突
cis memory conflicts list

# 2. 如果有冲突，查看详情
cis memory conflicts list

# 3. 解决冲突
cis memory conflicts resolve -i <conflict-id> -c <choice>
```

### 工作流 2: 同步后验证

```bash
# 1. 执行 P2P 同步
cis p2p sync

# 2. 检测关键键的冲突
cis memory conflicts detect -k \
  user/preference/theme,\
  project/config,\
  project/architecture

# 3. 如果发现冲突，解决它们
cis memory conflicts resolve -i <id> -c <choice>
```

### 工作流 3: 批量处理

```bash
# 1. 检测多个项目键
cis memory conflicts detect -k \
  project/config,\
  project/architecture,\
  project/api-contracts,\
  project/conventions

# 2. 列出所有冲突
cis memory conflicts list

# 3. 逐个解决
cis memory conflicts resolve -i conflict-1 -c 1
cis memory conflicts resolve -i conflict-2 -c 4
cis memory conflicts resolve -i conflict-3 -c 2
```

---

## 错误处理

### 无效的选择

```bash
$ cis memory conflicts resolve -i test-id -c 5
❌ 无效的选择: 5

有效选择:
  1 - KeepLocal (保留本地)
  2 - KeepRemote (保留远程)
  3 - KeepBoth (保留两个)
  4 - AIMerge (AI 合并)
```

### 缺少必需参数

```bash
$ cis memory conflicts resolve
error: the following required arguments were not provided:
  --id <conflict-id>
  --choice <choice>

Usage: cis memory conflicts resolve --id <conflict-id> --choice <choice>

For more information, try '--help'.
```

---

## 高级用法

### 结合脚本使用

```bash
#!/bin/bash
# 自动检查并解决冲突

# 检测冲突
cis memory conflicts detect -k project/config

# 如果有冲突，自动保留本地版本
CONFLICTS=$(cis memory conflicts list | grep "冲突")
if [ ! -z "$CONFLICTS" ]; then
    echo "发现冲突，自动解决..."
    # 这里可以添加自动解决逻辑
fi
```

### 监控和告警

```bash
#!/bin/bash
# 定期检查冲突并发送告警

while true; do
    RESULT=$(cis memory conflicts list)
    if echo "$RESULT" | grep -q "未解决的冲突"; then
        echo "警告: 发现未解决的冲突!"
        # 发送告警通知
    fi
    sleep 300  # 每 5 分钟检查一次
done
```

---

## 最佳实践

### 1. 定期检查

```bash
# 添加到 crontab
0 */6 * * * cis memory conflicts list
```

### 2. 同步后验证

```bash
# 始终在同步后检查冲突
cis p2p sync && cis memory conflicts detect -k project/config
```

### 3. 选择解决策略

- **KeepLocal**: 当你确定本地更改是正确的
- **KeepRemote**: 当远程版本更新或来自可信源
- **KeepBoth**: 当需要保留两个版本用于后续分析
- **AIMerge**: 当两个版本都需要合并，推荐使用 AI 智能合并

### 4. 批量处理

```bash
# 一次性检测所有关键键
cis memory conflicts detect -k \
  user/preference/theme,\
  user/preference/language,\
  project/config,\
  project/architecture,\
  project/conventions
```

---

## 故障排查

### 问题: 命令不识别

```bash
# 检查 CIS 版本
cis --version

# 确保使用 v1.1.6 或更高版本
```

### 问题: 检测不到冲突

```bash
# 确保键名正确
cis memory list

# 尝试检测不同的键
cis memory conflicts detect -k user/preference
```

### 问题: 解决后冲突仍然存在

```bash
# 重新检测
cis memory conflicts detect -k <keys>

# 查看详细日志
RUST_LOG=debug cis memory conflicts list
```

---

## 相关命令

| 命令 | 功能 | 相关性 |
|-----|------|--------|
| `cis memory list` | 列出所有内存键 | 🔗 查看键名 |
| `cis memory get` | 获取特定键的值 | 🔍 查看冲突内容 |
| `cis p2p sync` | P2P 同步 | ⚡ 可能触发冲突 |
| `cis memory status` | 内存系统状态 | 📊 系统健康检查 |

---

## 获取帮助

```bash
# 查看总体帮助
cis memory conflicts --help

# 查看子命令帮助
cis memory conflicts list --help
cis memory conflicts resolve --help
cis memory conflicts detect --help

# 查看 CIS 内存文档
cis memory --help
```

---

**提示**: 当前实现为演示模式，实际冲突检测和解决功能需要在完整 ConflictGuard 集成后才能使用。详见 [集成报告](./MEMORY_CONFLICTS_CLI_INTEGRATION.md)。
