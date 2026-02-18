# P1-2: 中英文注释统一任务指南

> **任务**: 将代码中的中文注释统一为英文
> **进度**: 3/304 文件已完成 (1%)
> **估计时间**: 2-3 天 (剩余)

---

## 已完成 ✅

| 文件 | 状态 | 提交 |
|------|------|------|
| memory/mod.rs | ✅ | be4d91f |
| memory/ops/mod.rs | ✅ | be4d91f |
| memory/ops/get.rs | ✅ | be4d91f |

---

## 剩余文件分类 (301 files)

### 核心模块 (优先级 🔴)

#### memory 模块剩余文件
- [ ] memory/ops/set.rs
- [ ] memory/ops/search.rs
- [ ] memory/ops/sync.rs
- [ ] memory/service.rs
- [ ] memory/encryption.rs
- [ ] memory/encryption_v2.rs
- [ ] memory/scope.rs
- [ ] memory/weekly_archived.rs
- [ ] memory/guard/*.rs

#### vector 模块
- [ ] vector/mod.rs
- [ ] vector/storage.rs
- [ ] vector/batch.rs
- [ ] vector/batch_loader.rs
- [ ] vector/switch.rs
- [ ] vector/merger.rs
- [ ] vector/adaptive_threshold.rs

#### scheduler 模块
- [ ] scheduler/mod.rs
- [ ] scheduler/dag_executor.rs
- [ ] scheduler/skill_executor*.rs
- [ ] scheduler/multi_agent_executor*.rs
- [ ] scheduler/core/*.rs
- [ ] scheduler/persistence/*.rs

#### storage 模块
- [ ] storage/mod.rs
- [ ] storage/memory_db.rs
- [ ] storage/sqlite_storage.rs
- [ ] storage/conversation_db.rs
- [ ] storage/federation_db.rs

#### p2p 模块
- [ ] p2p/mod.rs
- [ ] p2p/network.rs
- [ ] p2p/dht.rs
- [ ] p2p/kademlia/*.rs
- [ ] p2p/crypto/*.rs

### 次要模块 (优先级 🟡)

- [ ] agent/*.rs
- [ ] wasm/*.rs
- [ ] matrix/*.rs
- [ ] network/*.rs
- [ ] skill/*.rs

### 其他模块 (优先级 🟢)

- [ ] cli/*.rs
- [ ] config/*.rs
- [ ] event_bus/*.rs
- [ ] test/*.rs
- [ ] traits/*.rs

---

## 翻译流程

### 1. 识别中文注释

使用以下命令查找中文注释:
```bash
grep -n "^\/\/.*[\u4e00-\u9fff]\|^\/\/\/.*[\u4e00-\u9fff]" <file>
```

### 2. 翻译示例

#### ❌ 翻译前
```rust
/// 记忆服务模块
///
/// 提供私域/公域记忆管理，支持加密和访问控制。
/// 使用独立的 MemoryDb 存储，与核心数据库分离。
```

#### ✅ 翻译后
```rust
/// Memory service module
///
/// Provides private/public memory management with encryption and access control.
/// Uses independent MemoryDb storage, separated from the core database.
```

### 3. 翻译原则

1. **保持技术术语准确**
   - 记忆 → Memory (不是 Memory)
   - 域 → Domain
   - 命名空间 → Namespace
   - 向量 → Vector

2. **遵循 Rust 文档惯例**
   - 使用第三人称
   - 简洁明了
   - 包含参数和返回值说明

3. **保留代码不变**
   - 只翻译注释
   - 不修改代码逻辑
   - 不改变 API 命名

### 4. 常见术语对照

| 中文 | 英文 | 说明 |
|------|------|------|
| 记忆 | Memory | 核心概念 |
| 私域 | Private | 私有域 |
| 公域 | Public | 公共域 |
| 向量 | Vector | 数学概念 |
| 嵌入 | Embedding | AI 术语 |
| 加密 | Encryption | 安全术语 |
| 命名空间 | Namespace | 隔离机制 |
| 节点 | Node | 网络术语 |
| 同步 | Synchronization | P2P 术语 |
| 技能 | Skill | 能力单元 |
| DAG | DAG | 保持不变 |
| WASM | WASM | 保持不变 |

---

## 自动化辅助

### 方案 1: AI 辅助翻译

使用 AI 工具 (如 Claude, ChatGPT) 辅助翻译:

```bash
# 提取中文注释
grep -n "^\/\/.*[\u4e00-\u9fff]" <file> > /tmp/chinese_comments.txt

# 使用 AI 翻译 (人工审查后应用)
```

### 方案 2: 批量处理脚本

创建翻译脚本 (参考 `/tmp/translate_comments.py`):

```python
import re

def translate_file(filename):
    with open(filename, 'r', encoding='utf-8') as f:
        content = f.read()

    # 替换常见中文术语
    replacements = {
        "记忆": "memory",
        "私域": "private",
        "公域": "public",
        # ... 更多术语
    }

    for chinese, english in replacements.items():
        content = content.replace(chinese, english)

    with open(filename, 'w', encoding='utf-8') as f:
        f.write(content)
```

**⚠️ 注意**: 自动翻译必须人工审查，确保术语准确性和上下文正确。

---

## 质量检查清单

### 翻译后检查

- [ ] 所有中文注释已翻译
- [ ] 技术术语使用准确
- [ ] 英语语法正确
- [ ] 文档格式符合 Rust 惯例
- [ ] 代码编译通过
- [ ] 测试通过

### 验证命令

```bash
# 检查是否还有中文注释
grep -r "[\u4e00-\u9fff]" cis-core/src/ | grep "^.*\.rs.*//"

