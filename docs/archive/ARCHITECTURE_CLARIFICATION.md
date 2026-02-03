# CIS 与 AgentFlow 架构关系澄清

## 核心认知错误

**错误理解**: CIS 作为 AgentFlow 的 Skill 安装
**正确理解**: CIS 是独立的分布式系统，与 AgentFlow 平行存在

```
┌─────────────────────────────────────────────────────────┐
│                    用户工作流                            │
├─────────────────────────────────────────────────────────┤
│                                                         │
│   ┌───────────────┐         ┌───────────────┐          │
│   │   AgentFlow   │         │     CIS       │          │
│   │   (Master-    │         │   (P2P 节点)  │          │
│   │    Worker)    │         │               │          │
│   │               │         │  • 私域记忆    │          │
│   │  • 任务调度    │         │  • P2P通信    │          │
│   │  • LLM集成    │         │  • 硬件绑定    │          │
│   │  • 云端同步   │         │  • 零Token    │          │
│   │               │         │               │          │
│   │  Skills:      │         │  Skills:      │          │
│   │  • AI执行器   │         │  • AI整理     │          │
│   │  • 记忆整理   │         │  • 监控告警   │          │
│   │  • 推送通知   │         │  • IM接入     │          │
│   └───────┬───────┘         └───────┬───────┘          │
│           │                         │                  │
│           │    (可选：API/CLI桥接)    │                  │
│           └───────────┬─────────────┘                  │
│                       │                                │
│                  ┌────┴────┐                          │
│                  │ 用户选择 │                          │
│                  │ 使用哪个 │                          │
│                  └─────────┘                          │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

## 正确关系

### 1. 平行项目（推荐）

```
~/projects/
├── AgentFlow/          # 原版系统（Master-Worker）
│   ├── 适合：云端部署、集中管理
│   └── skills/         # 轻量级功能扩展
│       ├── ai-executor/
│       ├── memory-organizer/
│       └── push-client/
│
└── CIS/                # 新版系统（P2P节点）
    ├── 适合：本地优先、硬件绑定
    └── skills/         # 独立生态
        ├── ai-organizer/
        ├── monitor/
        └── im-gateway/
```

### 2. 迁移的本质

**不是**: AgentFlow → CIS (替换)
**而是**: AgentFlow 经验 → CIS (新建)

CIS 借鉴 AgentFlow 的代码经验，但是：
- 独立的代码库
- 独立的架构设计
- 独立的 skill 生态

## 重新规划

### AgentFlow（保持现状）

**定位**: 成熟的 Master-Worker 任务调度系统

**保留功能**:
- Master-Worker 架构
- 云端记忆同步
- 丰富的 Skill 生态
- Web Dashboard

**适用场景**:
- 团队协作
- 云端部署
- 需要集中管理

### CIS（新建系统）

**定位**: 极简的 P2P 分布式节点系统

**核心功能**:
- P2P 通信（无中心）
- 硬件绑定身份
- 本地加密存储
- 零 Token 设计

**适用场景**:
- 个人本地使用
- 隐私敏感场景
- 去中心化需求

## 代码复用策略

```
AgentFlow 代码
    │
    ├── 直接复用（复制到新项目）
    │   ├── db/migration.rs → cis-core/src/storage/migration.rs
    │   └── memory/index.rs → cis-core/src/storage/index.rs
    │
    ├── 改造后使用
    │   ├── project.rs → node/init.rs（无project_id）
    │   └── push/client.rs → p2p/transfer.rs（P2P改造）
    │
    └── 废弃
        ├── cloud_sync/（CIS禁止云端同步）
        └── master/（CIS无中心节点）
```

## 当前工作调整

### 已完成（正确方向）

✅ 分析了 AgentFlow 代码
✅ 设计了 CIS 架构
✅ 提取了可复用代码

### 需要调整

❌ 不应该试图把 CIS 作为 AgentFlow Skill
✅ CIS 是独立项目，独立仓库

### AgentFlow 的最终状态

- 保持 v0.4.0-refactor 分支
- 所有功能已完成
- 准备推送到 origin
- **不依赖 CIS**

### CIS 的后续开发

- 在独立仓库进行
- 参考 AgentFlow 代码，但独立实现
- 自己的 skill 系统（WASM 或 Native）

## 总结

1. **AgentFlow** 和 **CIS** 是两个独立项目
2. AgentFlow 继续维护（Master-Worker 架构）
3. CIS 是新项目（P2P 架构）
4. 代码经验可以借鉴，但不是依赖关系
5. 用户根据场景选择使用哪个系统

**AgentFlow 推送**: 准备就绪，独立推送
**CIS 开发**: 独立进行，不阻塞 AgentFlow
