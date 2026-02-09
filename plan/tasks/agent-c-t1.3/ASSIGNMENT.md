# Agent-C 任务分配

**Agent 标识**: Agent-C  
**任务**: T1.3 + T2.2 + T3.3  
**技能要求**: 系统编程、进程管理、Unix 信号  
**优先级**: P0/P1  
**预估总时间**: 11 小时

---

## 任务清单

### 任务 1: T1.3 - PID 文件管理库
**文件**: `plan/tasks/T1.3_pid_manager/README.md`  
**时间**: 3h  
**状态**: 🔴 立即开始（无依赖）

**核心目标**:
- 实现跨平台 PID 文件管理
- 支持 Linux/macOS
- 进程启动、停止、状态查询

**关键接口**:
```rust
impl PidManager {
    pub fn new(name: &str) -> Self;
    pub fn write(&self) -> Result<()>;
    pub fn read(&self) -> Result<Option<u32>>;
    pub fn is_running(&self) -> bool;
    pub fn signal(&self, signal: ProcessSignal) -> Result<bool>;
    pub fn stop(&self, timeout: Duration) -> Result<bool>;
    pub fn cleanup(&self) -> Result<()>;
}
```

**输出文件**:
- `cis-core/src/system/pid_manager.rs`
- `cis-core/src/system/tests/pid_manager_test.rs`

---

### 任务 2: T2.2 - Matrix Server 生命周期管理
**文件**: `plan/tasks/T2.2_matrix_lifecycle/README.md`  
**时间**: 4h  
**状态**: 🔴 等待 T1.3 完成后开始

**核心目标**:
- 实现 Matrix Server 的真实启动/停止
- 使用 PidManager 管理进程
- 修复 `TODO: PID file tracking`

**关键接口**:
```rust
impl MatrixServerManager {
    pub fn new(config: MatrixConfig) -> Self;
    pub async fn start(&self) -> Result<ServerHandle>;
    pub async fn stop(&self) -> Result<()>;
    pub fn status(&self) -> ServerStatus;
}
```

---

### 任务 3: T3.3 - matrix start/stop/status 命令
**文件**: `plan/tasks/T3.3_matrix_cmd/README.md`  
**时间**: 4h  
**状态**: 🔴 等待 T2.2 完成后开始

**核心目标**:
- 替换 `cis-node/src/commands/matrix.rs` 中的 TODO
- 实现真实的 start/stop/status
- 显示真实 PID 和状态

---

## 执行顺序

```
┌─────────────────────────────────────────────────────┐
│  1. T1.3 (3h)                                        │
│     - 实现 PidManager                               │
│     - 支持 SIGTERM/SIGKILL                          │
│     - 编写单元测试                                  │
│     - 提交 PR                                        │
│                                                      │
│     ↓                                                │
│                                                      │
│  2. T2.2 (4h)                                        │
│     - 使用 PidManager 管理 Matrix Server            │
│     - 实现 start/stop/status                        │
│     - 提交 PR                                        │
│                                                      │
│     ↓                                                │
│                                                      │
│  3. T3.3 (4h)                                        │
│     - 替换 matrix 命令实现                          │
│     - 显示真实状态                                  │
│     - 提交 PR                                        │
└─────────────────────────────────────────────────────┘
```

---

## 协作接口

**你提供的接口**:
```rust
// T1.3 完成后：
pub use cis_core::system::pid_manager::{PidManager, ProcessSignal, ProcessStatus};

// T2.2 完成后：
pub use cis_core::matrix::server_manager::{MatrixServerManager, ServerStatus};
```

**你依赖的接口**:
- 无（T1.3 是基础设施）

---

## 关键平台差异

### PID 文件位置
| 平台 | 路径 |
|-----|------|
| Linux | `~/.local/run/cis-{name}.pid` |
| macOS | `~/Library/Run/cis-{name}.pid` |

### 信号发送
```rust
// Unix 使用 libc::kill
libc::kill(pid, libc::SIGTERM)
```

---

## 验收标准

### T1.3 验收
- [ ] 写入后能正确读取 PID
- [ ] 进程不存在时返回 None
- [ ] 优雅关闭 (SIGTERM) 和强制关闭 (SIGKILL) 都工作
- [ ] 超时机制正常
- [ ] 跨平台兼容

### T2.2 验收
- [ ] start 后进程真实启动
- [ ] PID 文件正确写入
- [ ] status 显示真实状态
- [ ] stop 发送信号终止进程

### T3.3 验收
- [ ] start 启动真实进程
- [ ] stop 终止进程
- [ ] status 显示 PID、端口、运行时间

---

## 开始工作

1. 阅读: `plan/tasks/T1.3_pid_manager/README.md`
2. 创建分支: `git checkout -b agent-c/t1.3-pid`
3. 开始实现 PID 管理器

---

**祝你好运！**
