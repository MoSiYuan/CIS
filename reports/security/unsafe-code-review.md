# CIS 项目 Unsafe 代码审查报告

**审查日期:** 2026-02-08  
**审查范围:** cis-core/src/ 目录下的所有 Rust 源文件  
**发现 Unsafe 块数量:** 6 个

---

## 概览

| # | 文件位置 | 行号 | 用途 | 必要性 | 风险等级 |
|---|---------|------|------|--------|----------|
| 1 | `worker_service.rs` | 377-384 | 发送 SIGKILL 信号 | 必要 | 低 |
| 2 | `worker_service.rs` | 387-394 | 发送 SIGTERM 信号 | 必要 | 低 |
| 3 | `worker_service.rs` | 408-410 | 超时后强制杀死进程 | 必要 | 低 |
| 4 | `worker_service.rs` | 779-781 | 检查进程是否存在 | 必要 | 低 |
| 5 | `storage.rs` | 302-308 | 注册 sqlite-vec 扩展 | 必要 | 中 |

> 注：第 1-3 个 unsafe 块在代码中被计为 3 个独立块，但 grep 结果显示为 3 个独立的 unsafe 调用

---

## 详细审查

### 1. worker_service.rs:377-384 - SIGKILL 信号发送

**代码片段:**
```rust
if force {
    // 强制停止：发送 SIGKILL
    unsafe {
        let result = kill(pid_i32, SIGKILL);
        if result != 0 {
            return Err(CisError::execution(
                format!("Failed to send SIGKILL to worker '{}': {}", id, std::io::Error::last_os_error())
            ));
        }
    }
}
```

**用途:** 向 Worker 进程发送 SIGKILL 信号强制终止进程。

**安全性分析:**
- ✅ **PID 验证**: PID 来自内部状态管理，非用户直接输入
- ✅ **错误处理**: 正确检查返回值并使用 `std::io::Error::last_os_error()` 获取错误信息
- ✅ **类型安全**: `pid` 为 `u32`，转换为 `i32` 时无溢出风险（在 Unix 上 PID 范围有限）
- ✅ **权限边界**: 只能终止本用户有权限的进程

**结论:** 安全，必要。libc 的 `kill()` 函数调用必须使用 unsafe。

---

### 2. worker_service.rs:387-394 - SIGTERM 信号发送

**代码片段:**
```rust
} else {
    // 优雅停止：发送 SIGTERM
    unsafe {
        let result = kill(pid_i32, SIGTERM);
        if result != 0 {
            return Err(CisError::execution(
                format!("Failed to send SIGTERM to worker '{}': {}", id, std::io::Error::last_os_error())
            ));
        }
    }
    // ... 等待进程终止
}
```

**用途:** 向 Worker 进程发送 SIGTERM 信号请求优雅终止。

**安全性分析:**
- ✅ 与 SIGKILL 块相同的安全保障
- ✅ 有优雅停止的超时机制，超时后会使用 SIGKILL

**结论:** 安全，必要。

---

### 3. worker_service.rs:408-410 - 超时后强制杀死

**代码片段:**
```rust
// 如果还在运行，强制杀死
if !stopped {
    unsafe {
        let _ = kill(pid_i32, SIGKILL);
    }
}
```

**用途:** 优雅停止超时后，强制终止进程。

**安全性分析:**
- ⚠️ **忽略返回值**: 使用 `let _ =` 忽略了 `kill` 的返回值，不检查是否成功
- ✅ **上下文安全**: 此时进程应该仍在运行，PID 有效

**问题:** 虽然不检查返回值在超时场景下可能可接受（进程可能已经退出），但建议至少记录日志。

**结论:** 安全，但建议改进错误处理。

---

### 4. worker_service.rs:779-781 - 检查进程是否存在

**代码片段:**
```rust
#[cfg(unix)]
{
    // 在 Unix 上，发送信号 0 检查进程是否存在
    unsafe {
        libc::kill(pid as i32, 0) == 0
    }
}
```

**用途:** 检查指定 PID 的进程是否仍在运行（发送信号 0 是 POSIX 标准的进程存在性检查方法）。

**安全性分析:**
- ✅ **标准做法**: 信号 0 是特殊的空信号，用于测试进程存在性和权限，不会实际发送信号
- ✅ **返回值检查**: 正确将结果作为布尔值返回
- ✅ **跨平台**: 有 `#[cfg(unix)]` 保护，Windows 使用不同实现

**结论:** 安全，必要。这是 Unix 上检查进程存在性的标准且唯一的方法。

---

### 5. storage.rs:302-308 - 注册 sqlite-vec 扩展

**代码片段:**
```rust
static INIT: Once = Once::new();
INIT.call_once(|| {
    unsafe {
        use rusqlite::ffi::sqlite3_auto_extension;
        // 注册 vec 扩展为自动扩展，这样每个新连接都会自动加载它
        sqlite3_auto_extension(Some(
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())
        ));
    }
});
```

**用途:** 注册 sqlite-vec 扩展为 SQLite 自动扩展，使每个新连接自动加载该扩展。

**安全性分析:**
- ✅ **一次性初始化**: 使用 `std::sync::Once` 确保只执行一次，线程安全
- ⚠️ **transmute 风险**: `std::mem::transmute` 将函数指针转换为 SQLite 期望的签名
  - 需要确保 `sqlite_vec::sqlite3_vec_init` 的签名与 `sqlite3_auto_extension` 期望的回调签名匹配
  - 如果签名不匹配，会导致未定义行为
