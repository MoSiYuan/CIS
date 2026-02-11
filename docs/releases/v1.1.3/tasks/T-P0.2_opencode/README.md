# T-P0.2: OpenCode 多轮对话模拟

**优先级**: 🔴 P0  
**预估时间**: 2h  
**依赖**: -  
**分配**: Agent-A

---

## 问题描述

当前通过 prompt 注入模拟多轮对话，而非使用真实的 OpenCode session。

**问题文件**: `cis-core/src/ai/opencode.rs:100`

**当前代码**:
```rust
// 通过 prompt 注入模拟多轮对话
let prompt = format!("{previous_context}\nUser: {new_message}\nAssistant:");
```

---

## 修复方案

使用 OpenCode CLI 的 session 功能实现真实多轮对话:

```rust
pub struct OpenCodeSession {
    session_id: String,
    history: Vec<Message>,
}

impl OpenCodeSession {
    pub async fn chat(&mut self, message: &str) -> Result<String> {
        // 使用 opencode continue -c <session_id>
        // 或 opencode resume <session_id>
        let output = Command::new("opencode")
            .arg("continue")
            .arg("-c")
            .arg(&self.session_id)
            .arg("--")
            .arg(message)
            .output()
            .await?;
        
        let response = String::from_utf8_lossy(&output.stdout);
        self.history.push(Message::assistant(&response));
        
        Ok(response.to_string())
    }
}
```

---

## 验收标准

- [ ] 支持真实的多轮对话上下文
- [ ] 不使用 prompt 注入模拟
- [ ] session 持久化