# 编译检查
cargo build --package cis-core

# 运行测试
cargo test --package cis-core
```

---

## 提交规范

### Commit Message 格式

```
i18n(P1-2): Translate <module> comments to English

## 已完成文件

- <file1>
- <file2>
- <file3>

## 翻译说明

<可选的翻译说明>

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

### 示例

```
i18n(P1-2): Translate vector module comments to English

## 已完成文件

- vector/mod.rs
- vector/storage.rs
- vector/batch.rs

## 翻译说明

- 保持了技术术语 "embedding" 和 "vector"
- 统一了 "search" vs "query" 的使用

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

---

## 进度追踪

### 每日目标

- **Day 1**: core modules (memory, vector, scheduler) - 100 files
- **Day 2**: infrastructure (storage, p2p, network) - 100 files
- **Day 3**: remaining modules - 101 files

### 进度百分比

```
[████░░░░░░░░░░░░░░] 1% (3/304 files)
```

---

## 下一步行动

### 立即行动

1. ✅ 已完成: memory/mod.rs, memory/ops/mod.rs, memory/ops/get.rs
2. ⏳ 进行中: 选择下一个模块 (建议 vector 或 scheduler)
3. 📋 待办: 处理剩余 301 个文件

### 建议优先级

1. **vector 模块** (核心功能，20+ files)
2. **scheduler 模块** (核心功能，30+ files)
3. **storage 模块** (基础设施，15+ files)
4. **p2p 模块** (网络功能，20+ files)

---

## 常见问题

### Q1: 是否必须翻译所有中文注释？

A: 是的。为了代码国际化和团队协作，所有注释应统一为英文。

### Q2: 翻译后如何保证质量？

A:
1. 使用 AI 辅助 + 人工审查
2. 技术术语保持一致性
3. 运行测试确保无破坏性变更

### Q3: 需要多长时间？

A: 估计 2-3 天:
- 自动化翻译: 4-6 小时
- 人工审查: 1-2 天
- 总计: ~48 工时

### Q4: 可以分批提交吗？

A: 可以。建议按模块分批提交，便于 review 和回滚。

---

## 参考资料

- [Rust 文档指南](https://doc.rust-lang.org/book/ch14-02-publishing-to-crates-io.html)
- [API 注释惯例](https://rust-lang.github.io/api-guidelines/documentation.html)
- [英语技术写作指南](https://developers.google.com/tech-writing)

---

**最后更新**: 2026-02-18
**负责人**: Development Team
**状态**: 进行中 (1%)
