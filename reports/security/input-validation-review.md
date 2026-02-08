# CIS 项目输入验证和路径遍历防护审查报告

**审查日期**: 2026-02-08  
**审查范围**: cis-core/src/init/wizard.rs, cis-core/src/skill/manager.rs, cis-core/src/storage/  
**审查人员**: Security Review Agent

---

## 执行摘要

本次审查针对 CIS 项目的输入验证和路径遍历防护进行了全面分析。总体而言，项目采用了较好的安全实践，包括：
- 使用参数化 SQL 查询防止 SQL 注入
- 外部命令调用使用硬编码命令名
- 实现了字符串长度验证函数
- 沙箱模块实现了路径遍历检测

但也发现了一些需要关注的安全问题，主要包括 SQL 语句拼接和路径验证的不足之处。

---

## 详细发现

### 🔴 高危问题

#### 1. SQL 注入风险 - connection.rs

**位置**: `cis-core/src/storage/connection.rs`

**问题描述**: `attach` 和 `detach` 函数中直接拼接 SQL 语句，如果路径或别名未正确验证，可能导致 SQL 注入。

```rust
// 第 132 行
let sql = format!("ATTACH DATABASE '{}' AS {}", path_str, alias);
self.primary.execute(&sql, [])

// 第 165 行
let sql = format!("DETACH DATABASE {}", alias);
self.primary.execute(&sql, [])
```

**风险分析**: 
- `path_str` 来自文件系统路径的 `to_string_lossy()`，如果路径包含单引号，可能破坏 SQL 语句结构
- 虽然 `alias` 经过 `is_valid_alias` 验证（只允许字母数字下划线），但 `path_str` 没有类似验证

**缓解措施**:
- `path_str` 通过 `canonicalize()` 获取绝对路径，减少了路径遍历风险
- 路径通常来自受控的 `Paths` 模块，而非直接用户输入

**建议修复**:
```rust
// 对路径进行转义或验证
fn validate_path_for_sql(path: &str) -> Result<()> {
    if path.contains('\'') || path.contains('\0') {
        return Err(CisError::invalid_input("Invalid path characters"));
    }
    Ok(())
}
```

**严重程度**: 🟠 中危（路径通常来自受控来源）

---

### 🟠 中危问题

#### 2. 路径验证不足 - paths.rs

**位置**: `cis-core/src/storage/paths.rs`

**问题描述**: 多个函数直接使用用户输入的 `skill_name` 构建路径，没有验证是否包含路径遍历字符。

```rust
// 第 275-282 行
pub fn skill_data_dir(skill_name: &str) -> PathBuf {
    Self::skills_data_dir().join(skill_name)
}

pub fn skill_db(skill_name: &str) -> PathBuf {
    Self::skills_dir().join(format!("{}.db", skill_name))
}
```

**风险分析**:
- 如果 `skill_name` 包含 `../` 或 `..\`，可能导致路径遍历
- 虽然调用方通常使用 `check_string_length` 验证长度，但没有验证路径字符

**受影响函数**:
- `skill_data_dir(skill_name)` - 第 275 行
- `skill_db(skill_name)` - 第 280 行
- `skill_log_file(skill_name)` - 第 297 行

**建议修复**:
```rust
pub fn skill_data_dir(skill_name: &str) -> PathBuf {
    // 验证 skill_name 不包含路径分隔符
    if skill_name.contains('/') || skill_name.contains('\\') || skill_name.contains("..") {
        panic!("Invalid skill name: contains path separators");
    }
    Self::skills_data_dir().join(skill_name)
}
```

**严重程度**: 🟠 中危

---

#### 3. 文件删除操作缺乏验证

**位置**: `cis-core/src/skill/manager.rs`

**问题描述**: `remove` 函数中直接删除文件和目录，没有对路径进行二次验证。

```rust
// 第 699-707 行
let native_path = Paths::skills_native_dir().join(name);
if native_path.exists() {
    std::fs::remove_dir_all(&native_path)?;
}

