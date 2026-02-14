# 记忆作用域隔离设计（User + Group + Path 完整版）

> **版本**: v1.1.7
> **创建日期**: 2026-02-13
> **更新日期**: 2026-02-14（添加 User + Group 维度）
> **关联**: [MEMORY_SOURCE_TRUST_DESIGN.md](./MEMORY_SOURCE_TRUST_DESIGN.md)

---

## 设计方案：路径字符串（模仿 Linux 权限）

### 核心思想

**v1.1.7 完整设计**：使用 **User + Group + Path** 三维路径表示作用域，完全模仿 Linux 文件系统权限。

```
v1.1.6 方案（不完整）：
❌ /                           → 全局作用域
❌ /project-A                 → 项目作用域
❌ /project-A/module-db        → 模块作用域
❌ /project-A/task-123       → 任务作用域

v1.1.7 完整方案（User + Group + Path）：
✅ /                                      → 全局作用域（系统级）
✅ /user-alice                            → 用户作用域（个人级）
✅ /user-alice/team-dev                   → 组作用域（团队级）
✅ /user-alice/team-dev/project-A         → 项目作用域（项目级）
✅ /user-alice/team-dev/project-A/module-db → 模块作用域（模块级）
✅ /user-alice/team-dev/project-A/task-123  → 任务作用域（任务级）
✅ /user-alice/.session-456                → 会话作用域（临时隔离，隐藏）
```

### Git Worktree 兼容性

**关键优势**：路径结构与 git worktree 完美对应

```
~/.cis/
├── user-alice/                      # 用户作用域
│   ├── team-dev/                    # 组作用域
│   │   ├── project-A/              # 项目作用域
│   │   │   ├── module-db/          # 模块作用域
│   │   │   ├── task-123/           # 任务作用域
│   │   │   └── .session-456/       # 会话作用域（临时）
│   │   └── project-B/
│   └── team-design/
└── user-bob/
    └── team-dev/
        └── project-A/              # Bob 的项目 A 视图（独立隔离）
```

**示例场景**：
```bash
# Alice 在 team-dev 组的 project-A 中工作
cd ~/.cis/user-alice/team-dev/project-A
cis memory set "language" "Rust" --scope auto
# 存储到: /user-alice/team-dev/project-A

# Bob 同时在 team-dev 组的 project-A 中工作
cd ~/.cis/user-bob/team-dev/project-A
cis memory get "language"
# 返回 None（Bob 的记忆完全隔离）

# 团队共享记忆（组作用域）
cis memory set "team-conventions" "Follow Rust API guidelines" \
  --scope /user-alice/team-dev
# 任何 team-dev 成员都能访问
```

### 三维路径结构（v1.1.7）

```
完整路径格式：
/{user}/{group}/{project}/{module|task|session}

示例：
/                                           → 系统全局（所有用户）
/user-alice                                → Alice 的个人空间
/user-alice/team-dev                        → Alice 的 dev 团队
/user-alice/team-dev/project-A             → dev 团队的 project-A
/user-alice/team-dev/project-A/module-db   → project-A 的 db 模块
/user-alice/team-dev/project-A/task-123    → project-A 的 task-123
/user-alice/.session-456                   → Alice 的临时会话（无团队）
```

### 权限继承规则

```
1. User 维度：完全隔离
   - /user-alice/* ≠ /user-bob/*
   - 不同用户的记忆无法互相访问

2. Group 维度：团队共享
   - /user-alice/team-dev/* = /user-bob/team-dev/*
   - 同一团队成员可共享组级记忆

3. Project 维度：项目隔离
   - /user-alice/team-dev/project-A ≠ /user-alice/team-dev/project-B
   - 同一团队不同项目完全隔离

4. Module/Task 维度：模块/任务隔离
   - /user-alice/team-dev/project-A/module-db ≠ /user-alice/team-dev/project-A/module-api
   - 同一项目不同模块完全隔离
```

### Git Worktree 映射

```
~/.cis/                                   → CIS 根目录
├── user-alice/                            → 用户 Alice 的根
│   ├── team-dev/                          → dev 团队共享空间
│   │   ├── project-A/                    → 项目 A（可 git worktree）
│   │   │   ├── .cis/                    → 项目配置
│   │   │   │   ├── memory.db            → 项目级记忆数据库
│   │   │   │   ├── skills/              → 项目本地 Skills
│   │   │   │   └── dags/                → 项目 DAGs
│   │   │   ├── module-db/               → 数据库模块（子 worktree）
│   │   │   ├── module-api/              → API 模块（子 worktree）
│   │   │   └── task-123/               → 任务 123（临时 worktree）
│   │   └── project-B/
│   └── team-design/                     → design 团队空间
└── user-bob/                            → 用户 Bob 的根（完全隔离）
    └── team-dev/                        → Bob 的 dev 团队视图
        └── project-A/                   → Bob 的项目 A 视图
```

### 优势对比

| 特性 | 枚举方案 | 路径方案 | User+Group+Path 方案 |
|------|---------|---------|---------------------|
| **简单性** | ❌ 复杂（多个枚举变体） | ✅ 简单（字符串） | ✅✅ 最简单（统一路径） |
| **直观性** | ❌ 需要理解枚举层级 | ✅ 路径结构（开发者熟悉） | ✅✅ 完全对应文件系统 |
| **数据库查询** | ❌ 多字段组合查询 | ✅ LIKE 前缀匹配 | ✅✅ LIKE 前缀匹配 |
| **层级判断** | ❌ match 分支 | ✅ 路径深度（/ 数量） | ✅✅ 路径深度（/ 数量） |
| **扩展性** | ❌ 需修改枚举 | ✅ 任意添加路径层级 | ✅✅ 任意添加层级 |
| **用户隔离** | ❌ 不支持 | ❌ 不支持 | ✅✅ User 级别隔离 |
| **团队协作** | ❌ 不支持 | ❌ 不支持 | ✅✅ Group 级别共享 |
| **Git 兼容** | ❌ 无法映射 | ⚠️ 部分映射 | ✅✅ 完美映射 worktree |

---

## 实现设计

### 1. 路径定义规则（v1.1.7 完整版）

```rust
/// 记忆作用域（User + Group + Path 三维路径字符串）
///
/// # 规则
/// - `/` - 系统全局作用域（所有用户、组、项目）
/// - `/{user}` - 用户作用域（个人级隔离）
/// - `/{user}/{group}` - 组作用域（团队级共享）
/// - `/{user}/{group}/{project}` - 项目作用域（项目级隔离）
/// - `/{user}/{group}/{project}/module/{mid}` - 模块作用域
/// - `/{user}/{group}/{project}/task/{tid}` - 任务作用域
/// - `/{user}/.session/{sid}` - 用户级会话作用域（隐藏，临时）
///
/// # 优先级
/// - 路径越长（层级越深），优先级越高
/// - 同一路径，最新覆盖旧值
/// - User > Group > Project > Module > Task > Session
///
/// # Git Worktree 兼容性
/// - 每一层级都可以映射到独立的 git worktree
/// - 示例：`~/.cis/user-alice/team-dev/project-A/module-db/`
///
/// # 示例
/// ```rust
/// use cis_core::memory::MemoryScope;
///
/// // 系统全局作用域
/// let global = MemoryScope::global();
/// assert_eq!(global.as_str(), "/");
///
/// // 用户作用域
/// let alice = MemoryScope::user("alice");
/// assert_eq!(alice.as_str(), "/user-alice");
///
/// // 组作用域
/// let team_dev = MemoryScope::group("alice", "team-dev");
/// assert_eq!(team_dev.as_str(), "/user-alice/team-dev");
///
/// // 项目作用域
/// let project_a = MemoryScope::project("alice", "team-dev", "project-a");
/// assert_eq!(project_a.as_str(), "/user-alice/team-dev/project-a");
///
/// // 模块作用域
/// let module_db = MemoryScope::module("alice", "team-dev", "project-a", "database");
/// assert_eq!(module_db.as_str(), "/user-alice/team-dev/project-a/module-database");
///
/// // 任务作用域
/// let task_123 = MemoryScope::task("alice", "team-dev", "project-a", "task-123");
/// assert_eq!(task_123.as_str(), "/user-alice/team-dev/project-a/task-task-123");
///
/// // 会话作用域（隐藏，临时）
/// let session_456 = MemoryScope::session("alice", "session-456");
/// assert_eq!(session_456.as_str(), "/user-alice/.session-session-456");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScope(pub String);

impl MemoryScope {
    /// 系统全局作用域
    pub fn global() -> Self {
        Self("/".to_string())
    }

    /// 用户作用域（个人级隔离）
    pub fn user(user_id: &str) -> Self {
        Self(format!("/user-{}", user_id))
    }

    /// 组作用域（团队级共享）
    pub fn group(user_id: &str, group_id: &str) -> Self {
        Self(format!("/user-{}/{}", user_id, group_id))
    }

    /// 项目作用域
    pub fn project(user_id: &str, group_id: &str, project_id: &str) -> Self {
        Self(format!("/user-{}/{}/{}", user_id, group_id, project_id))
    }

    /// 模块作用域
    pub fn module(user_id: &str, group_id: &str, project_id: &str, module_id: &str) -> Self {
        Self(format!("/user-{}/{}/{}/module-{}", user_id, group_id, project_id, module_id))
    }

    /// 任务作用域
    pub fn task(user_id: &str, group_id: &str, project_id: &str, task_id: &str) -> Self {
        Self(format!("/user-{}/{}/{}/task-{}", user_id, group_id, project_id, task_id))
    }

    /// 会话作用域（隐藏，临时）
    pub fn session(user_id: &str, session_id: &str) -> Self {
        Self(format!("/user-{}/.session-{}", user_id, session_id))
    }

