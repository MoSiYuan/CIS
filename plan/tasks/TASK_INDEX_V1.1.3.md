# CIS v1.1.3 剩余任务索引

> **状态**: Phase 1 & 2 完成，Phase 3 进行中  
> **任务数**: 9 个 (P0: 2, P1: 7)  

---

## 🔴 P0 - 核心功能模拟 (必须修复)

| 任务 | 模块 | 预估时间 | 分配 | 状态 |
|------|------|---------|------|------|
| [T-P0.1](./T-P0.1/README.md) | AI/Embedding | 4h | Agent-F | ✅ 完成 |
| [T-P0.2](./T-P0.2_opencode/README.md) | OpenCode | 2h | Agent-A | ⏳ |

---

## 🟡 P1 - 重要功能不完整

| 任务 | 模块 | 预估时间 | 分配 | 状态 |
|------|------|---------|------|------|
| [T-P1.1](./T-P1.1_cors/README.md) | Matrix CORS | 2h | Agent-C | ⏳ |
| [T-P1.2](./T-P1.2_udp/README.md) | Matrix UDP | 6h | Agent-D | ⏳ |
| [T-P1.3](./T-P1.3_challenge/README.md) | Matrix Challenge | 4h | Agent-B | ⏳ |
| [T-P1.4](./T-P1.4_mdns/README.md) | Matrix mDNS | 2h | Agent-E | ⏳ |
| [T-P1.5](./T-P1.5_scheduler/README.md) | Scheduler | 3h | Agent-F | ⏳ |
| [T-P1.6](./T-P1.6_quota/README.md) | Matrix Cloud | 3h | Agent-A | ⏳ |
| [T-P1.7](./T-P1.7_federation/README.md) | Federation | 2h | Agent-B | ⏳ |

---

## 依赖关系

```
T-P0.1 (Embedding)
  └─ 使用 embedding_service.rs ✅ 已完成

T-P0.2 (OpenCode)
  └─ 无依赖

T-P1.1 (CORS)
  └─ MatrixConfig

T-P1.2 (UDP)
  └─ P2PNetwork UDP

T-P1.3 (Challenge)
  └─ Noise protocol

T-P1.4 (mDNS)
  └─ MdnsService ✅ 已完成

T-P1.5 (Scheduler)
  └─ tokio::sync::mpsc

T-P1.6 (Quota)
  └─ Cloud API

T-P1.7 (Federation)
  └─ FederationClient ✅ 已完成
```

---

## 并行策略

### 无依赖任务 (可立即开始)
- T-P0.1, T-P0.2
- T-P1.1, T-P1.4, T-P1.5

### 依赖其他库
- T-P1.2 (Noise protocol)
- T-P1.3 (Noise protocol)

---

## 验收检查

```bash
# 检查是否还有模拟代码
grep -rn "模拟\|mock\|stub\|placeholder" --include="*.rs" cis-core/src cis-node/src | grep -v "test\|Test" | wc -l

# 期望: 0
```
