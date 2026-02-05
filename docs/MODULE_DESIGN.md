# CIS-DAG 模块划分确认

## 类型定义位置

### cis-core/src/scheduler/mod.rs（✅ 正确）
```rust
pub enum DagScope { ... }          // 核心：作用域定义
pub struct DagTaskSpec { ... }     // API：任务规格
pub struct DagSpec { ... }         // API：DAG规格
```

**理由**：
- GLM API 接收 HTTP 请求 → 需要解析 `DagSpec`
- DAG Skill 处理事件 → 需要使用 `DagScope`
- CLI 创建 DAG → 需要构造 `DagSpec`
- 避免循环依赖（core 不依赖 skill）

## 使用关系

```
cis-core/src/scheduler/mod.rs
    │
    ├── cis-core/src/glm/mod.rs          # GLM API 接收 DagSpec
    │   pub async fn publish_dag(req: DagSpec) -> Result<...>
    │
    ├── skills/dag-executor/src/lib.rs   # DAG Skill 使用 DagScope
    │   impl Skill for DagExecutor {
    │       fn handle(&self, spec: DagSpec) { ... }
    │   }
    │
    └── cis-node/src/commands/dag.rs     # CLI 构造 DagSpec
        pub fn create_dag(file: PathBuf) -> Result<()> {
            let spec: DagSpec = serde_json::from_str(&content)?;
        }
```

## 后续模块

### skills/dag-executor/（Task 1.3 创建）
```rust
// 不定义类型，只使用
use cis_core::scheduler::{DagSpec, DagScope};

pub struct DagExecutor;
impl Skill for DagExecutor {
    fn handle_trigger(&self, trigger: SkillTrigger) {
        let spec: DagSpec = parse_trigger(trigger);
        let worker_id = spec.worker_id();  // 使用 DagScope
        ...
    }
}
```

**确认**：当前类型定义位置正确，无需调整。