    /// 获取字符串引用
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 是否是系统全局作用域
    pub fn is_global(&self) -> bool {
        self.0 == "/"
    }

    /// 是否是用户作用域
    pub fn is_user(&self) -> bool {
        let parts: Vec<&str> = self.split('/').collect();
        parts.len() == 2 && parts[0].is_empty() && parts[1].starts_with("user-")
    }

    /// 是否是组作用域
    pub fn is_group(&self) -> bool {
        let parts: Vec<&str> = self.split('/').collect();
        parts.len() == 3 && parts[0].is_empty() && parts[1].starts_with("user-")
    }

    /// 是否是会话作用域（隐藏）
    pub fn is_session(&self) -> bool {
        self.0.contains("/.session-")
    }

    /// 获取用户 ID（如果有）
    pub fn user_id(&self) -> Option<&str> {
        if self.is_global() {
            return None;
        }

        // 解析 /user-{user_id}/...
        let parts: Vec<&str> = self.split('/').collect();
        if parts.len() >= 2 && parts[0].is_empty() && parts[1].starts_with("user-") {
            Some(&parts[1][5..])  // 去掉 "user-" 前缀
        } else {
            None
        }
    }

    /// 获取组 ID（如果有）
    pub fn group_id(&self) -> Option<&str> {
        // 解析 /user-{user_id}/{group_id}/...
        let parts: Vec<&str> = self.split('/').collect();
        if parts.len() >= 3 && parts[0].is_empty() && parts[1].starts_with("user-") {
            Some(parts[2])
        } else {
            None
        }
    }

    /// 获取项目 ID（如果有）
    pub fn project_id(&self) -> Option<&str> {
        // 解析 /user-{user_id}/{group_id}/{project_id}/...
        let parts: Vec<&str> = self.split('/').collect();
        if parts.len() >= 4 && parts[0].is_empty() && parts[1].starts_with("user-") {
            Some(parts[3])
        } else {
            None
        }
    }

    /// 获取模块 ID（如果有）
    pub fn module_id(&self) -> Option<&str> {
        // 解析 .../module-{module_id}
        if let Some(idx) = self.0.find("/module-") {
            let remaining = &self.0[idx + 7..];  // 跳过 "/module-"
            if let Some(end_idx) = remaining.find('/') {
                Some(&remaining[..end_idx])
            } else {
                Some(remaining)
            }
        } else {
            None
        }
    }

    /// 获取任务 ID（如果有）
    pub fn task_id(&self) -> Option<&str> {
        // 解析 .../task-{task_id}
        if let Some(idx) = self.0.find("/task-") {
            let remaining = &self.0[idx + 6..];  // 跳过 "/task-"
            if let Some(end_idx) = remaining.find('/') {
                Some(&remaining[..end_idx])
            } else {
                Some(remaining)
            }
        } else {
            None
        }
    }

    /// 获取层级（0=全局, 1=用户, 2=组, 3=项目, 4=模块/任务, 5=会话）
    pub fn level(&self) -> usize {
        self.split('/').count() - 1
    }

    /// 获取父级作用域
    pub fn parent(&self) -> Option<Self> {
        if self.is_global() {
            return None;  // 全局无父级
        }

        let path = std::path::Path::new(self.0);
        path.parent().map(|p| Self(p.to_string_lossy().to_string()))
    }

    /// 获取所有父级作用域（从近到远）
    pub fn parents(&self) -> Vec<Self> {
        if self.is_global() {
            return vec![];
        }

        let mut parents = vec![];
        let mut current = self.clone();
        while let Some(p) = current.parent() {
            parents.push(p.clone());
            current = p;
        }
        parents
    }

    /// 判断是否是另一个作用域的子级
    pub fn is_child_of(&self, other: &Self) -> bool {
        if self.is_global() {
            return false;
        }

        self.0.starts_with(&other.0) && self != other
    }

    /// 判断是否是另一个作用域的父级
    pub fn is_parent_of(&self, other: &Self) -> bool {
        other.is_child_of(self)
    }

    /// 转换为文件系统路径（用于 git worktree 映射）
    pub fn to_path_buf(&self, base: &std::path::Path) -> std::path::PathBuf {
        let mut path = base.to_path_buf();
        for component in self.0.split('/').filter(|s| !s.is_empty()) {
            path.push(component);
        }
        path
    }
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::global()
    }
}

impl AsRef<str> for MemoryScope {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// 用于 SQL 查询的辅助方法
impl MemoryScope {
    /// SQL LIKE 模式（用于前缀查询）
    pub fn like_pattern(&self) -> String {
        if self.is_global() {
            "/%".to_string()  // 全局：所有路径
        } else {
            format!("{}%", self.0)  // 前缀匹配
        }
    }

    /// 路径分隔符（用于 split 查询）
    pub fn separator() -> char {
        '/'
    }

    /// 转义路径中的特殊字符（用于 SQL LIKE）
    pub fn escape_like(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('_', "\\_")
            .replace('%', "\\%")
    }
}
```

---

## 问题背景

### 场景 1：跨项目记忆污染

```rust
// 项目 A 中
service.set_user_forced(
    "project-A/language",
    b"Use Rust for development",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Project { id: "project-A" },  // 🔥 项目作用域
).await?;

// 项目 B 中
service.set_user_forced(
    "project-B/language",
    b"Use Python for development",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Project { id: "project-B" },  // 🔥 项目作用域
).await?;

// ❌ 如果没有作用域隔离：
// 搜索"开发语言偏好"会同时返回两个 UserForced 记忆
// 导致 Agent 困惑："到底是 Rust 还是 Python？"
```

### 场景 2：全局 vs 项目级配置冲突

```rust
// 全局配置（用户默认偏好）
service.set_user_input(
    "global/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Global,  // 🔥 全局作用域
).await?;

// 项目特定配置（覆盖全局）
service.set_user_forced(
    "project-A/theme",
    b"light",  // 🔥 项目 A 强制使用浅色主题
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Project { id: "project-A" },
).await?;

// ✅ 作用域隔离：
// - 项目 A 中查询 → 返回 light（Project scope，优先级高）
// - 其他项目中查询 → 返回 dark（Global scope）
```

### 场景 3：模块间记忆隔离

```rust
// 模块 A：数据库模块
service.set_user_forced(
    "module-db/connection-pool",
    b"max_connections=100",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    MemoryScope::Module {
        project_id: "project-A",
        module_id: "database",
    },
).await?;

// 模块 B：API 模块
service.set_user_forced(
    "module-api/connection-pool",
    b"max_connections=50",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    MemoryScope::Module {
        project_id: "project-A",
        module_id: "api",
    },
).await?;

// ✅ 模块隔离：
// - 模块 A 查询 → 只看到 module-db 的配置
// - 模块 B 查询 → 只看到 module-api 的配置
// - 互不干扰
```

---

## 设计方案

### 1. 引入 MemoryScope 枚举

```rust
/// 记忆作用域（隔离维度）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum MemoryScope {
    /// 全局作用域（所有项目/模块共享）
    Global,

    /// 项目作用域（项目级别隔离）
    Project {
        id: String,
    },

    /// 模块作用域（模块级别隔离）
    Module {
        project_id: String,
        module_id: String,
    },

    /// 任务作用域（任务级别隔离）
    Task {
        project_id: String,
        task_id: String,
    },

    /// 会话作用域（临时隔离）
    Session {
        project_id: String,
        session_id: String,
    },
}

impl Default for MemoryScope {
    fn default() -> Self {
        Self::Global  // 默认全局作用域
    }
}

impl MemoryScope {
    /// 作用域层级（数值越小优先级越高）
    pub fn level(&self) -> usize {
        match self {
            Self::Session { .. } => 0,      // 最高优先级（会话级）
            Self::Task { .. } => 1,         // 任务级
            Self::Module { .. } => 2,        // 模块级
            Self::Project { .. } => 3,       // 项目级
            Self::Global => 4,              // 最低优先级（全局）
        }
    }

    /// 是否是全局作用域
    pub fn is_global(&self) -> bool {
        matches!(self, Self::Global)
    }

    /// 获取作用域 ID（用于查询过滤）
    pub fn scope_id(&self) -> String {
        match self {
            Self::Global => "global".to_string(),
            Self::Project { id } => format!("project/{}", id),
            Self::Module { project_id, module_id } => {
                format!("project/{}/module/{}", project_id, module_id)
            }
            Self::Task { project_id, task_id } => {
                format!("project/{}/task/{}", project_id, task_id)
            }
            Self::Session { project_id, session_id } => {
                format!("project/{}/session/{}", project_id, session_id)
            }
        }
    }
}
```

### 2. 扩展 MemoryEntry 结构

```rust
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: i64,
    pub updated_at: i64,

    // 来源可信度
    pub source: MemorySource,
    pub confidence: f32,
    pub vector_indexed: bool,
    pub access_count: i64,
    pub parent_key: Option<String>,
    pub confirmed_by_user: bool,

    // 🔥 新增：作用域隔离
    pub scope: MemoryScope,
}