let wasm_path = Paths::skills_wasm_dir().join(format!("{}.wasm", name));
if wasm_path.exists() {
    std::fs::remove_file(&wasm_path)?;
}
```

**风险分析**:
- `name` 参数经过 `check_string_length` 验证，但没有验证路径字符
- 如果 `name` 为 `../../../etc`，可能导致意外删除

**建议修复**:
在删除前验证 `name` 不包含路径遍历字符：
```rust
fn validate_skill_name(name: &str) -> Result<()> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(CisError::invalid_input("Invalid skill name"));
    }
    Ok(())
}
```

**严重程度**: 🟠 中危

---

### 🟢 低危问题

#### 4. SQL LIKE 模式匹配可能的问题

**位置**: `cis-core/src/storage/memory_db.rs`

**问题描述**: 使用 `LIKE` 查询时构造模式字符串：

```rust
// 第 329 行
let like = format!("{}%", prefix);
```

**风险分析**:
- `LIKE` 模式中的 `%` 和 `_` 有特殊含义
- 如果 `prefix` 包含这些字符，可能影响查询结果
- 这不是安全问题，但可能导致意外的查询行为

**建议**: 考虑对特殊字符进行转义

**严重程度**: 🟢 低危

---

## 正面发现（良好实践）

### ✅ 1. 使用参数化 SQL 查询

**位置**: `cis-core/src/storage/db.rs`, `cis-core/src/storage/memory_db.rs`, `cis-core/src/storage/federation_db.rs`

所有主要的 SQL 操作都使用参数化查询：

```rust
// db.rs 第 225-232 行
self.conn.execute(
    "INSERT INTO core_config (key, value, encrypted, updated_at) 
     VALUES (?1, ?2, ?3, ?4)
     ON CONFLICT(key) DO UPDATE SET 
     value = excluded.value, 
     encrypted = excluded.encrypted,
     updated_at = excluded.updated_at",
    rusqlite::params![key, value, encrypted, now],
)
```

**评估**: 有效防止 SQL 注入攻击 ✅

---

### ✅ 2. 外部命令调用安全

**位置**: `cis-core/src/ai/opencode.rs`, `cis-core/src/ai/claude.rs`, `cis-core/src/ai/kimi.rs`

外部命令调用使用硬编码命令名，用户输入仅作为参数传递：

```rust
// opencode.rs
let mut cmd = Command::new("opencode");
cmd.arg("-p").arg(prompt);
```

**评估**: 命令名硬编码，防止命令注入 ✅

---

### ✅ 3. 字符串长度验证

**位置**: `cis-core/src/lib.rs` 第 276-284 行

实现了统一的字符串长度验证函数：

```rust
pub fn check_string_length(s: &str, max_len: usize) -> Result<()> {
    if s.len() > max_len {
        return Err(CisError::invalid_input(format!(
            "String length {} exceeds maximum allowed {}",
            s.len(), max_len
        )));
    }
    Ok(())
}
```

**使用情况**:
- `cis-core/src/skill/manager.rs` - 验证 skill 名称（256 字符限制）
- `cis-core/src/storage/db.rs` - 验证配置键（1024 字符限制）
- `cis-core/src/storage/db.rs` - 验证 DAG 名称（256 字符限制）

**评估**: 有效防止缓冲区溢出和资源耗尽 ✅

---

### ✅ 4. 沙箱路径验证

**位置**: `cis-core/src/sandbox/mod.rs` 第 189-290 行

实现了完整的路径遍历防护：

```rust
pub fn validate_path(&self, path: &Path) -> std::result::Result<(), SandboxError> {
    // 1. Normalize path
    let normalized = Self::normalize_path(path);
    
    // 2. Check for path traversal attacks
    if Self::contains_path_traversal(path) {
        return Err(SandboxError::PathTraversalDetected(...));
    }
    
    // 3. Check symlink attacks
    if !self.allow_symlinks {
        self.check_symlink_attack(&normalized, 0)?;
    }
    
    // 4. Check if in whitelist
    if self.strict_mode {
        self.is_path_allowed(&normalized)?;
    }
    
    Ok(())
}
```

**路径遍历检测逻辑**:
```rust
fn contains_path_traversal(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    
    // 1. Contains "../" or "..\" 
    if path_str.contains("../") || path_str.contains("..\\") {
        return true;
    }
    
    // 2. Path component starts with ".."
    if path.components().any(|c| {
        c.as_os_str().to_string_lossy().starts_with("..")
    }) {
        return true;
    }
    
    false
}
```

**评估**: 全面的路径安全防护 ✅

---

### ✅ 5. WASM 魔术数字验证

**位置**: `cis-core/src/skill/manager.rs` 第 193 行

```rust
// 验证 WASM 魔术数字
crate::validate_wasm_magic(wasm_bytes)?;

