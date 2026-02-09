# T1.3: PID 文件管理库

**任务编号**: T1.3  
**任务名称**: PID File Manager  
**优先级**: P0 (最高)  
**预估时间**: 3 小时  
**依赖**: 无  
**分配状态**: 待分配

---

## 任务概述

实现跨平台的 PID 文件管理库，用于守护进程的启动、停止和状态查询。

---

## 输入

### 依赖
- `libc` crate (Unix 信号)
- `sysinfo` crate (进程检测)

### 平台
- Linux
- macOS

---

## 输出要求

### 必须实现的接口

```rust
// 文件: cis-core/src/system/pid_manager.rs (新建)

use std::path::PathBuf;
use anyhow::Result;

/// PID 文件管理器
pub struct PidManager {
    name: String,
    pid_file: PathBuf,
}

/// 进程信号类型
#[derive(Debug, Clone, Copy)]
pub enum ProcessSignal {
    /// 优雅关闭 (SIGTERM)
    Term,
    /// 强制关闭 (SIGKILL)
    Kill,
    /// 重新加载配置 (SIGHUP)
    Hup,
}

/// 进程状态
#[derive(Debug, Clone)]
pub struct ProcessStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub start_time: Option<std::time::SystemTime>,
}

impl PidManager {
    /// 创建 PID 管理器
    /// 
    /// # Arguments
    /// * `name` - 服务名称 (用于生成 PID 文件名)
    pub fn new(name: &str) -> Self {
        // 实现...
    }
    
    /// 获取 PID 文件路径
    fn pid_file_path(name: &str) -> PathBuf {
        // Linux: /run/user/{uid}/{name}.pid 或 ~/.local/run/{name}.pid
        // macOS: ~/Library/Run/{name}.pid
    }
    
    /// 写入当前进程 PID
    /// 
    /// 原子写入，确保完整性
    pub fn write(&self) -> Result<()> {
        // 实现...
    }
    
    /// 读取 PID 并验证进程是否存在
    /// 
    /// # Returns
    /// Some(pid) - 进程存在
    /// None - 进程不存在或 PID 文件不存在
    pub fn read(&self) -> Result<Option<u32>> {
        // 实现...
    }
    
    /// 检查进程是否运行
    pub fn is_running(&self) -> bool {
        // 实现...
    }
    
    /// 获取进程状态
    pub fn status(&self) -> ProcessStatus {
        // 实现...
    }
    
    /// 发送信号给管理的进程
    /// 
    /// # Arguments
    /// * `signal` - 信号类型
    /// 
    /// # Returns
    /// 是否成功发送信号
    pub fn signal(&self, signal: ProcessSignal) -> Result<bool> {
        // 实现...
    }
    
    /// 优雅关闭进程
    /// 
    /// 先发送 SIGTERM，超时后发送 SIGKILL
    pub fn stop(&self, timeout: std::time::Duration) -> Result<bool> {
        // 实现...
    }
    
    /// 清理 PID 文件
    pub fn cleanup(&self) -> Result<()> {
        // 实现...
    }
}

impl Drop for PidManager {
    /// 可选：Drop 时自动清理
    /// 注意：通常不应在 Drop 中清理，因为进程可能故意保持运行
    fn drop(&mut self) {}
}
```

---

## 技术规格

### PID 文件位置

| 平台 | 路径 |
|-----|------|
| Linux (有 XDG) | `$XDG_RUNTIME_DIR/cis-{name}.pid` |
| Linux (无 XDG) | `~/.local/run/cis-{name}.pid` |
| macOS | `~/Library/Run/cis-{name}.pid` |

### PID 文件格式
```
{pid}\n
{timestamp}\n
{executable_path}\n
```

示例:
```
12345
1707456000
/usr/local/bin/cis-node
```

---

## 实现步骤

1. **创建目录**
   - 确保 PID 文件目录存在

2. **实现 write**
   - 原子写入（先写临时文件再重命名）
   - 写入 PID + 时间戳 + 可执行路径

3. **实现 read**
   - 读取 PID 文件
   - 验证进程是否存在 (/proc/{pid} 或 sysinfo)
   - 验证可执行路径匹配（防 PID 复用）

4. **实现 signal**
   - Unix: 使用 `libc::kill(pid, signal)`
   - 处理权限错误

5. **实现 stop**
   - 发送 SIGTERM
   - 轮询等待进程退出
   - 超时后发送 SIGKILL

6. **添加测试**
   - 使用子进程模拟测试

---

## 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Child};
    
    fn start_test_process() -> Child {
        Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap()
    }
    
    #[test]
    fn test_write_and_read() {
        let manager = PidManager::new("test-write");
        manager.write().unwrap();
        
        let pid = manager.read().unwrap();
        assert_eq!(pid, Some(std::process::id()));
        
        // 清理
        manager.cleanup().unwrap();
    }
    
    #[test]
    fn test_is_running() {
        let mut child = start_test_process();
        
        let manager = PidManager::new("test-running");
        std::fs::write(&manager.pid_file, child.id().to_string()).unwrap();
        
        assert!(manager.is_running());
        
        child.kill().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        assert!(!manager.is_running());
    }
    
    #[test]
    fn test_stop() {
        let mut child = start_test_process();
        let manager = PidManager::new("test-stop");
        std::fs::write(&manager.pid_file, child.id().to_string()).unwrap();
        
        let stopped = manager.stop(std::time::Duration::from_secs(5)).unwrap();
        assert!(stopped);
    }
}
```

---

## 验收标准

- [ ] 写入后能正确读取 PID
- [ ] 进程不存在时返回 None（不是 panic）
- [ ] 支持优雅关闭 (SIGTERM) 和强制关闭 (SIGKILL)
- [ ] 超时机制正常工作
- [ ] 单测覆盖率 > 80%
- [ ] 跨平台兼容 (Linux/macOS)

---

## 参考文档

- [libc crate](https://docs.rs/libc)
- [sysinfo crate](https://docs.rs/sysinfo)
- Unix 信号: `man 7 signal`

---

## 输出文件

```
cis-core/src/system/
├── pid_manager.rs       # 主要实现
├── mod.rs               # 添加导出
└── tests/
    └── pid_manager_test.rs
```

---

## 阻塞关系

**阻塞**:
- T2.2: Matrix Server 生命周期管理
- T3.3: matrix start/stop/status 命令