impl MemoryEntry {
    /// 检查记忆是否属于某个作用域
    pub fn belongs_to_scope(&self, scope: &MemoryScope) -> bool {
        match (&self.scope, scope) {
            // 全局作用域：所有记忆都匹配
            (MemoryScope::Global, _) | (_, MemoryScope::Global) => true,

            // 项目作用域
            (MemoryScope::Project { id: a_id }, MemoryScope::Project { id: b_id }) => {
                a_id == b_id
            }

            // 模块作用域
            (MemoryScope::Module { project_id: pa, module_id: ma },
             MemoryScope::Module { project_id: pb, module_id: mb }) => {
                pa == pb && ma == mb
            }

            // 任务作用域
            (MemoryScope::Task { project_id: pa, task_id: ta },
             MemoryScope::Task { project_id: pb, task_id: tb }) => {
                pa == pb && ta == tb
            }

            // 会话作用域
            (MemoryScope::Session { project_id: pa, session_id: sa },
             MemoryScope::Session { project_id: pb, session_id: sb }) => {
                pa == pb && sa == sb
            }

            // 不同级别：检查包含关系
            (MemoryScope::Session { .. }, _) |
            (_, MemoryScope::Session { .. }) => {
                // 会话级别最严格，不同会话不共享
                false
            }

            (MemoryScope::Task { .. }, MemoryScope::Project { .. }) |
            (MemoryScope::Project { .. }, MemoryScope::Task { .. }) => {
                // 任务属于项目，可以访问
                self.scope.project_id() == scope.project_id()
            }

            (MemoryScope::Module { .. }, MemoryScope::Project { .. }) |
            (MemoryScope::Project { .. }, MemoryScope::Module { .. }) => {
                // 模块属于项目，可以访问
                self.scope.project_id() == scope.project_id()
            }

            _ => false,
        }
    }

    /// 获取项目 ID（如果有）
    pub fn project_id(&self) -> Option<&str> {
        match &self.scope {
            MemoryScope::Global => None,
            MemoryScope::Project { id } => Some(id),
            MemoryScope::Module { project_id, .. } => Some(project_id),
            MemoryScope::Task { project_id, .. } => Some(project_id),
            MemoryScope::Session { project_id, .. } => Some(project_id),
        }
    }
}

impl MemoryScope {
    /// 获取项目 ID（辅助方法）
    fn project_id(&self) -> Option<&str> {
        match self {
            Self::Global => None,
            Self::Project { id } => Some(id),
            Self::Module { project_id, .. } => Some(project_id),
            Self::Task { project_id, .. } => Some(project_id),
            Self::Session { project_id, .. } => Some(project_id),
        }
    }
}
```

### 2. 数据库 Schema 更新（v1.1.7 完整版）

```sql
-- ================================================================
-- v1.1.7: User + Group + Path 三维作用域 Schema
-- ================================================================

-- memory_entries 表增加 User + Group + Path 字段
ALTER TABLE private_entries ADD COLUMN scope_path TEXT NOT NULL DEFAULT '/';
ALTER TABLE private_entries ADD COLUMN scope_user_id TEXT;
ALTER TABLE private_entries ADD COLUMN scope_group_id TEXT;
ALTER TABLE private_entries ADD COLUMN scope_project_id TEXT;
ALTER TABLE private_entries ADD COLUMN scope_component_type TEXT;  -- 'module', 'task', 'session'
ALTER TABLE private_entries ADD COLUMN scope_component_id TEXT;

ALTER TABLE public_entries ADD COLUMN scope_path TEXT NOT NULL DEFAULT '/';
ALTER TABLE public_entries ADD COLUMN scope_user_id TEXT;
ALTER TABLE public_entries ADD COLUMN scope_group_id TEXT;
ALTER TABLE public_entries ADD COLUMN scope_project_id TEXT;
ALTER TABLE public_entries ADD COLUMN scope_component_type TEXT;
ALTER TABLE public_entries ADD COLUMN scope_component_id TEXT;

-- 创建作用域路径索引（用于前缀查询）
CREATE INDEX IF NOT EXISTS idx_private_scope_path
    ON private_entries(scope_path);

CREATE INDEX IF NOT EXISTS idx_public_scope_path
    ON public_entries(scope_path);

-- 创建用户索引（用于用户级隔离）
CREATE INDEX IF NOT EXISTS idx_private_scope_user
    ON private_entries(scope_user_id);

CREATE INDEX IF NOT EXISTS idx_public_scope_user
    ON public_entries(scope_user_id);

-- 创建组索引（用于团队级共享）
CREATE INDEX IF NOT EXISTS idx_private_scope_group
    ON private_entries(scope_user_id, scope_group_id);

CREATE INDEX IF NOT EXISTS idx_public_scope_group
    ON public_entries(scope_user_id, scope_group_id);

-- 创建项目索引（用于项目级隔离）
CREATE INDEX IF NOT EXISTS idx_private_scope_project
    ON private_entries(scope_user_id, scope_group_id, scope_project_id);

CREATE INDEX IF NOT EXISTS idx_public_scope_project
    ON public_entries(scope_user_id, scope_group_id, scope_project_id);

-- 创建复合索引（作用域 + 可信度）
CREATE INDEX IF NOT EXISTS idx_private_scope_confidence
    ON private_entries(scope_path, confidence);

CREATE INDEX IF NOT EXISTS idx_public_scope_confidence
    ON public_entries(scope_path, confidence);

-- 创建复合索引（用户 + 可信度）
CREATE INDEX IF NOT EXISTS idx_private_user_confidence
    ON private_entries(scope_user_id, confidence);

CREATE INDEX IF NOT EXISTS idx_public_user_confidence
    ON public_entries(scope_user_id, confidence);

-- 创建复合索引（组 + 可信度）
CREATE INDEX IF NOT EXISTS idx_private_group_confidence
    ON private_entries(scope_user_id, scope_group_id, confidence);

CREATE INDEX IF NOT EXISTS idx_public_group_confidence
    ON public_entries(scope_user_id, scope_group_id, confidence);
```

### 3. 路径结构示例（v1.1.7）

```sql
-- ================================================================
-- 记忆路径示例
-- ================================================================

-- 系统全局记忆
INSERT INTO private_entries (key, value, scope_path, scope_user_id, confidence)
VALUES ('system/version', 'v1.1.7', '/', NULL, 1.0);

-- 用户级记忆
INSERT INTO private_entries (key, value, scope_path, scope_user_id, confidence)
VALUES ('user/theme', 'dark', '/user-alice', 'alice', 0.8);

-- 组级记忆（团队共享）
INSERT INTO public_entries (key, value, scope_path, scope_user_id, scope_group_id, confidence, federate)
VALUES ('team/conventions', 'Follow Rust API guidelines', '/user-alice/team-dev', 'alice', 'team-dev', 0.8, 1);

-- 项目级记忆
INSERT INTO public_entries (key, value, scope_path, scope_user_id, scope_group_id, scope_project_id, confidence, federate)
VALUES ('project/architecture', 'Microservices with Rust', '/user-alice/team-dev/project-a', 'alice', 'team-dev', 'project-a', 1.0, 1);

-- 模块级记忆
INSERT INTO private_entries (key, value, scope_path, scope_user_id, scope_group_id, scope_project_id, scope_component_type, scope_component_id, confidence)
VALUES ('module/connection-pool', 'max_connections=100', '/user-alice/team-dev/project-a/module-database', 'alice', 'team-dev', 'project-a', 'module', 'database', 1.0);

-- 任务级记忆
INSERT INTO private_entries (key, value, scope_path, scope_user_id, scope_group_id, scope_project_id, scope_component_type, scope_component_id, confidence)
VALUES ('task/status', 'in_progress', '/user-alice/team-dev/project-a/task-123', 'alice', 'team-dev', 'project-a', 'task', '123', 0.8);

-- 会话级记忆（临时）
INSERT INTO private_entries (key, value, scope_path, scope_user_id, confidence)
VALUES ('session/temp-var', '42', '/user-alice/.session-456', 'alice', 0.5);
```

### 4. Git Worktree 目录结构映射

```bash
# ================================================================
# CIS 记忆目录结构（对应 git worktree）
# ================================================================

~/.cis/                                    # CIS 根目录
├── user-alice/                             # 用户 Alice 的根
│   ├── .cis/                               # 用户配置
│   │   ├── memory.db                        # 用户级数据库
│   │   └── user.toml                       # 用户配置文件
│   ├── team-dev/                           # dev 团队共享空间
│   │   ├── .cis/                          # 组配置
│   │   │   ├── memory.db                   # 组级数据库（共享）
│   │   │   └── team.toml                  # 组配置文件
│   │   ├── project-a/                      # 项目 A
│   │   │   ├── .cis/                      # 项目配置
│   │   │   │   ├── memory.db              # 项目级数据库
│   │   │   │   ├── project.toml           # 项目配置文件
│   │   │   │   ├── skills/                # 项目本地 Skills
│   │   │   │   └── dags/                  # 项目 DAGs
│   │   │   ├── src/                       # 项目源代码（git worktree）
│   │   │   ├── module-database/           # 数据库模块（子 worktree）
│   │   │   ├── module-api/                # API 模块（子 worktree）
│   │   │   └── task-123/                 # 任务 123（临时 worktree）
│   │   │       └── .cis/
│   │   │           └── memory.db          # 任务级数据库（临时）
│   │   └── project-b/
│   └── team-design/                       # design 团队空间
└── user-bob/                              # 用户 Bob 的根（完全隔离）
    └── team-dev/                          # Bob 的 dev 团队视图
        └── project-a/                     # Bob 的项目 A 视图
            └── .cis/
                └── memory.db              # Bob 的项目 A 记忆（独立）
```

---

## 完整实现

### 1.1 数据库 Schema（简化版）

```sql
-- memory_entries 表（v1.1.7 简化版）
ALTER TABLE memory_entries ADD COLUMN scope TEXT NOT NULL DEFAULT '/';

-- 创建索引（路径前缀匹配）
CREATE INDEX IF NOT EXISTS idx_memory_scope
    ON memory_entries(scope);

CREATE INDEX IF NOT EXISTS idx_memory_scope_confidence
    ON memory_entries(scope, confidence);
