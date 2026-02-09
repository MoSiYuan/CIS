# Agent-E 任务分配

**Agent 标识**: Agent-E  
**任务**: T2.3 + T3.4  
**技能要求**: 系统信息、进程检测、跨平台  
**优先级**: P1/P2  
**预估总时间**: 7 小时

---

## 任务清单

### 任务 1: T2.3 - Agent 进程检测器
**文件**: `plan/tasks/T2.3_agent_detector/README.md`  
**时间**: 4h  
**状态**: 🔴 立即开始（无依赖）

**核心目标**:
- 实现真实的 Agent 进程检测
- 支持 Claude/OpenCode/Kimi
- 跨平台 (Linux/macOS)

**关键接口**:
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
    pub port: Option<u16>,
}
```

---

### 任务 2: T3.4 - agent status 命令
**文件**: `plan/tasks/T3.4_agent_status_cmd/README.md`  
**时间**: 3h  
**状态**: 🔴 等待 T2.3 完成后开始

**核心目标**:
- 实现 `cis agent status` 命令
- 显示真实的 Agent 状态

**输出格式**:
```
📊 Agent Status
═══════════════

Claude:
  🟢 Running (PID: 12345)
  📁 Working dir: /Users/xxx/.cis/agents/claude-xxx
  ⏱️  Started: 2026-02-09 10:00:00
  
OpenCode:
  🔴 Not running
  💡 Start with: cis agent start opencode
```

---

## 执行顺序

```
┌─────────────────────────────────────────────────────┐
│  1. T2.3 (4h)                                        │
│     - 实现 AgentProcessDetector                     │
│     - 实现进程检测逻辑                              │
│     - 支持 macOS 和 Linux                           │
│     - 提交 PR                                        │
│                                                      │
│     ↓                                                │
│                                                      │
│  2. T3.4 (3h)                                        │
│     - 实现 agent status 命令                        │
│     - 格式化输出                                    │
│     - 提交 PR                                        │
└─────────────────────────────────────────────────────┘
```

---

## 进程检测方法

### macOS
```bash
ps aux | grep claude
# 或
lsof -i :port
```

### Linux
```bash
cat /proc/{pid}/cmdline
cat /proc/{pid}/cwd
```

### Rust 实现
```rust
use sysinfo::{System, ProcessExt, SystemExt};

let s = System::new_all();
for (pid, process) in s.processes() {
    if process.name().contains("claude") {
        // 找到进程
    }
}
```

---

## 验收标准

### T2.3 验收
- [ ] 正确识别运行中的 Agent 进程
- [ ] 返回准确的 PID、启动时间、工作目录
- [ ] 支持 macOS 和 Linux
- [ ] 单测覆盖率 > 80%

### T3.4 验收
- [ ] 显示真实运行的 Agent
- [ ] 标记僵尸进程 (stale)
- [ ] 统计信息准确

---

## 特殊考虑

### Claude 进程识别
- 进程名: `claude` 或 `Claude`
- 命令行参数可能包含 `--session`
- 可能有多个实例（不同 session）

### OpenCode 进程识别
- 进程名: `opencode`
- 可能监听 HTTP 端口

### Kimi 进程识别
- 进程名: `kimi`
- 可能有多个子进程

---

## 依赖关系

**依赖你的 Agent**:
- T3.4 (你) - 使用 T2.3 的接口

**你依赖的 Agent**:
- 无（T2.3 可立即开始）

---

## 开始工作

1. 阅读: `plan/tasks/T2.3_agent_detector/README.md`
2. 创建分支: `git checkout -b agent-e/t2.3-detector`
3. 开始实现进程检测器

---

**祝你好运！**
