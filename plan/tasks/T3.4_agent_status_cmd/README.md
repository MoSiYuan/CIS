# T3.4: agent status 命令

**任务编号**: T3.4  
**任务名称**: Agent Status Command  
**优先级**: P2  
**预估时间**: 3h  
**依赖**: T2.3 (Agent Detector)  
**分配状态**: 待分配

---

## 任务概述

实现 `cis agent status` 命令，显示真实的 Agent 状态。

---

## 输入

### 依赖任务输出
- **T2.3**: `AgentProcessDetector`

---

## 输出格式

```
📊 Agent Status
═══════════════

Claude:
  🟢 Running (PID: 12345)
  📁 Working dir: /Users/xxx/.cis/agents/claude-xxx
  ⏱️  Started: 2026-02-09 10:00:00
  
OpenCode:
  🔴 Not running
```

---

## 验收标准

- [ ] 显示真实运行的 Agent
- [ ] 标记僵尸进程 (stale)
- [ ] 统计信息准确

---

## 阻塞关系

**依赖**:
- T2.3: AgentProcessDetector