```

### 1.2 存储操作（路径作用域）

```rust
impl MemoryService {
    /// 存储记忆（支持路径作用域）
    pub async fn set_with_scope(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
        source: MemorySource,
        scope: &MemoryScope,  // 🔥 路径字符串
    ) -> Result<()> {
        let full_key = self.state.full_key(key);
        let category_str = format!("{:?}", category);
        let confidence = source.confidence();
        let now = chrono::Utc::now().timestamp();

        // 1. 存储到数据库（包含作用域）
        match domain {
            MemoryDomain::Private => {
                self.conn.execute(
                    "INSERT INTO private_entries (key, value, category, created_at, updated_at, source, confidence, scope)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     source = excluded.source,
                     confidence = excluded.confidence,
                     scope = excluded.scope",
                    rusqlite::params![
                        key, value, category_str, now, now,
                        source, confidence,
                        scope.0,  // 路径字符串
                    ],
                )?;
            }
            MemoryDomain::Public => {
                self.conn.execute(
                    "INSERT INTO public_entries (key, value, category, created_at, updated_at, source, confidence, scope, federate, sync_status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'pending')
                     ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     source = excluded.source,
                     confidence = excluded.confidence,
                     scope = excluded.scope
                     ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     source = excluded.source,
                     confidence = excluded.confidence,
                     scope = excluded.scope",
                    rusqlite::params![
                        key, value, category_str, now, now,
                        source, confidence,
                        scope.0,  // 路径字符串
                    ],
                )?;
            }
        }

        // 2. 条件化向量索引（仍然基于 source）
        match source {
            MemorySource::UserForced => {
                let text = String::from_utf8_lossy(value);
                let category_str = format!("{:?}", category);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
            MemorySource::UserInput => {
                let text = String::from_utf8_lossy(value);
                let category_str = format!("{:?}", category);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
            MemorySource::AIProposalConfirmed => {
                let text = String::from_utf8_lossy(value);
                let category_str = format!("{:?}", category);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
            _ => {
                // 其他 source 不索引
            }
        }

        // 3. 更新索引
        self.update_index(key, domain, category, None)?;

        // 4. 使缓存失效
        if let Some(cache) = &self.state.cache {
            cache.invalidate(key).await;
        }

        Ok(())
    }

    /// 快捷方法：用户强制指定
    pub async fn set_user_forced(
        &self,
        key: &str,
        value: &[u8],
        scope: &MemoryScope,
    ) -> Result<()> {
        self.set_with_scope(
            key,
            value,
            MemoryDomain::Public,
            MemoryCategory::Context,
            MemorySource::UserForced,
            scope,
        ).await
    }

    /// 快捷方法：用户输入
    pub async fn set_user_input(
        &self,
        key: &str,
        value: &[u8],
        scope: &MemoryScope,
    ) -> Result<()> {
        self.set_with_scope(
            key,
            value,
            MemoryDomain::Public,
            MemoryCategory::Context,
            MemorySource::UserInput,
            scope,
        ).await
    }
}
```

### 1.3 作用域感知查询

```rust
impl MemoryService {
    /// 获取记忆（自动处理作用域继承）
    pub async fn get_with_scope(
        &self,
        key: &str,
        query_scope: &MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        // 1. 先尝试精确匹配当前作用域
        if let Some(entry) = self.get_by_scope(key, query_scope).await? {
            return Ok(Some(entry));
        }

        // 2. 尝试父级作用域（路径前缀继承）
        let parent_scopes = query_scope.parent_scopes();
        for parent_scope in parent_scopes {
            if let Some(entry) = self.get_by_scope(key, &parent_scope).await? {
                tracing::debug!(
                    "Found {} in parent scope {:?} (query scope {:?})",
                    key, parent_scope, query_scope
                );
                return Ok(Some(entry));
            }
        }

        // 3. 未找到
        Ok(None)
    }

    /// 按作用域查询
    async fn get_by_scope(
        &self,
        key: &str,
        scope: &MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        let like_pattern = format!("{}%", scope.like_pattern());

        let mut stmt = self.conn.prepare(&format!(
            "SELECT key, value, category, created_at, updated_at, source, confidence, scope
             FROM private_entries
             WHERE key = ?1 AND scope LIKE ?2
             UNION ALL
             SELECT key, value, category, created_at, updated_at, source, confidence, scope
             FROM public_entries
             WHERE key = ?1 AND scope LIKE ?2"
        ))?;

        let result = stmt.query_row(
            rusqlite::params![
                key,
                like_pattern,
                key,
                like_pattern,
            ],
            |row| {
                Ok(MemoryEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    domain: match row.get::<_, Option<String>>(7)?.unwrap_or(None) {
                        Some(_) => MemoryDomain::Private,
                        None => MemoryDomain::Public,
                    },
                    category: parse_category(&row.get::<_, String>(2)?),
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    source: parse_source(&row.get::<_, String>(5)?),
                    confidence: row.get(6)?,
                    vector_indexed: false,  // TODO: 查询实际值
                    access_count: 0,        // TODO: 查询实际值
                    parent_key: None,       // TODO: 查询实际值
                    confirmed_by_user: false, // TODO: 查询实际值
                    scope: row.get::<_, String>(7)?,
                })
            }
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get by scope: {}", e))),
        }
    }
}

// 用于 SQL 查询的辅助方法
impl MemoryScope {
    /// LIKE 模式（用于前缀查询）
    fn like_pattern(&self) -> String {
        if self.is_global() {
            "/%".to_string()  // 全局：所有路径
        } else {
            format!("{}%", self.0)  // 前缀匹配
        }
    }
}
```

### 1.4 父级判断

```rust
impl MemoryScope {
    /// 判断是否是另一个作用域的子级
    pub fn is_child_of(&self, other: &Self) -> bool {
        other.0.starts_with(self.0)
    }

    /// 判断是否是另一个作用域的父级
    pub fn is_parent_of(&self, other: &Self) -> bool {
        self.0.starts_with(other.0) && self != other
    }
}
```

---

## Phase 1: 作用域隔离存储 (P1.5.1)

### 1.1 作用域感知的存储操作

```rust
impl MemoryService {
    /// 存储记忆（支持作用域）
    pub async fn set_with_scope(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
        source: MemorySource,
        scope: MemoryScope,  // 🔥 新增参数
    ) -> Result<()> {
        let full_key = self.state.full_key(key);
        let category_str = format!("{:?}", category);
        let confidence = source.confidence();
        let now = chrono::Utc::now().timestamp();

        // 1. 存储到数据库（包含作用域信息）
        match domain {
            MemoryDomain::Private => {
                self.conn.execute(
                    "INSERT INTO private_entries (key, value, category, created_at, updated_at, source, confidence, scope_type, scope_project_id, scope_module_id, scope_task_id, scope_session_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                     ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     source = excluded.source,
                     confidence = excluded.confidence,
                     scope_type = excluded.scope_type,
                     scope_project_id = excluded.scope_project_id,
                     scope_module_id = excluded.scope_module_id,
                     scope_task_id = excluded.scope_task_id,
                     scope_session_id = excluded.scope_session_id",
                    rusqlite::params![
                        key, value, category_str, now, now,
                        source, confidence,
                        scope.scope_id(),
                        scope.project_id(),
                        scope.module_id(),
                        scope.task_id(),
                        scope.session_id(),
                    ],
                )?;
            }
            MemoryDomain::Public => {
                self.conn.execute(
                    "INSERT INTO public_entries (key, value, category, created_at, updated_at, source, confidence, scope_type, scope_project_id, scope_module_id, scope_task_id, scope_session_id, federate, sync_status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, 'pending')
                     ON CONFLICT(key) DO UPDATE SET
                     value = excluded.value,
                     category = excluded.category,
                     updated_at = excluded.updated_at,
                     source = excluded.source,
                     confidence = excluded.confidence,
                     scope_type = excluded.scope_type,
                     scope_project_id = excluded.scope_project_id,
                     scope_module_id = excluded.scope_module_id,
                     scope_task_id = excluded.scope_task_id,
                     scope_session_id = excluded.scope_session_id",
                    rusqlite::params![
                        key, value, category_str, now, now,
                        source, confidence,
                        scope.scope_id(),
                        scope.project_id(),
                        scope.module_id(),
                        scope.task_id(),
                        scope.session_id(),
                    ],
                )?;
            }
        }

        // 2. 条件化向量索引（仍然基于 source）
        match source {
            MemorySource::UserInput | MemorySource::UserForced { .. } => {
                let text = String::from_utf8_lossy(value);
                let category_str = format!("{:?}", category);
                self.state
                    .vector_storage
                    .index_memory(&full_key, text.as_bytes(), Some(&category_str))
                    .await?;
            }
            _ => {
                // 其他 source 不索引
            }
        }

        // 3. 更新索引
        self.update_index(key, domain, category, None)?;

        // 4. 使缓存失效
        if let Some(cache) = &self.state.cache {
            cache.invalidate(key).await;
        }

        Ok(())
    }
}

impl MemoryScope {
    /// 辅助方法：获取 module_id
    fn module_id(&self) -> Option<&str> {
        match self {
            Self::Module { module_id, .. } => Some(module_id),
            _ => None,
        }
    }

    /// 辅助方法：获取 task_id
    fn task_id(&self) -> Option<&str> {
        match self {
            Self::Task { task_id, .. } => Some(task_id),
            _ => None,
        }
    }

    /// 辅助方法：获取 session_id
    fn session_id(&self) -> Option<&str> {
        match self {
            Self::Session { session_id, .. } => Some(session_id),
            _ => None,
        }
    }
}
```

---

## Phase 2: 作用域感知检索 (P1.5.2)

### 2.1 作用域优先级查询

```rust
impl MemoryService {
    /// 作用域感知查询（自动处理优先级）
    pub async fn get_with_scope(
        &self,
        key: &str,
        query_scope: MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        // 1. 先尝试精确匹配当前作用域
        if let Some(entry) = self.get_by_scope(key, &query_scope).await? {
            return Ok(Some(entry));
        }

        // 2. 尝试父级作用域（作用域层级继承）
        let parent_scopes = query_scope.parent_scopes();
        for parent_scope in parent_scopes {
            if let Some(entry) = self.get_by_scope(key, &parent_scope).await? {
                tracing::debug!(
                    "Found {} in parent scope {:?} (query scope {:?})",
                    key, parent_scope, query_scope
                );
                return Ok(Some(entry));
            }
        }

        // 3. 未找到
        Ok(None)
    }

    /// 按作用域查询
    async fn get_by_scope(
        &self,
        key: &str,
        scope: &MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        let (table, field) = match scope {
            MemoryScope::Global => ("private_entries", "scope_type = 'Global'"),
            MemoryScope::Project { id } => (
                "private_entries",
                "scope_type = 'Project' AND scope_project_id = ?"
            ),
            MemoryScope::Module { project_id, module_id } => (
                "private_entries",
                "scope_type = 'Module' AND scope_project_id = ? AND scope_module_id = ?"
            ),
            MemoryScope::Task { project_id, task_id } => (
                "private_entries",
                "scope_type = 'Task' AND scope_project_id = ? AND scope_task_id = ?"
            ),
            MemoryScope::Session { project_id, session_id } => (
                "private_entries",
                "scope_type = 'Session' AND scope_project_id = ? AND scope_session_id = ?"
            ),
        };

        let mut stmt = self.conn.prepare(&format!(
            "SELECT key, value, category, created_at, updated_at, source, confidence,
                    scope_type, scope_project_id, scope_module_id, scope_task_id, scope_session_id
             FROM {} WHERE key = ?1 AND {}",
            table, field
        ))?;

        let result = stmt.query_row(
            rusqlite::params![
                key,
                scope.project_id(),
                scope.module_id(),
                scope.task_id(),
                scope.session_id(),
            ],
            |row| {
                Ok(MemoryEntry {
                    key: row.get(0)?,
                    value: row.get(1)?,
                    domain: MemoryDomain::Private,
                    category: parse_category(&row.get::<_, String>(2)?),
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    source: parse_source(&row.get::<_, String>(5)?),
                    confidence: row.get(6)?,
                    vector_indexed: false,  // TODO: 查询实际值
                    access_count: 0,        // TODO: 查询实际值
                    parent_key: None,       // TODO: 查询实际值
                    confirmed_by_user: false, // TODO: 查询实际值
                    scope: parse_scope(&row, 7)?,
                })
            }
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get by scope: {}", e))),
        }
    }
}

impl MemoryScope {
    /// 获取父级作用域（用于层级继承）
    pub fn parent_scopes(&self) -> Vec<MemoryScope> {
        match self {
            Self::Global => vec![],

            Self::Project { .. } => vec![Self::Global],

            Self::Module { project_id, .. } => vec![
                Self::Project { id: project_id.clone() },
                Self::Global,
            ],

            Self::Task { project_id, .. } => vec![
                Self::Project { id: project_id.clone() },
                Self::Global,
            ],

            Self::Session { project_id, .. } => vec![
                Self::Project { id: project_id.clone() },
                Self::Global,
            ],
        }
    }
}

fn parse_scope(row: &rusqlite::Row, offset: usize) -> Result<MemoryScope> {
    let scope_type: String = row.get(offset)?;
    let scope_project_id: Option<String> = row.get(offset + 1)?;
    let scope_module_id: Option<String> = row.get(offset + 2)?;
    let scope_task_id: Option<String> = row.get(offset + 3)?;
    let scope_session_id: Option<String> = row.get(offset + 4)?;

    Ok(match scope_type.as_str() {
        "Global" => MemoryScope::Global,
        "Project" => MemoryScope::Project {
            id: scope_project_id.ok_or_else(|| {
                CisError::storage("Missing project_id for Project scope")
            })?,
        },
        "Module" => MemoryScope::Module {
            project_id: scope_project_id.ok_or_else(|| {
                CisError::storage("Missing project_id for Module scope")
            })?,
            module_id: scope_module_id.ok_or_else(|| {
                CisError::storage("Missing module_id for Module scope")
            })?,
        },
        "Task" => MemoryScope::Task {
            project_id: scope_project_id.ok_or_else(|| {
                CisError::storage("Missing project_id for Task scope")
            })?,
            task_id: scope_task_id.ok_or_else(|| {
                CisError::storage("Missing task_id for Task scope")
            })?,
        },
        "Session" => MemoryScope::Session {
            project_id: scope_project_id.ok_or_else(|| {
                CisError::storage("Missing project_id for Session scope")
            })?,
            session_id: scope_session_id.ok_or_else(|| {
                CisError::storage("Missing session_id for Session scope")
            })?,
        },
        _ => return Err(CisError::storage(format!("Unknown scope type: {}", scope_type))),
    })
}
```

### 2.2 作用域过滤的向量搜索

```rust
impl VectorStorage {
    /// 作用域感知的向量搜索
    pub async fn search_memory_with_scope(
        &self,
        query: &[f32],
        top_k: usize,
        search_scope: MemoryScope,
        prefer_user_input: bool,
        min_confidence: Option<f32>,
    ) -> Result<Vec<SearchResult>> {
        // 1. HNSW 搜索获取候选
        let mut results = self.hnsw_search(query, top_k * 3).await?;

        // 2. 过滤作用域（只保留搜索作用域或父级作用域的记忆）
        results.retain(|r| {
            r.memory.belongs_to_scope(&search_scope) ||
            search_scope.is_global()  // 全局作用域可以看到所有记忆
        });

        // 3. 用户输入优先
        if prefer_user_input {
            results.sort_by(|a, b| {
                let priority_a = match a.source {
                    MemorySource::UserForced { .. } => 0,  // 🔥 UserForced 最高优先
                    MemorySource::UserInput => 1,
                    MemorySource::AIProposalConfirmed => 2,
                    _ => 3,
                };
                let priority_b = match b.source {
                    MemorySource::UserForced { .. } => 0,
                    MemorySource::UserInput => 1,
                    MemorySource::AIProposalConfirmed => 2,
                    _ => 3,
                };
                priority_a.cmp(&priority_b).unwrap()
            });
        }

        // 4. 过滤 AIProposalSummary（未确认的方案总结）
        results.retain(|r| {
            !matches!(r.source, MemorySource::AIProposalSummary)
        });

        // 5. 过滤低可信度
        if let Some(min_conf) = min_confidence {
            results.retain(|r| r.confidence >= min_conf);
        }

        // 6. 联合排序（confidence * 0.7 + similarity * 0.3）
        results.sort_by(|a, b| {
            let score_a = a.confidence * 0.7 + a.similarity * 0.3;
            let score_b = b.confidence * 0.7 + b.similarity * 0.3;
            score_b.partial_cmp(&score_a).unwrap()
        });

        results.truncate(top_k);
        Ok(results)
    }
}
```

---

## Phase 3: 作用域继承和覆盖 (P1.5.3)

### 3.1 作用域层级继承

```rust
impl MemoryService {
    /// 列出记忆（支持作用域继承）
    pub async fn list_keys_with_scope_inherit(
        &self,
        prefix: &str,
        scope: MemoryScope,
    ) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        // 1. 当前作用域精确匹配
        let scope_keys = self.list_keys_by_scope(prefix, Some(&scope)).await?;
        keys.extend(scope_keys);

        // 2. 父级作用域继承（不重复）
        for parent_scope in scope.parent_scopes() {
            let parent_keys = self.list_keys_by_scope(prefix, Some(&parent_scope)).await?;
            for key in parent_keys {
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
        }

        Ok(keys)
    }

    /// 按作用域列出记忆键
    async fn list_keys_by_scope(
        &self,
        prefix: &str,
        scope: Option<&MemoryScope>,
    ) -> Result<Vec<String>> {
        let mut keys = Vec::new();

        let (where_clause, params) = if let Some(scope) = scope {
            match scope {
                MemoryScope::Global => (
                    "scope_type = ?1".to_string(),
                    vec!["Global"]
                ),
                MemoryScope::Project { id } => (
                    "scope_type = ?1 AND scope_project_id = ?2".to_string(),
                    vec!["Project", id]
                ),
                MemoryScope::Module { project_id, module_id } => (
                    "scope_type = ?1 AND scope_project_id = ?2 AND scope_module_id = ?3".to_string(),
                    vec!["Module", project_id, module_id]
                ),
                MemoryScope::Task { project_id, task_id } => (
                    "scope_type = ?1 AND scope_project_id = ?2 AND scope_task_id = ?3".to_string(),
                    vec!["Task", project_id, task_id]
                ),
                MemoryScope::Session { project_id, session_id } => (
                    "scope_type = ?1 AND scope_project_id = ?2 AND scope_session_id = ?3".to_string(),
                    vec!["Session", project_id, session_id]
                ),
            }
        } else {
            ("1 = 1".to_string(), vec![1])  // 所有作用域
        };

        let mut stmt = self.conn.prepare(&format!(
            "SELECT key FROM private_entries WHERE key LIKE ? AND {}",
            where_clause
        ))?;

        let like = format!("{}%", prefix);
        let mut params = vec![Box::new(like)];
        for param in params {
            params.push(Box::new(param));
        }

        let rows = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref()))),
            |row| row.get::<_, String>(0)
        ).map_err(|e| CisError::storage(format!("Failed to list keys: {}", e)))?;

        for row in rows {
            keys.push(row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?);
        }

        Ok(keys)
    }
}
```

### 3.2 作用域覆盖优先级

```rust
impl MemoryService {
    /// 获取记忆（自动处理作用域覆盖）
    ///
    /// 优先级：Session > Task > Module > Project > Global
    pub async fn get_with_scope_override(
        &self,
        key: &str,
        query_scope: MemoryScope,
    ) -> Result<Option<MemoryEntry>> {
        // 1. 按优先级从高到低查找
        let scopes_by_priority = vec![
            query_scope.clone(),           // 当前作用域（最高优先级）
        ];

        // 添加父级作用域（按优先级排序）
        let mut parent_scopes = query_scope.parent_scopes();
        parent_scopes.reverse();  // 反转（优先级从高到低）
        scopes_by_priority.extend(parent_scopes);

        for scope in scopes_by_priority {
            if let Some(entry) = self.get_by_scope(key, &scope).await? {
                tracing::debug!(
                    "Found {} in scope {:?} (query scope {:?})",
                    key, scope, query_scope
                );
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }
}
```

---

## 完整使用示例

### 场景 1：项目级 UserForced 隔离

```rust
// ========== 项目 A ==========
service.set_user_forced(
    "project-A/theme",
    b"light",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Project {
        id: "project-A".to_string(),
    },
).await?;

// ========== 项目 B ==========
service.set_user_forced(
    "project-B/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Project {
        id: "project-B".to_string(),
    },
).await?;

// ========== 全局默认 ==========
service.set_user_input(
    "global/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context,
    MemoryScope::Global,
).await?;

// ========== 查询：项目 A 中 ==========
let entry = service.get_with_scope(
    "theme",  // 🔥 相同 key
    MemoryScope::Project {
        id: "project-A".to_string(),
    },
).await?;

// ✅ 返回：project-A/theme (light)
// - 优先返回项目级 UserForced
// - 不会污染项目 B 的配置

// ========== 查询：项目 B 中 ==========
let entry = service.get_with_scope(
    "theme",
    MemoryScope::Project {
        id: "project-B".to_string(),
    },
).await?;

// ✅ 返回：project-B/theme (dark)
// - 独立于项目 A 的配置

// ========== 查询：其他项目（无项目级配置）==========
let entry = service.get_with_scope(
    "theme",
    MemoryScope::Project {
        id: "project-C".to_string(),
    },
).await?;

// ✅ 返回：global/theme (dark)
// - 继承全局默认配置
```

### 场景 2：模块级隔离

```rust
// ========== 模块 A：数据库 ==========
service.set_user_forced(
    "database/connection-pool",
    b"max_connections=100",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    MemoryScope::Module {
        project_id: "project-A".to_string(),
        module_id: "database".to_string(),
    },
).await?;

// ========== 模块 B：API ==========
service.set_user_forced(
    "api/connection-pool",
    b"max_connections=50",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    MemoryScope::Module {
        project_id: "project-A".to_string(),
        module_id: "api".to_string(),
    },
).await?;

// ========== 查询：模块 A 中 ==========
let entry = service.get_with_scope(
    "connection-pool",
    MemoryScope::Module {
        project_id: "project-A".to_string(),
        module_id: "database".to_string(),
    },
).await?;

// ✅ 返回：database/connection-pool (max_connections=100)
// - 不会返回 api/connection-pool

// ========== 向量搜索：模块 A ==========
let results = vector_storage.search_memory_with_scope(
    query_vec,
    10,
    MemoryScope::Module {
        project_id: "project-A".to_string(),
        module_id: "database".to_string(),
    },
    true,  // prefer_user_input
    Some(0.8),  // min_confidence
).await?;

// ✅ 结果：
// - 只包含模块 A 的记忆（或全局记忆）
// - 不包含模块 B 的记忆（避免污染）
```

### 场景 3：会话级临时隔离

```rust
// ========== 会话 1 ==========
let session1_scope = MemoryScope::Session {
    project_id: "project-A".to_string(),
    session_id: "session-123".to_string(),
};

service.set_user_input(
    "temp/workflow-state",
    b"step=3",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    session1_scope.clone(),
).await?;

// ========== 会话 2 ==========
let session2_scope = MemoryScope::Session {
    project_id: "project-A".to_string(),
    session_id: "session-456".to_string(),
};

service.set_user_input(
    "temp/workflow-state",
    b"step=5",
    MemoryDomain::Private,
    MemoryCategory::Execution,
    session2_scope.clone(),
).await?;

// ========== 查询：会话 1 ==========
let entry = service.get_with_scope(
    "temp/workflow-state",
    session1_scope,
).await?;

// ✅ 返回：step=3（会话 1 的状态）
// - 不会返回会话 2 的状态

// ========== 查询：项目级别（继承所有会话）==========
let entry = service.get_with_scope(
    "temp/workflow-state",
    MemoryScope::Project {
        id: "project-A".to_string(),
    },
).await?;

// ❌ 返回 None
// - 项目级别看不到会话级记忆（会话隔离）
```

---

## 性能和存储

### 索引优化

```sql
-- 复合索引（作用域 + 可信度）
CREATE INDEX idx_private_scope_confidence
    ON private_entries(scope_type, scope_project_id, confidence);

-- 查询性能
-- ✅ 精确作用域查询：~1ms
-- ✅ 作用域继承查询：~5ms（查父级作用域）
-- ✅ 向量搜索作用域过滤：~2ms
```

### 存储开销

| 字段 | 类型 | 大小 |
|------|------|------|
| scope_type | TEXT | 8 bytes |
| scope_project_id | TEXT | 0-50 bytes |
| scope_module_id | TEXT | 0-50 bytes |
| scope_task_id | TEXT | 0-50 bytes |
| scope_session_id | TEXT | 0-50 bytes |
| **总计** | | **~8-208 bytes/条目** |

假设 10000 条记忆：
- 额外开销：~2 MB
- 可忽略不计

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 作用域配置错误 | 记忆无法访问 | 提供配置验证和调试工具 |
| 作用域层级过深 | 查询性能下降 | 限制最多 3 层继承（Session/Task → Module → Project → Global） |
| 跨作用域向量检索 | 性能下降 | 缓存作用域过滤结果 |
| 作用域迁移 | 项目重组困难 | 提供作用域重命名工具 |

---

## 实施计划

### Phase 1: 作用域隔离存储 (P1.5.1)
- [ ] 定义 `MemoryScope` 枚举
- [ ] 扩展 `MemoryEntry` 结构
- [ ] 数据库 Schema 迁移
- [ ] 实现 `set_with_scope()`
- [ ] 单元测试

### Phase 2: 作用域感知检索 (P1.5.2)
- [ ] 实现 `get_with_scope()`
- [ ] 实现 `get_by_scope()`
- [ ] 实现 `list_keys_by_scope()`
- [ ] 作用域继承逻辑
- [ ] 性能测试

### Phase 3: 作用域向量搜索 (P1.5.3)
- [ ] 实现 `search_memory_with_scope()`
- [ ] 作用域过滤优化
- [ ] 集成到 `ContextProvider`

---

**维护者**: CIS v1.1.6 Team
**最后更新**: 2026-02-13

---

## 完整使用示例（简化版）

### 场景 1：项目级隔离

```rust
use cis_core::memory::{MemoryService, MemoryScope, MemorySource};

async fn example_project_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 项目 A：Rust 架构
    service.set_user_forced(
        "project-a/architecture",
        b"Microservices architecture with Rust and SQLite",
        &MemoryScope::project("project-a"),  // 🔥 路径字符串
    ).await?;

