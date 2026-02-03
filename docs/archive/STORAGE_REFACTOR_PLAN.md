# SQLite 存储架构重构计划

## 问题分析

### 当前实现（多库分离）
```
$CIS_DATA_DIR/
├── core/core.db              # 核心数据库
├── skills/data/
│   ├── im/data.db           # IM Skill 独立库
│   ├── ai-executor/data.db  # AI Executor 独立库
│   └── ...                  # 每个 Skill 一个库
```

**问题**:
- 与 MATRIX-final.md 的"单文件主权"理念冲突
- 无法有效利用 WAL 模式的全局优化
- Skill 间 JOIN 查询困难
- 备份需要复制多个文件

### 目标架构（主库 + ATTACH）
```
~/.cis/
├── node.db                   # 主数据库（Matrix、DID、公域记忆）
├── skills/
│   └── {skill_name}.db       # Skill 分离库（大数据量时 ATTACH）
└── wal/
    └── node.db-wal           # WAL 文件（随时关机安全）
```

**优势**:
- 单文件主权，便于迁移
- WAL 模式全局优化
- Skill 可通过 ATTACH 实现跨库 JOIN
- 统一备份策略

## 修改点清单

### 1. 存储配置层 (storage/config.rs)
- [ ] 定义 WAL 模式配置常量
- [ ] 实现 `set_wal_mode()` 函数
- [ ] 配置 `synchronous = NORMAL`
- [ ] 配置 `wal_autocheckpoint = 1000`
- [ ] 配置 `journal_size_limit = 100MB`

### 2. 统一存储管理器 (storage/unified.rs)
- [ ] 创建 `UnifiedStorage` 结构体
- [ ] 主库连接管理
- [ ] `attach_skill_db(skill_name)` 方法
- [ ] `detach_skill_db(skill_name)` 方法
- [ ] 跨库查询支持

### 3. 数据库 Schema 调整
- [ ] 合并 core.db 和 skill 数据库到 node.db
- [ ] 使用表前缀区分：`matrix_*`, `skill_*`, `cis_*`
- [ ] 添加 `federate` 字段到 matrix_events

### 4. 热插拔支持改造
- [ ] Skill 加载时 ATTACH 其数据库
- [ ] Skill 卸载时 DETACH
- [ ] 保持向后兼容（支持独立库模式）

### 5. 关机安全机制
- [ ] 启动时检查 WAL 并恢复
- [ ] 优雅关机时 checkpoint
- [ ] 定期自动 checkpoint

## 并发执行计划

### 任务 A: WAL 模式配置
负责：storage/config.rs + 基础 WAL 支持

### 任务 B: 统一存储管理器
负责：storage/unified.rs + 主库连接池

### 任务 C: Schema 迁移
负责：database schema 设计 + 迁移脚本

### 任务 D: Skill 热插拔改造
负责：skill/manager.rs + attach/detach 逻辑
