# T2.2: Matrix Server 生命周期管理

**任务编号**: T2.2  
**任务名称**: Matrix Server Lifecycle Management  
**优先级**: P1  
**预估时间**: 4h  
**依赖**: T1.3 (PID Manager)  
**分配状态**: 待分配

---

## 任务概述

实现 Matrix Server 的真实启动、停止和状态管理。

---

## 输入

### 依赖任务输出
- **T1.3**: `PidManager`

### 待修改文件
- `cis-node/src/commands/matrix.rs` (TODO: line 139, 155)

### 当前问题
```rust
// TODO: Implement PID file tracking and graceful shutdown
// TODO: Check if server is running via PID file
```

---

## 输出要求

### 必须实现的接口

```rust
// 文件: cis-core/src/matrix/server_manager.rs (新建)

use crate::system::pid_manager::{PidManager, ProcessSignal};

pub struct MatrixServerManager {
    pid_manager: PidManager,
    config: MatrixConfig,
}

#[derive(Debug, Clone)]
pub struct MatrixConfig {
    pub port: u16,
    pub bind_addr: String,
}

#[derive(Debug, Clone)]
pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub uptime_secs: Option<u64>,
}

impl MatrixServerManager {
    pub fn new(config: MatrixConfig) -> Self;
    
    /// 启动 Matrix 服务
    /// 阻塞直到启动成功或失败
    pub async fn start(&self) -> Result<ServerHandle>;
    
    /// 停止服务
    pub async fn stop(&self) -> Result<()>;
    
    /// 获取状态
    pub fn status(&self) -> ServerStatus;
    
    /// 重启服务
    pub async fn restart(&self) -> Result<ServerHandle>;
}

pub struct ServerHandle {
    pub pid: u32,
    pub port: u16,
}
```

---

## 验收标准

- [ ] start 后进程真实启动
- [ ] PID 文件正确写入
- [ ] status 显示真实状态（不是 "Unknown"）
- [ ] stop 发送 SIGTERM 终止进程
- [ ] 重复 start 给出友好提示

---

## 输出文件

```
cis-core/src/matrix/
├── server_manager.rs    # 主要实现
└── mod.rs               # 添加导出
```

---

## 阻塞关系

**依赖**:
- T1.3: PidManager

**阻塞**:
- T3.3: matrix start/stop/status 命令