    // 项目 B：Python 架构  
    service.set_user_forced(
        "project-b/architecture",
        b"RESTful API with Python and PostgreSQL",
        &MemoryScope::project("project-b"),
    ).await?;

    // 查询项目 A（自动继承，看不到项目 B）
    let entry = service.get_with_scope(
        "architecture",
        &MemoryScope::project("project-a"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"Microservices architecture with Rust and SQLite");
}
```

### 场景 2：模块级隔离

```rust
async fn example_module_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 模块 A：数据库
    service.set_user_forced(
        "database/connection-pool",
        b"max_connections=100",
        &MemoryScope::module("project-a", "database"),
    ).await?;

    // 模块 B：API
    service.set_user_forced(
        "api/connection-pool",
        b"max_connections=50",
        &MemoryScope::module("project-a", "api"),
    ).await?;

    // 查询数据库模块（看不到 API 模块）
    let entry = service.get_with_scope(
        "connection-pool",
        &MemoryScope::module("project-a", "database"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"max_connections=100");
}
```

### 场景 3：全局 vs 项目

```rust
async fn example_global_vs_project() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 全局默认
    service.set_user_input(
        "theme",
        b"dark",
        &MemoryScope::global(),
    ).await?;

    // 项目特定（覆盖全局）
    service.set_user_forced(
        "theme",
        b"light",  // 🔥 项目 A 强制浅色
        &MemoryScope::project("project-a"),
    ).await?;

