# T2.3: Agent 进程检测器

**任务编号**: T2.3  
**任务名称**: Agent Process Detector  
**优先级**: P1  
**预估时间**: 4h  
**依赖**: 无  
**分配状态**: 待分配

---

## 任务概述

实现真实的 Agent 进程检测，支持 Claude/OpenCode/Kimi。

---

## 输入

### 待修改文件
- `cis-core/src/agent/persistent/opencode.rs:569`
- `cis-core/src/agent/persistent/claude.rs:577-579`

### 当前问题
```rust
// TODO: 实现进程扫描或端口检测
last_active_at: s.created_at, // TODO: 从 session 获取
total_tasks: 0, // TODO: 从持久化存储获取
```

---

## 输出要求

```rust
pub struct AgentProcessDetector;

impl AgentProcessDetector {
    pub fn detect(agent_type: AgentType) -> Vec<AgentProcessInfo>;
    pub fn is_running(pid: u32) -> bool;
    pub fn get_sessions(agent_type: AgentType) -> Vec<AgentSession>;
}

pub enum AgentType { Claude, OpenCode, Kimi }

pub struct AgentProcessInfo {
    pub pid: u32,
    pub agent_type: AgentType,
    pub working_dir: PathBuf,
    pub start_time: SystemTime,
}
```

---

## 验收标准

- [ ] 正确识别运行中的 Agent 进程
- [ ] 返回准确的 PID、启动时间、工作目录
- [ ] 支持 macOS 和 Linux
- [ ] 单测覆盖率 > 80%

---

## 阻塞关系

**阻塞**:
- T3.4: agent status 命令
