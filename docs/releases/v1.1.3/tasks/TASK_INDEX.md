# CIS v1.1.3 任务索引

## 任务总览

| 编号 | 任务名称 | 优先级 | 预估时间 | 依赖 | 状态 |
|-----|---------|-------|---------|------|------|
| T1.1 | mDNS 服务封装 | P0 | 4h | - | 🔴 待分配 |
| T1.2 | QUIC 传输层实现 | P0 | 6h | - | 🔴 待分配 |
| T1.3 | PID 文件管理库 | P0 | 3h | - | 🔴 待分配 |
| T2.1 | P2P Network 状态管理 | P1 | 5h | T1.1, T1.2 | 🔴 待分配 |
| T2.2 | Matrix Server 生命周期 | P1 | 4h | T1.3 | 🔴 待分配 |
| T2.3 | Agent 进程检测器 | P1 | 4h | - | 🔴 待分配 |
| T3.1 | p2p discover 命令 | P1 | 3h | T2.1 | 🔴 待分配 |
| T3.2 | p2p connect/disconnect 命令 | P1 | 3h | T2.1 | 🔴 待分配 |
| T3.3 | matrix start/stop/status 命令 | P1 | 4h | T2.2 | 🔴 待分配 |
| T3.4 | agent status 命令 | P2 | 3h | T2.3 | 🔴 待分配 |
| T4.1 | DHT 真实操作 | P2 | 6h | T2.1 | 🔴 待分配 |
| T4.2 | Federation 事件发送 | P2 | 5h | T2.2 | 🔴 待分配 |
| T4.3 | Embedding 服务替换 | P2 | 4h | - | 🔴 待分配 |

---

## 并行执行组

### 组 1: 基础设施 (可立即并行)
- T1.1: mDNS 服务封装
- T1.2: QUIC 传输层实现
- T1.3: PID 文件管理库
- T2.3: Agent 进程检测器
- T4.3: Embedding 服务替换

### 组 2: 核心服务 (依赖组 1)
- T2.1: P2P Network 状态管理 (需 T1.1, T1.2)
- T2.2: Matrix Server 生命周期 (需 T1.3)

### 组 3: CLI 集成 (依赖组 2)
- T3.1: p2p discover 命令 (需 T2.1)
- T3.2: p2p connect/disconnect 命令 (需 T2.1)
- T3.3: matrix start/stop/status 命令 (需 T2.2)
- T3.4: agent status 命令 (需 T2.3)

### 组 4: 高级功能 (依赖组 2, 3)
- T4.1: DHT 真实操作 (需 T2.1)
- T4.2: Federation 事件发送 (需 T2.2)

---

## 任务依赖图

```
Phase 1 (Foundation)
├── T1.1 mDNS ──────────┐
├── T1.2 QUIC ──────────┼──→ T2.1 P2P Network ───→ T3.1/T3.2 discover/connect
├── T1.3 PID Manager ───┼──→ T2.2 Matrix Lifecycle ─→ T3.3 matrix commands
└── T2.3 Agent Detector ───→ T3.4 agent status

Phase 2 (Advanced)
└── (T2.1 + T2.2) ───→ T4.1 DHT, T4.2 Federation
```

---

## Agent 分配建议

| Agent | 任务 | 技能要求 |
|-------|------|---------|
| **Agent-A** | T1.1, T3.1 | 网络编程、mDNS、局域网发现 |
| **Agent-B** | T1.2, T4.1 | QUIC、P2P 协议、网络传输 |
| **Agent-C** | T1.3, T2.2, T3.3 | 系统编程、进程管理、Unix 信号 |
| **Agent-D** | T2.1, T3.2 | Rust async、架构设计、系统集成 |
| **Agent-E** | T2.3, T3.4 | 系统信息、进程检测、跨平台 |
| **Agent-F** | T4.2, T4.3 | Matrix 协议、机器学习、向量嵌入 |

---

## 任务详情目录

```
plan/tasks/
├── TASK_INDEX.md                    # 本文件
├── T1.1_mdns_service/
│   └── README.md                    # mDNS 服务封装
├── T1.2_quic_transport/
│   └── README.md                    # QUIC 传输层
├── T1.3_pid_manager/
│   └── README.md                    # PID 文件管理
├── T2.1_p2p_network/
│   └── README.md                    # P2P Network 状态管理
├── T2.2_matrix_lifecycle/
│   └── README.md                    # Matrix Server 生命周期
├── T2.3_agent_detector/
│   └── README.md                    # Agent 进程检测器
├── T3.1_p2p_discover_cmd/
│   └── README.md                    # p2p discover 命令
├── T3.2_p2p_connect_cmd/
│   └── README.md                    # p2p connect/disconnect 命令
├── T3.3_matrix_cmd/
│   └── README.md                    # matrix start/stop/status 命令
├── T3.4_agent_status_cmd/
│   └── README.md                    # agent status 命令
├── T4.1_dht_operations/
│   └── README.md                    # DHT 真实操作
├── T4.2_federation_events/
│   └── README.md                    # Federation 事件发送
└── T4.3_embedding_service/
    └── README.md                    # Embedding 服务替换
```

---

## 执行规则

### 1. 独立执行原则
- 每个任务必须在无上下文的情况下可执行
- 任务文档必须包含完整的接口定义
- 不允许引用"如之前的实现"

### 2. 接口契约
- 每个任务必须明确定义输入/输出接口
- 依赖的任务必须通过接口使用，不直接访问内部

### 3. 测试要求
- 每个任务必须有单元测试 (>80% 覆盖)
- 集成测试在依赖任务完成后执行

### 4. 禁止事项
- ❌ 模拟/假数据
- ❌ placeholder 实现
- ❌ 硬编码
- ❌ sleep 假装在工作

---

## 验收流程

1. **自测**: 任务执行者运行单测
2. **提交**: PR 到主分支
3. **集成**: 合并后运行集成测试
4. **端到端**: 多节点组网验证

---

## 时间表

| 阶段 | 任务 | 预计完成 |
|-----|------|---------|
| Week 1 | 组 1 (基础设施) | Day 3-4 |
| Week 1 | 组 2 (核心服务) | Day 5-7 |
| Week 2 | 组 3 (CLI 集成) | Day 3-5 |
| Week 2 | 组 4 (高级功能) | Day 7 |
| Week 3 | 集成测试 + Bug 修复 | Day 7 |

---

## 关键路径

**最长依赖链**:
```
T1.1/T1.2 → T2.1 → T3.1/T3.2 → (集成测试)
     ↓
   T4.1
```

**关键路径时间**: 4h + 5h + 3h = **12h 最小交付时间**

---

## 联系与协调

- 技术问题: 在任务目录创建 `QUESTIONS.md`
- 接口变更: 必须同步更新依赖任务
- 每日同步: 汇报进度和阻塞问题