    // 查询项目 A（返回 light）
    let entry = service.get_with_scope(
        "theme",
        &MemoryScope::project("project-a"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"light");

    // 查询其他项目（继承全局 dark）
    let entry = service.get_with_scope(
        "theme",
        &MemoryScope::project("project-b"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"dark");
}
```

### 场景 4：路径层级

```rust
async fn example_path_hierarchy() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 全局
    let global = MemoryScope::global();
    assert_eq!(global.level(), 0);
    assert!(global.is_global());

    // 项目
    let project = MemoryScope::project("my-project");
    assert_eq!(project.level(), 1);
    assert_eq!(project.project_id().unwrap(), "my-project");

    // 模块
    let module = MemoryScope::module("my-project", "database");
    assert_eq!(module.level(), 2);
    assert!(module.project_id().unwrap(), "my-project");

    // 任务
    let task = MemoryScope::task("my-project", "task-123");
    assert_eq!(task.level(), 3);
    assert!(task.project_id().unwrap(), "my-project");

    // 层级判断
    assert!(task.is_child_of(&module));  // Task 是 Module 的子级
    assert!(module.is_parent_of(&task));  // Module 是 Task 的父级
    assert!(global.is_parent_of(&project));  // Global 是 Project 的父级
}
```


---

## v1.1.7 完整使用示例（User + Group + Path 三维隔离）

### 场景 1：用户级完全隔离

```rust
use cis_core::memory::{MemoryService, MemoryScope, MemorySource};

async fn example_user_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // Alice 的个人偏好
    service.set_user_input(
        "preference/theme",
        b"dark",
        &MemoryScope::user("alice"),  // /user-alice
    ).await?;

