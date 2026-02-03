# Skill 开发指南

## 概述

CIS Skill 可以是:
- **Native Skill**: Rust 实现的本地 Skill
- **WASM Skill**: WebAssembly 格式的跨平台 Skill

## 快速开始

### 1. 定义 Skill 语义

```rust
use cis_core::skill::semantics::{SkillSemantics, SkillIoSignature, SkillScope};

impl MySkill {
    fn semantics() -> SkillSemantics {
        SkillSemantics {
            skill_id: "my-skill".to_string(),
            skill_name: "My Skill".to_string(),
            description: "描述 Skill 的功能".to_string(),
            example_intents: vec![
                "执行操作 A".to_string(),
                "执行操作 B".to_string(),
            ],
            parameter_schema: Some(json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            })),
            io_signature: Some(SkillIoSignature {
                input_types: vec!["data".to_string()],
                output_types: vec!["result".to_string()],
                pipeable: true,
                source: false,
                sink: false,
            }),
            scope: SkillScope::Global,
        }
    }
}
```

### 2. 实现 Skill Trait

```rust
use cis_core::skill::Skill;
use cis_core::skill::context::SkillContext;
use cis_core::skill::event::Event;

#[async_trait]
impl Skill for MySkill {
    fn name(&self) -> &str {
        "my-skill"
    }
    
    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => {
                // 处理自定义事件
            }
            _ => {}
        }
        Ok(())
    }
}
```

### 3. 使用 Host API

WASM Skill 可以调用 Host 提供的 API:

```rust
// 读取记忆
let value = host_memory_get("key");

// 写入记忆
host_memory_set("key", b"value");

// 调用 AI
let response = host_ai_chat("prompt");

// 记录日志
host_log(1, "Info message");
```

## Skill Chain 兼容性

要使 Skill 可以参与 Chain 编排:

1. 设置 `pipeable = true`
2. 明确定义 `input_types` 和 `output_types`
3. 使用标准数据格式 (JSON)

## 测试

```bash
# 加载 Skill
cis skill load my-skill

# 测试 Skill
cis skill do "执行操作" --candidates

# 查看日志
cis telemetry logs
```
