# 个人知识管理示例

这个示例展示如何使用 CIS 构建个人知识管理系统，包括笔记整理、智能搜索和跨设备同步。

## 功能特性

- 📚 自动整理笔记和文档
- 🔍 语义搜索（基于 sqlite-vec）
- 🏷️ 自动标签生成
- 📱 跨设备同步
- 🔗 知识图谱构建

## 目录结构

```
personal-knowledge/
├── README.md
├── config.toml          # CIS 配置
├── dags/               # DAG 工作流
│   ├── daily-sync.dag.toml
│   ├── note-organize.dag.toml
│   └── knowledge-graph.dag.toml
├── notes/              # 笔记存储
│   ├── inbox/         # 收件箱
│   ├── archive/       # 归档
│   └── tags/          # 标签
└── templates/          # 笔记模板
    ├── daily.md
    ├── project.md
    └── meeting.md
```

## 快速开始

### 1. 初始化项目

```bash
# 进入示例目录
cd examples/personal-knowledge

# 初始化 CIS
cis init

# 复制配置
cp config.example.toml ~/.cis/config.toml
# 编辑配置，设置 AI Provider
```

### 2. 配置 AI Provider

编辑 `~/.cis/config.toml`：

```toml
[ai]
provider = "kimi"
api_key = "your-api-key"
```

### 3. 启动 CIS

```bash
cis node start
```

### 4. 运行示例 DAG

```bash
# 每日同步
cis dag run daily-sync

# 整理笔记
cis dag run note-organize

# 构建知识图谱
cis dag run knowledge-graph
```

## DAG 说明

### daily-sync.dag.toml

每日自动同步和整理任务：

```toml
[dag]
name = "daily-sync"
description = "每日笔记同步和整理"
schedule = "0 9 * * *"  # 每天上午 9 点

[step.collect]
command = "cis skill do '收集所有设备的新笔记'"

[step.organize]
command = "cis skill do '整理收件箱笔记，生成标签'"
depends_on = ["collect"]

[step.sync]
command = "cis network sync"
depends_on = ["organize"]
```

### note-organize.dag.toml

智能笔记整理：

```toml
[dag]
name = "note-organize"
description = "智能整理笔记"

[step.extract-tags]
command = "cis skill do '分析笔记内容，提取关键词作为标签'"

[step.categorize]
command = "cis skill do '根据标签自动分类笔记'"
depends_on = ["extract-tags"]

[step.update-index]
command = "cis memory index"
depends_on = ["categorize"]
```

### knowledge-graph.dag.toml

构建知识图谱：

```toml
[dag]
name = "knowledge-graph"
description = "构建知识图谱"

[step.analyze-links]
command = "cis skill do '分析笔记之间的关联'"

[step.generate-graph]
command = "cis skill do '生成知识图谱数据'"
depends_on = ["analyze-links"]

[step.export]
command = "cis skill do '导出图谱到 notes/graph.md'"
depends_on = ["generate-graph"]
```

## 使用方法

### 添加笔记

```bash
# 创建新笔记
cis skill do "创建笔记：关于 Rust 所有权系统的学习"

# 或手动创建
echo "# Rust 所有权" > notes/inbox/rust-ownership.md
cis dag run note-organize
```

### 搜索笔记

```bash
# 语义搜索
cis memory search "Rust 内存管理"

# 标签搜索
cis memory search "tag:rust"
```

### 查看知识图谱

```bash
# 生成并查看图谱
cis dag run knowledge-graph
cat notes/graph.md
```

## 跨设备同步

### 配置多设备

1. 在所有设备上安装 CIS
2. 使用相同的助记词恢复（生成相同 DID）
3. 添加设备到白名单

```bash
# 设备 A
cis network allow did:cis:device-b --reason "笔记本"

# 设备 B
cis network allow did:cis:device-a --reason "工作站"
```

### 自动同步

DAG 自动处理同步：

```toml
[step.sync]
command = "cis network sync --strategy merge"
```

## 自定义扩展

### 添加新的 Skill

```rust
// skills/note-processor/src/lib.rs
use cis_skill_sdk::prelude::*;

#[skill]
fn process_note(input: NoteInput) -> Result<NoteOutput> {
    // 自定义处理逻辑
}
```

### 自定义模板

在 `templates/` 目录创建 Markdown 模板：

```markdown
---
title: {{title}}
date: {{date}}
tags: {{tags}}
---

# {{title}}

## 概述

{{summary}}

## 内容

{{content}}

## 相关

{{related_notes}}
```

## 注意事项

1. **隐私**: 所有笔记存储在本地，不会上传到云端
2. **备份**: 定期备份 `~/.cis/data` 目录
3. **加密**: 数据库使用 ChaCha20-Poly1305 加密

## 参考

- [CIS 文档](../../docs/README.md)
- [记忆管理](../../docs/memory-management.md)
- [网络同步](../../docs/network-sync.md)