    // Bob 的个人偏好（完全隔离）
    service.set_user_input(
        "preference/theme",
        b"light",
        &MemoryScope::user("bob"),  // /user-bob
    ).await?;

    // 查询 Alice 的偏好
    let entry = service.get_with_scope(
        "preference/theme",
        &MemoryScope::user("alice"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"dark");

    // 查询 Bob 的偏好（完全独立）
    let entry = service.get_with_scope(
        "preference/theme",
        &MemoryScope::user("bob"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"light");

    // Alice 查不到 Bob 的记忆
    let entry = service.get_with_scope(
        "preference/theme",
        &MemoryScope::user("alice"),
    ).await?.unwrap();

    assert_ne!(entry.value, b"light");  // ✅ 确认隔离
}
```

### 场景 2：组级团队共享

```rust
async fn example_group_sharing() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // Alice 在 dev 团队设置的团队约定
    service.set_user_forced(
        "team/coding-standards",
        b"Follow Rust API guidelines and use Result<T> for errors",
        &MemoryScope::group("alice", "team-dev"),  // /user-alice/team-dev
    ).await?;

    // Bob 查询 dev 团队的约定（可以看到 Alice 设置的）
    let entry = service.get_with_scope(
        "team/coding-standards",
        &MemoryScope::group("bob", "team-dev"),  // /user-bob/team-dev
    ).await?.unwrap();

    assert_eq!(entry.value, b"Follow Rust API guidelines and use Result<T> for errors");

    // design 团队的约定（独立）
    service.set_user_forced(
        "team/coding-standards",
        b"Use TypeScript with strict mode",
        &MemoryScope::group("charlie", "team-design"),  // /user-charlie/team-design
    ).await?;

    // dev 团队看不到 design 团队的约定
    let entry = service.get_with_scope(
        "team/coding-standards",
        &MemoryScope::group("alice", "team-dev"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"Follow Rust API guidelines and use Result<T> for errors");
    
    // 🔥 关键：不同用户的组 ID 相同，可以共享
    // Alice 和 Bob 都在 team-dev，共享记忆
    // Charlie 在 team-design，完全独立
}
```

### 场景 3：项目级隔离（同一团队）

```rust
async fn example_project_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 项目 A：Rust 架构
    service.set_user_forced(
        "project/architecture",
        b"Microservices architecture with Rust and SQLite",
        &MemoryScope::project("alice", "team-dev", "project-a"),  // /user-alice/team-dev/project-a
    ).await?;

    // 项目 B：Python 架构
    service.set_user_forced(
        "project/architecture",
        b"RESTful API with Python and PostgreSQL",
        &MemoryScope::project("alice", "team-dev", "project-b"),  // /user-alice/team-dev/project-b
    ).await?;

    // 查询项目 A（看不到项目 B）
    let entry = service.get_with_scope(
        "project/architecture",
        &MemoryScope::project("alice", "team-dev", "project-a"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"Microservices architecture with Rust and SQLite");

    // Bob 在同一团队也可以看到项目 A 的架构（组级共享）
    let entry = service.get_with_scope(
        "project/architecture",
        &MemoryScope::project("bob", "team-dev", "project-a"),  // /user-bob/team-dev/project-a
    ).await?.unwrap();

    assert_eq!(entry.value, b"Microservices architecture with Rust and SQLite");

    // 🔥 关键：同一团队(team-dev)的成员(Alice, Bob)共享项目级记忆
    // 不同项目(project-a, project-b)完全隔离
}
```

### 场景 4：跨团队项目访问（多团队项目）

```rust
async fn example_cross_team_project() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // Alice 在 dev 团队设置项目配置
    service.set_user_forced(
        "project/deadline",
        b"2026-03-01",
        &MemoryScope::project("alice", "team-dev", "project-x"),  // /user-alice/team-dev/project-x
    ).await?;

    // Charlie 在 design 团队访问同一项目
    // 🔥 关键：不同团队访问同一项目需要特殊处理
    // 方案 1: 项目级共享（推荐）
    let entry = service.get_with_scope_shared(
        "project/deadline",
        &MemoryScope::project("charlie", "team-design", "project-x"),
        SharedMode::GroupShared,  // 允许跨团队访问共享项目
    ).await?.unwrap();

    assert_eq!(entry.value, b"2026-03-01");

    // 方案 2: 显式映射（不推荐，需要手动配置）
    let entry = service.get_with_scope_mapped(
        "project/deadline",
        &MemoryScope::project("charlie", "team-design", "project-x"),
        &MemoryScope::project("alice", "team-dev", "project-x"),  // 映射到原始团队
    ).await?.unwrap();

    assert_eq!(entry.value, b"2026-03-01");
}
```

### 场景 5：模块级隔离

```rust
async fn example_module_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 模块 A：数据库
    service.set_user_forced(
        "module/connection-pool",
        b"max_connections=100",
        &MemoryScope::module("alice", "team-dev", "project-a", "database"),  // /user-alice/team-dev/project-a/module-database
    ).await?;

    // 模块 B：API
    service.set_user_forced(
        "module/connection-pool",
        b"max_connections=50",
        &MemoryScope::module("alice", "team-dev", "project-a", "api"),  // /user-alice/team-dev/project-a/module-api
    ).await?;

    // 查询数据库模块（看不到 API 模块）
    let entry = service.get_with_scope(
        "module/connection-pool",
        &MemoryScope::module("alice", "team-dev", "project-a", "database"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"max_connections=100");

    // 向量搜索：数据库模块
    let results = service.search_with_scope(
        "connection pool configuration",
        10,
        &MemoryScope::module("alice", "team-dev", "project-a", "database"),
        true,  // prefer_user_input
        Some(0.8),  // min_confidence
    ).await?;

    // ✅ 结果只包含数据库模块的记忆（不包含 API 模块）
    for result in results {
        assert!(result.memory.scope.contains("/module-database"));
        assert!(!result.memory.scope.contains("/module-api"));
    }
}
```

### 场景 6：全局 vs 用户 vs 组 vs 项目（层级继承）

```rust
async fn example_scope_hierarchy() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 1. 系统全局默认
    service.set_user_input(
        "theme",
        b"dark",
        &MemoryScope::global(),  // /
    ).await?;

    // 2. Alice 的个人偏好（覆盖全局）
    service.set_user_input(
        "theme",
        b"light",
        &MemoryScope::user("alice"),  // /user-alice
    ).await?;

    // 3. dev 团队的偏好（覆盖个人）
    service.set_user_forced(
        "theme",
        b"auto",
        &MemoryScope::group("alice", "team-dev"),  // /user-alice/team-dev
    ).await?;

    // 4. 项目 A 的偏好（最高优先级）
    service.set_user_forced(
        "theme",
        b"dimmed",
        &MemoryScope::project("alice", "team-dev", "project-a"),  // /user-alice/team-dev/project-a
    ).await?;

