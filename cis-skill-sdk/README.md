# CIS Skill SDK

CIS Skill 开发工具包 - 统一 Skill 开发框架

## 特性

- **双模式支持**: Native (完整功能) 和 WASM (沙箱安全)
- **统一接口**: Skill trait 定义清晰的开发契约
- **Host API**: 丰富的系统调用接口（记忆、AI、HTTP、日志等）
- **IM 扩展**: 专为即时通讯 Skill 设计的类型和 API
- **AI 集成**: 便捷的 AI 调用封装
- **Derive 宏**: 简化 Skill 定义

## 快速开始

### 添加依赖

```toml
[dependencies]
cis-skill-sdk = { path = "../cis-skill-sdk", features = ["native"] }
```

### 创建一个简单的 Skill

```rust
use cis_skill_sdk::{Skill, SkillContext, Event, Result};

pub struct HelloSkill;

impl Skill for HelloSkill {
    fn name(&self) -> &str {
        "hello"
    }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        if let Event::Custom { name, data } = event {
            if name == "greet" {
                ctx.log_info(&format!("Hello, {:?}!", data));
            }
        }
        Ok(())
    }
}
```

### IM Skill 示例

```rust
use cis_skill_sdk::{Skill, Event, Result};
use cis_skill_sdk::im::{ImContextExt, ImMessage, ImMessageBuilder};

pub struct EchoSkill;

impl Skill for EchoSkill {
    fn name(&self) -> &str { "echo" }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        if let Event::Custom { name, data } = event {
            if name == "im:message" {
                let msg: ImMessage = serde_json::from_value(data)?;
                
                // 回复消息
                let reply = ImMessageBuilder::text("收到！")
                    .to(&msg.from)
                    .reply_to(&msg.id)
                    .build();
                
                ctx.im_send(&reply)?;
            }
        }
        Ok(())
    }
}
```

## 模块说明

| 模块 | 说明 |
|------|------|
| `skill` | Skill trait 和 SkillContext 定义 |
| `types` | 通用类型（Event, MemoryEntry, HttpRequest 等） |
| `host` | Host API 接口（Native/WASM 双模式） |
| `im` | IM 专用接口（消息、用户、群组等） |
| `ai` | AI 调用封装 |
| `error` | 错误类型定义 |

## 编译目标

### Native 模式

```bash
cargo build --features native
```

### WASM 模式

```bash
cargo build --target wasm32-unknown-unknown --features wasm
```

## 目录结构

```
cis-skill-sdk/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs          # 主入口
│   ├── skill.rs        # Skill trait
│   ├── types.rs        # 类型定义
│   ├── host.rs         # Host API
│   ├── im.rs           # IM 接口
│   ├── ai.rs           # AI 接口
│   └── error.rs        # 错误处理
├── cis-skill-sdk-derive/  # Derive 宏
│   └── src/lib.rs
└── examples/
    └── hello_skill.rs  # 示例
```

## 许可证

MIT