// 检查 WASM 字节码大小
crate::check_allocation_size(wasm_bytes.len(), 128 * 1024 * 1024)?;
```

**评估**: 防止加载恶意 WASM 文件 ✅

---

### ✅ 6. 内存分配限制

**位置**: `cis-core/src/lib.rs` 第 240-255 行

```rust
pub fn check_allocation_size(size: usize, max_size: usize) -> Result<()> {
    if size == 0 {
        return Err(CisError::invalid_input("Allocation size cannot be zero"));
    }
    
    if size > max_size {
        return Err(CisError::invalid_input(format!(
            "Allocation size {} exceeds maximum allowed {}",
            size, max_size
        )));
    }
    
    Ok(())
}
```

**评估**: 防止内存耗尽攻击 ✅

---

### ✅ 7. SQLite 别名验证

**位置**: `cis-core/src/storage/connection.rs` 第 367-390 行

```rust
fn is_valid_alias(alias: &str) -> bool {
    if alias.is_empty() {
        return false;
    }
    
    // 检查第一个字符
    let first = alias.chars().next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    
    // 检查其余字符
    if !alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return false;
    }
    
    // 检查保留关键字
    let reserved = ["main", "temp", "sqlite"];
    if reserved.contains(&alias.to_lowercase().as_str()) {
        return false;
    }
    
    true
}
```

**评估**: 严格的别名验证防止 SQL 注入 ✅

---

## 按文件风险评估

| 文件 | 风险等级 | 主要问题 | 建议操作 |
|------|----------|----------|----------|
| `wizard.rs` | 🟢 低 | 文件操作路径来自受控来源 | 无需立即修复 |
| `manager.rs` | 🟠 中 | skill_name 未验证路径字符 | 添加路径字符验证 |
| `db.rs` | 🟢 低 | 使用参数化查询，安全 | 保持现状 |
| `memory_db.rs` | 🟢 低 | LIKE 模式可能的问题 | 考虑转义特殊字符 |
| `federation_db.rs` | 🟢 低 | 使用参数化查询，安全 | 保持现状 |
| `connection.rs` | 🟠 中 | SQL 语句拼接 | 添加路径验证或参数化处理 |
| `paths.rs` | 🟠 中 | 路径构建未验证输入 | 添加路径字符验证 |
| `sandbox/mod.rs` | 🟢 低 | 完整的路径防护实现 | 保持现状 |

---

## 修复建议优先级

### 高优先级（建议 1-2 周内修复）

1. **添加 skill_name 路径字符验证**
   - 在 `paths.rs` 中所有使用 `skill_name` 的函数添加验证
   - 禁止 `/`, `\`, `..` 等路径遍历字符

2. **修复 connection.rs 中的 SQL 拼接**
   - 对 `path_str` 进行验证或转义
   - 考虑使用参数化的 ATTACH DATABASE（如果 SQLite 支持）

### 中优先级（建议 1 个月内修复）

3. **统一路径验证**
   - 在 `manager.rs` 的删除操作前添加路径验证
   - 考虑复用 `sandbox` 模块的验证逻辑

### 低优先级（可选改进）

4. **LIKE 模式转义**
   - 在 `memory_db.rs` 中对 LIKE 特殊字符进行转义

---

## 总结

CIS 项目在安全性方面整体表现良好，主要安全实践：
- ✅ 参数化 SQL 查询
- ✅ 外部命令硬编码
- ✅ 字符串长度验证
- ✅ 沙箱路径验证
- ✅ WASM 魔术数字验证
- ✅ 内存分配限制

需要改进的方面：
- ⚠️ 部分路径构建未验证用户输入
- ⚠️ 少数 SQL 语句拼接场景

**总体安全评级**: 🟡 **良好** (7/10)

建议优先修复标记为 🟠 中危的问题，以提升整体安全性。

---

## 附录：关键代码引用

### 路径遍历检测实现
```rust
// sandbox/mod.rs 第 273-290 行
fn contains_path_traversal(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    
    if path_str.contains("../") || path_str.contains("..\\") {
        return true;
    }
    
    if path.components().any(|c| {
        c.as_os_str().to_string_lossy().starts_with("..")
    }) {
        return true;
    }
    
    false
}
```

### 安全的参数化查询示例
```rust
// db.rs 第 276-292 行
self.conn.execute(
    "INSERT INTO memory_index 
     (key, skill_name, storage_type, category, created_at, updated_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
     ON CONFLICT(key) DO UPDATE SET
     skill_name = excluded.skill_name,
     storage_type = excluded.storage_type,
     category = excluded.category,
     updated_at = excluded.updated_at",
    rusqlite::params![key, skill_name, storage_type, category, now, now],
)
```

### 安全的命令调用示例
```rust
// ai/opencode.rs
let mut cmd = Command::new("opencode");
cmd.arg("-p").arg(prompt);
```