    // 查询项目 A（返回 dimmed）
    let entry = service.get_with_scope(
        "theme",
        &MemoryScope::project("alice", "team-dev", "project-a"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"dimmed");

    // 查询同一团队的另一项目（返回 auto）
    let entry = service.get_with_scope(
        "theme",
        &MemoryScope::project("alice", "team-dev", "project-b"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"auto");

    // 查询 design 团队（继承全局 dark）
    let entry = service.get_with_scope(
        "theme",
        &MemoryScope::group("alice", "team-design"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"dark");

    // 🔥 层级优先级：Project > Group > User > Global
}
```

### 场景 7：Git Worktree 映射

```rust
async fn example_git_worktree_mapping() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 1. 获取 CIS 基础目录
    let cis_base = std::path::PathBuf::from("~/.cis");
    let cis_base = std::fs::canonicalize(cis_base.expand_env())?;

    // 2. 创建项目作用域
    let scope = MemoryScope::project("alice", "team-dev", "project-a");

    // 3. 映射到文件系统路径（用于 git worktree）
    let worktree_path = scope.to_path_buf(&cis_base);
    // 结果：~/.cis/user-alice/team-dev/project-a/

    // 4. 创建 git worktree
    std::fs::create_dir_all(&worktree_path)?;
    std::process::Command::new("git")
        .args(["worktree", "add", worktree_path.to_str().unwrap(), "branch-a"])
        .current_dir("~/repos/project-a")
        .output()?;

    // 5. 存储项目记忆（存储到 worktree 中的 .cis/memory.db）
    service.set_user_forced(
        "project/branch",
        b"branch-a",
        &scope,
    ).await?;

    // 6. 在 worktree 中工作
    std::env::set_current_dir(&worktree_path);

    // 7. 记忆自动关联到当前 worktree
    let current_scope = MemoryScope::from_current_dir(&cis_base)?;
    assert_eq!(current_scope, scope);

    // 8. CI/CD 集成
    // 在 GitHub Actions 中，自动检测当前 worktree 并加载对应记忆
    if let Ok(scope) = MemoryScope::from_current_dir(&cis_base) {
        let entry = service.get_with_scope("project/branch", &scope).await?;
        if let Some(entry) = entry {
            println!("Current branch: {}", String::from_utf8_lossy(&entry.value));
        }
    }

    Ok(())
}
```

### 场景 8：会话级临时隔离

```rust
async fn example_session_isolation() -> Result<()> {
    let service = MemoryService::new_default().await?;

    // 会话 1：临时工作流状态
    let session1 = MemoryScope::session("alice", "session-123");  // /user-alice/.session-123

    service.set_user_input(
        "workflow/current-step",
        b"3",
        &session1,
    ).await?;

    // 会话 2：独立的工作流状态
    let session2 = MemoryScope::session("alice", "session-456");  // /user-alice/.session-456

    service.set_user_input(
        "workflow/current-step",
        b"5",
        &session2,
    ).await?;

    // 查询会话 1（看不到会话 2）
    let entry = service.get_with_scope(
        "workflow/current-step",
        &session1,
    ).await?.unwrap();

    assert_eq!(entry.value, b"3");

    // 项目级别查询（看不到会话级记忆）
    let entry = service.get_with_scope(
        "workflow/current-step",
        &MemoryScope::project("alice", "team-dev", "project-a"),
    ).await?;

    assert_eq!(entry, None);  // 会话隔离

    // 🔥 会话记忆可以导出到项目级
    service.promote_session_to_project(
        &session1,
        &MemoryScope::project("alice", "team-dev", "project-a"),
        "workflow/final-step".to_string(),
    ).await?;

    // 现在项目级可以看到
    let entry = service.get_with_scope(
        "workflow/final-step",
        &MemoryScope::project("alice", "team-dev", "project-a"),
    ).await?.unwrap();

    assert_eq!(entry.value, b"3");
}
```

### 场景 9：实际项目场景（完整流程）

```rust
async fn example_real_world_workflow() -> Result<()> {
    let service = MemoryService::new_default().await?;
    let cis_base = std::path::PathBuf::from("~/.cis");

    // ========== 阶段 1：团队初始化 ==========
    // 团队管理员设置团队约定
    service.set_user_forced(
        "team/code-review-policy",
        b"All PRs must be reviewed by at least 2 team members",
        &MemoryScope::group("alice", "team-dev"),
    ).await?;

    // ========== 阶段 2：项目初始化 ==========
    let project_scope = MemoryScope::project("alice", "team-dev", "project-x");

    // 创建 git worktree
    let worktree_path = project_scope.to_path_buf(&cis_base);
    std::fs::create_dir_all(&worktree_path)?;
    std::process::Command::new("git")
        .args(["worktree", "add", worktree_path.to_str().unwrap(), "main"])
        .current_dir("~/repos/project-x")
        .output()?;

    // 项目级配置
    service.set_user_forced(
        "project/build-tool",
        b"cargo",
        &project_scope,
    ).await?;

    // ========== 阶段 3：模块开发 ==========
    let db_module = MemoryScope::module("alice", "team-dev", "project-x", "database");

    // 模块级配置
    service.set_user_forced(
        "module/connection-pool",
        b"max_connections=100",
        &db_module,
    ).await?;

    // ========== 阶段 4：任务执行 ==========
    let task_scope = MemoryScope::task("alice", "team-dev", "project-x", "migration-001");

    service.set_user_input(
        "task/status",
        b"in_progress",
        &task_scope,
    ).await?;

    // ========== 阶段 5：临时会话 ==========
    let session_scope = MemoryScope::session("alice", "session-debug-001");

    service.set_user_input(
        "session/debug-var",
        b"42",
        &session_scope,
    ).await?;

    // ========== 验证隔离 ==========
    
    // 1. 任务级查询（只看任务级和父级）
    let entry = service.get_with_scope(
        "module/connection-pool",
        &task_scope,  // 任务作用域
    ).await?.unwrap();

    // ✅ 继承模块级配置（任务继承模块）
    assert_eq!(entry.value, b"max_connections=100");

    // 2. 模块级查询（看不到会话级）
    let entry = service.get_with_scope(
        "session/debug-var",
        &db_module,  // 模块作用域
    ).await?;

    // ✅ 会话隔离
    assert_eq!(entry, None);

    // 3. 项目级查询（看到团队约定）
    let entry = service.get_with_scope(
        "team/code-review-policy",
        &project_scope,  // 项目作用域
    ).await?.unwrap();

    // ✅ 继承团队级配置（项目继承团队）
    assert_eq!(entry.value, b"All PRs must be reviewed by at least 2 team members");

    Ok(())
}
```

---

## v1.1.7 实现细节补充

### 1. 跨团队项目访问模式

```rust
/// 跨团队项目访问模式
pub enum SharedMode {
    /// 组级共享（默认）：同一组 ID 的用户可以访问
    GroupShared,
    /// 项目级共享：所有团队都可以访问（需要显式配置）
    ProjectShared,
    /// 私有：仅本团队可以访问
    Private,
}

impl MemoryService {
    /// 跨作用域查询（支持跨团队访问）
    pub async fn get_with_scope_shared(
        &self,
        key: &str,
        query_scope: &MemoryScope,
        mode: SharedMode,
    ) -> Result<Option<MemoryEntry>> {
        // 1. 尝试精确匹配当前作用域
        if let Some(entry) = self.get_by_scope(key, query_scope).await? {
            return Ok(Some(entry));
        }

        // 2. 根据 mode 决定是否跨作用域查询
        match mode {
            SharedMode::GroupShared => {
                // 组级共享：同一组 ID 的用户可以访问
                if let (Some(user_id), Some(group_id), Some(project_id)) = (
                    query_scope.user_id(),
                    query_scope.group_id(),
                    query_scope.project_id(),
                ) {
                    // 尝试从其他用户的同一组访问
                    let cross_user_scope = MemoryScope::project("other-user", group_id, project_id);
                    if let Some(entry) = self.get_by_scope(key, &cross_user_scope).await? {
                        return Ok(Some(entry));
                    }
                }
            }
            SharedMode::ProjectShared => {
                // 项目级共享：所有团队都可以访问
                if let Some(project_id) = query_scope.project_id() {
                    // 尝试从其他组访问
                    let cross_group_scope = MemoryScope::project("other-user", "other-group", project_id);
                    if let Some(entry) = self.get_by_scope(key, &cross_group_scope).await? {
                        return Ok(Some(entry));
                    }
                }
            }
            SharedMode::Private => {
                // 私有：不跨作用域查询
            }
        }

        // 3. 尝试父级作用域
        for parent_scope in query_scope.parents() {
            if let Some(entry) = self.get_by_scope(key, &parent_scope).await? {
                return Ok(Some(entry));
            }
        }

        Ok(None)
    }
}
```

### 2. 从当前目录自动检测作用域

```rust
impl MemoryScope {
    /// 从当前目录自动检测作用域
    ///
    /// # 示例
    /// ```ignore
    /// // 当前目录：~/.cis/user-alice/team-dev/project-a/module-database/
    /// let scope = MemoryScope::from_current_dir(&cis_base)?;
    /// assert_eq!(scope, MemoryScope::module("alice", "team-dev", "project-a", "database"));
    /// ```
    pub fn from_current_dir(base: &std::path::Path) -> Result<Self> {
        let current_dir = std::env::current_dir()?;
        let relative_path = current_dir.strip_prefix(base)
            .map_err(|_| CisError::memory("Not in CIS directory"))?;

        let parts: Vec<&str> = relative_path
            .iter()
            .filter_map(|p| p.to_str())
            .collect();

        if parts.is_empty() {
            return Ok(Self::global());
        }

        // 解析路径
        match parts.len() {
            0 => Ok(Self::global()),
            1 if parts[0].starts_with("user-") => Ok(Self::user(&parts[0][5..])),
            2 if parts[0].starts_with("user-") => {
                Ok(Self::group(&parts[0][5..], parts[1]))
            }
            3 if parts[0].starts_with("user-") => {
                Ok(Self::project(&parts[0][5..], parts[1], parts[2]))
            }
            4 if parts[0].starts_with("user-") => {
                match parts[3].split_once('-') {
                    Some(("module", module_id)) => {
                        Ok(Self::module(&parts[0][5..], parts[1], parts[2], module_id))
                    }
                    Some(("task", task_id)) => {
                        Ok(Self::task(&parts[0][5..], parts[1], parts[2], task_id))
                    }
                    _ => Err(CisError::memory("Invalid scope component")),
                }
            }
            _ => Err(CisError::memory("Invalid scope path")),
        }
    }
}
```

### 3. 会话记忆提升

```rust
impl MemoryService {
    /// 将会话级记忆提升到项目级
    pub async fn promote_session_to_project(
        &self,
        session_scope: &MemoryScope,
        project_scope: &MemoryScope,
        new_key: String,
    ) -> Result<()> {
        // 1. 获取会话级记忆
        let old_key = "session/temp-var";  // 或者从参数传入
        if let Some(entry) = self.get_by_scope(old_key, session_scope).await? {
            // 2. 复制到项目级
            self.set_with_scope(
                &new_key,
                &entry.value,
                MemoryDomain::Private,
                entry.category,
                entry.source,
                project_scope,
            ).await?;

            // 3. 删除会话级记忆
            self.delete_with_scope(old_key, session_scope).await?;

            Ok(())
        } else {
            Err(CisError::memory("Session memory not found"))
        }
    }

    /// 删除指定作用域的记忆
    pub async fn delete_with_scope(
        &self,
        key: &str,
        scope: &MemoryScope,
    ) -> Result<()> {
        let like_pattern = scope.like_pattern();
        
        self.conn.execute(
            "DELETE FROM private_entries WHERE key = ?1 AND scope LIKE ?2",
            rusqlite::params![key, like_pattern],
        )?;

        self.conn.execute(
            "DELETE FROM public_entries WHERE key = ?1 AND scope LIKE ?2",
            rusqlite::params![key, like_pattern],
        )?;

        Ok(())
    }
}
```

---

**维护者**: CIS v1.1.7 Team  
**最后更新**: 2026-02-14  
**版本**: v1.1.7 (User + Group + Path 完整三维隔离)