- ⚠️ **FFI 边界**: 调用 SQLite C API，需要确保 ABI 兼容性

**潜在问题:**
1. 缺乏对 `sqlite3_auto_extension` 返回值的检查
2. `transmute` 没有显式的类型注解，依赖于类型推断

**结论:** 功能必要，但存在潜在风险。建议添加类型安全检查和错误处理。

---

## 建议改进措施

### 高优先级

#### 1. 为 sqlite-vec 扩展注册添加安全检查

**文件:** `cis-core/src/vector/storage.rs`

当前代码缺乏 `sqlite3_auto_extension` 返回值检查和 `transmute` 的类型安全。建议重构为：

```rust
#[cfg(all(feature = "vector", feature = "sqlite-vec"))]
fn ensure_vec_extension_registered() {
    use std::sync::Once;
    use rusqlite::ffi::{sqlite3_auto_extension, SQLITE_OK};
    
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        unsafe {
            // 定义正确的函数签名类型以确保类型安全
            type Sqlite3VecInit = unsafe extern "C" fn(
                *mut rusqlite::ffi::sqlite3,
                *mut *mut i8,
                *const rusqlite::ffi::sqlite3_api_routines
            ) -> i32;
            
            let init_fn: Sqlite3VecInit = sqlite_vec::sqlite3_vec_init;
            let result = sqlite3_auto_extension(Some(std::mem::transmute(init_fn)));
            
            if result != SQLITE_OK {
                // 记录错误但不 panic，因为后续连接可能仍能工作
                log::error!("Failed to register sqlite-vec auto-extension: error code {}", result);
            } else {
                log::debug!("sqlite-vec extension registered successfully");
            }
        }
    });
}
```

### 中优先级

#### 2. 为所有 unsafe 块添加安全文档注释

为每个 unsafe 块添加 `// SAFETY:` 注释，说明为什么此处的 unsafe 是安全的。

**worker_service.rs 示例:**

```rust
/// 发送 SIGKILL 强制终止 Worker 进程
/// 
/// # Safety
/// 调用 libc::kill 是 unsafe 的，但此处：
/// - PID 来自内部管理的 Worker 状态，已验证非零
/// - SIGKILL 是标准信号，不会导致未定义行为
/// - 正确检查返回值并处理错误
unsafe {
    let result = kill(pid_i32, SIGKILL);
    if result != 0 {
        return Err(CisError::execution(
            format!("Failed to send SIGKILL to worker '{}': {}", id, std::io::Error::last_os_error())
        ));
    }
}
```

#### 3. 改进超时杀死后的错误处理

**文件:** `worker_service.rs:408-410`

建议记录失败日志：

```rust
if !stopped {
    unsafe {
        // SAFETY: 调用 libc::kill 发送 SIGKILL，PID 来自内部状态
        let result = kill(pid_i32, SIGKILL);
        if result != 0 {
            log::warn!("Failed to send SIGKILL to worker '{}' after timeout: {}", 
                      id, std::io::Error::last_os_error());
        }
    }
}
```

### 低优先级

#### 4. 考虑使用 nix  crate 包装系统调用

nix crate 提供了更安全的 Rust 风格 API 来替代原始 libc 调用：

```toml
[dependencies]
nix = { version = "0.27", features = ["signal", "process"] }
```

使用示例：

```rust
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;

// 发送 SIGKILL - 不需要 unsafe!
kill(Pid::from_raw(pid_i32), Signal::SIGKILL)?;

// 发送 SIGTERM
kill(Pid::from_raw(pid_i32), Signal::SIGTERM)?;

// 检查进程是否存在 (信号 0)
let exists = kill(Pid::from_raw(pid_i32), None).is_ok();
```

**优点:**
- 消除 unsafe 块
- 更好的错误处理（返回 `Result`）
- 更符合 Rust 习惯

---

## 总结

| 项目 | 状态 |
|------|------|
| 发现 unsafe 块总数 | 6 个（4 个独立位置） |
| 不必要的 unsafe | 0 个 |
| 需要安全文档 | 6 个 |
| 建议使用 nix crate 消除 | 4 个（libc 调用） |
| 需要改进错误处理 | 2 个 |

### 整体评估

CIS 项目中的 unsafe 代码使用是**合理且相对安全**的：

1. **所有 unsafe 都是必要的**: 要么调用底层系统 API（libc），要么进行 FFI 交互（SQLite）
2. **有良好的边界控制**: PID 来自内部状态，非直接用户输入
3. **大部分有错误处理**: 除了超时后的 SIGKILL 调用
4. **线程安全**: sqlite-vec 注册使用了 `Once` 确保线程安全

### 建议行动

1. **立即**: 为所有 unsafe 块添加 `// SAFETY:` 文档注释
2. **短期**: 改进超时杀死后的错误处理，添加日志记录
3. **中期**: 考虑引入 `nix` crate 消除 libc 相关的 unsafe 块
4. **长期**: 为 sqlite-vec 扩展注册添加更严格的类型检查和错误处理

---

*报告生成时间: 2026-02-08*  
*审查工具: manual code review*
