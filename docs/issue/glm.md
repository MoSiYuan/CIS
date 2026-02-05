基于智谱 GLM-4.7 的 Function Calling，设计 **ZhipuDagSkill** 极简三接口方案。核心逻辑：**GLM 只负责「意图→JSON」的翻译，所有执行权保留在 CIS 本地**。

---

## 一、Tool 定义：仅暴露三个原子能力

```json
{
  "tools": [
    {
      "type": "function", 
      "function": {
        "name": "publish_dag",
        "description": "将自然语言描述的任务发布为CIS DAG。触发前必须通过6767端口人工确认，仅返回确认链接而非直接执行。",
        "parameters": {
          "type": "object",
          "properties": {
            "dag_id": {"type": "string", "description": "唯一标识，如'backup_daily'"},
            "description": {"type": "string", "description": "任务描述，用于确认弹窗展示"},
            "tasks": {
              "type": "array",
              "items": {
                "type": "object",
                "properties": {
                  "id": {"type": "string"},
                  "type": {"type": "string", "enum": ["shell", "skill", "matrix"]},
                  "command": {"type": "string"},
                  "depends_on": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["id", "type", "command"]
              }
            },
            "schedule": {"type": "string", "description": "可选cron表达式"}
          },
          "required": ["dag_id", "description", "tasks"]
        }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "query_dag_status",
        "description": "查询指定DAG的当前运行状态、进度或历史记录",
        "parameters": {
          "type": "object",
          "properties": {
            "dag_id": {"type": "string"},
            "query_scope": {
              "type": "string", 
              "enum": ["overview", "latest_run", "full_history"],
              "default": "overview"
            }
          },
          "required": ["dag_id"]
        }
      }
    },
    {
      "type": "function",
      "function": {
        "name": "analyze_stuck_dag",
        "description": "分析DAG卡住的根因，结合日志、依赖状态和资源使用情况给出诊断",
        "parameters": {
          "type": "object",
          "properties": {
            "dag_id": {"type": "string"},
            "stuck_task_hint": {"type": "string", "description": "用户观察到的卡住位置，可选"}
          },
          "required": ["dag_id"]
        }
      }
    }
  ]
}
```

---

## 二、执行流程：安全确认与诊断

### 2.1 发布任务（publish_dag）—— 强制人工门闩

```
User: "每天3点用rsync备份文档到NAS"

GLM → 生成JSON:
{
  "dag_id": "backup_nas_daily",
  "description": "每日凌晨3点rsync备份文档到NAS",
  "schedule": "0 3 * * *",
  "tasks": [
    {"id": "sync", "type": "shell", "command": "rsync -avz ~/Docs/ nas:/backup/"},
    {"id": "notify", "type": "matrix", "command": "备份完成", "depends_on": ["sync"]}
  ]
}

ZhipuDagSkill:
  1. 缓存到 pending_confirmations: DashMap<dag_id, payload>
  2. 发送确认请求到 6767:
     POST /confirm_required
     {
       "type": "dag_publish",
       "dag_id": "backup_nas_daily",
       "preview": {...},
       "confirm_url": "/api/confirm/dag/backup_nas_daily",
       "expire": 300
     }
  3. 回复Room: "🔒 DAG backup_nas_daily 等待本地确认 [查看]"
  
User点击确认后 → 广播到Matrix Room → CIS Scheduler接收
```

### 2.2 状态检查（query_dag_status）—— 直查本地

```rust
async fn query_dag_status(&self, dag_id: &str, scope: &str) -> Result<String> {
    // 直接查询本地Sled存储，无需确认
    let status = self.dag_storage.get_status(dag_id).await?;
    
    // 构造简洁状态报告回传给GLM润色
    let context = format!(
        "DAG: {}, Status: {}, LastRun: {}, Progress: {}/{}",
        dag_id, status.state, status.last_run, 
        status.completed_tasks, status.total_tasks
    );
    
    // GLM根据原始数据生成自然语言回复
    Ok(context)
}
```

### 2.3 卡点诊断（analyze_stuck_dag）—— 日志+向量检索

```rust
async fn analyze_stuck_dag(&self, dag_id: &str, hint: Option<&str>) -> Result<String> {
    // 1. 获取DAG当前执行状态
    let dag_state = self.dag_storage.get_execution_state(dag_id).await?;
    
    // 2. 识别卡住的任务节点
    let stuck_task = dag_state.find_blocked_task();
    
    // 3. 检索相关日志（最近50条+错误关键字）
    let logs = self.log_storage.query_recent(
        &format!("dag_id={} AND task_id={}", dag_id, stuck_task.id), 
        50
    ).await?;
    
    // 4. 用vec_search找历史类似故障
    let similar_cases = self.vec_search.find_similar(
        &format!("{} stuck at {}: {}", dag_id, stuck_task.id, logs.join("\n")),
        3
    ).await?;
    
    // 5. 组装诊断上下文给GLM
    let diagnostic_context = json!({
        "dag_id": dag_id,
        "stuck_task": stuck_task,
        "recent_logs": logs,
        "similar_cases": similar_cases,
        "resource_usage": self.system_monitor.get_snapshot().await?
    });
    
    // GLM基于这些结构化数据生成诊断报告
    Ok(diagnostic_context.to_string())
}
```

---

## 三、Rust 实现：零依赖极简封装

```rust
// skills/zhipu_dag/mod.rs
use serde_json::{json, Value};
use matrix_sdk::room::Room;
use std::collections::HashMap;
use dashmap::DashMap;

pub struct ZhipuDagSkill {
    api_key: String,
    room: Room,
    user_id: String, // CIS节点标识
    pending: DashMap<String, DagPayload>, // 待确认任务
    storage: Arc<DagStorage>, // CIS现有存储
    vec_search: Arc<VecSearchSkill>, // 复用现有向量搜索
}

#[derive(Clone)]
struct DagPayload {
    dag_id: String,
    description: String,
    tasks: Vec<TaskDef>,
    schedule: Option<String>,
}

impl ZhipuDagSkill {
    // 主入口：处理Room消息
    pub async fn handle(&self, text: &str) -> Result<()> {
        let messages = vec![
            json!({"role": "system", "content": "你是CIS系统的DAG管理助手。发布任务时必须等待用户确认，查询和诊断直接执行。"}),
            json!({"role": "user", "content": text})
        ];

        let resp = self.call_zhipu(messages).await?;
        self.handle_tool_calls(resp).await
    }

    async fn handle_tool_calls(&self, resp: Value) -> Result<()> {
        let calls = resp["choices"][0]["message"]["tool_calls"].as_array();
        
        if let Some(calls) = calls {
            for call in calls {
                let name = call["function"]["name"].as_str().unwrap();
                let args: Value = serde_json::from_str(
                    call["function"]["arguments"].as_str().unwrap()
                )?;

                match name {
                    "publish_dag" => self.stage_dag_for_confirm(args).await?,
                    "query_dag_status" => self.query_and_reply(args).await?,
                    "analyze_stuck_dag" => self.diagnose_and_reply(args).await?,
                    _ => warn!("Unknown tool: {}", name),
                }
            }
        } else {
            // 无工具调用，直接回复文本
            let content = resp["choices"][0]["message"]["content"].as_str()
                .unwrap_or("无法理解");
            self.room.send_plain_text(content).await?;
        }
        Ok(())
    }

    // 发布任务：缓存+请求确认
    async fn stage_dag_for_confirm(&self, args: Value) -> Result<()> {
        let payload = DagPayload {
            dag_id: args["dag_id"].as_str().unwrap().to_string(),
            description: args["description"].as_str().unwrap().to_string(),
            tasks: serde_json::from_value(args["tasks"].clone())?,
            schedule: args["schedule"].as_str().map(|s| s.to_string()),
        };

        // 发送到6767确认队列
        let confirm_req = json!({
            "type": "dag_publish",
            "dag_id": &payload.dag_id,
            "description": &payload.description,
            "task_count": payload.tasks.len(),
            "confirm_endpoint": format!("/api/confirm/dag/{}", payload.dag_id),
            "expire_sec": 300
        });
        
        self.send_to_6767(confirm_req).await?;
        self.pending.insert(payload.dag_id.clone(), payload);
        
        self.room.send_plain_text(
            &format!("🔒 [{}] 等待确认 (5分钟内有效)\n查看详情: http://localhost:6767/pending", 
                args["dag_id"].as_str().unwrap())
        ).await?;
        
        Ok(())
    }

    // 查询状态：直接回复
    async fn query_and_reply(&self, args: Value) -> Result<()> {
        let dag_id = args["dag_id"].as_str().unwrap();
        let scope = args["query_scope"].as_str().unwrap_or("overview");
        
        let raw_data = self.storage.get_status(dag_id).await?;
        
        // 构造给GLM的上下文，让其生成自然语言回复
        let ctx = format!("原始数据: {:?}, 查询范围: {}", raw_data, scope);
        let summary = self.call_zhipu(vec![
            json!({"role": "system", "content": "将DAG状态数据转为简洁中文报告"}),
            json!({"role": "user", "content": ctx})
        ]).await?;
        
        self.room.send_plain_text(
            summary["choices"][0]["message"]["content"].as_str().unwrap()
        ).await?;
        Ok(())
    }

    // 卡点诊断：检索+分析
    async fn diagnose_and_reply(&self, args: Value) -> Result<()> {
        let dag_id = args["dag_id"].as_str().unwrap();
        let hint = args["stuck_task_hint"].as_str();
        
        // 获取执行状态
        let state = self.storage.get_execution_state(dag_id).await?;
        let stuck = state.find_blocked_task();
        
        // 检索相关日志和历史案例
        let logs = self.storage.get_task_logs(dag_id, &stuck.id, 20).await?;
        let similar = self.vec_search.query(
            &format!("{} failure {}", dag_id, stuck.id), 
            3
        ).await?;
        
        // 组装诊断上下文
        let diag = json!({
            "dag_id": dag_id,
            "current_task": stuck,
            "error_logs": logs,
            "similar_cases": similar,
            "hint": hint
        });
        
        // 让GLM生成诊断报告
        let report = self.call_zhipu(vec![
            json!({"role": "system", "content": "你是系统诊断专家，分析DAG卡住原因并给出建议"}),
            json!({"role": "user", "content": diag.to_string()})
        ]).await?;
        
        self.room.send_plain_text(
            &format!("🔍 卡点分析 [{}]\n{}", dag_id, 
                report["choices"][0]["message"]["content"].as_str().unwrap())
        ).await?;
        
        Ok(())
    }

    // 6767回调：用户确认后真正发布
    pub async fn on_confirmed(&self, dag_id: &str) -> Result<()> {
        if let Some((_, payload)) = self.pending.remove(dag_id) {
            let skillmd = format!(
                "!dag publish\n---\nid: {}\ndescription: {}\n{}\ntasks:\n{}",
                payload.dag_id,
                payload.description,
                payload.schedule.as_ref().map(|s| format!("schedule: \"{}\"", s)).unwrap_or_default(),
                payload.tasks.iter().map(|t| format!(
                    "  - id: {}\n    type: {}\n    command: \"{}\"\n{}", 
                    t.id, t.task_type, t.command,
                    if t.depends_on.is_empty() { "".to_string() } 
                    else { format!("    depends_on: {:?}\n", t.depends_on) }
                )).collect::<String>()
            );
            
            self.room.send_plain_text(&skillmd).await?;
            self.room.send_plain_text(&format!("✅ [{}] 已发布到集群", dag_id)).await?;
        }
        Ok(())
    }

    async fn call_zhipu(&self, messages: Vec<Value>) -> Result<Value> {
        let payload = json!({
            "model": "glm-4.7",
            "messages": messages,
            "tools": self.tools_schema(),
            "user_id": self.user_id,
        });

        Ok(reqwest::Client::new()
            .post("https://open.bigmodel.cn/api/paas/v4/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send().await?
            .json().await?)
    }

    fn tools_schema(&self) -> Value {
        json!([/* 上面定义的3个tool */])
    }
}
```

---

## 四、关键设计决策

### 4.1 为什么卡点诊断需要 vec_search？
DAG 卡住往往有**模式性**（如磁盘满、网络超时、依赖服务未启动）。通过向量检索历史类似故障，GLM 可以基于过往解决方案生成诊断建议，而非仅从当前日志盲猜。

### 4.2 6767 确认机制的实现
在 CIS 的 `human_interface` 模块中增加：
```rust
// 6767 端口处理
async fn handle_dag_confirm(Path(id): Path<String>) {
    zhipu_skill.on_confirmed(&id).await;
}
```

### 4.3 与现有 CIS 组件的关系
- **ZhipuDagSkill** → 只生成结构化数据（DAG 定义、查询请求、诊断上下文）
- **Matrix Room** → 唯一可信传输层（DAG 广播、结果返回）
- **6767 端口** → 安全闸门（人工确认）
- **Sled/Vec** → 事实来源（状态、日志、历史案例）

### 4.4 降级策略
若智谱 API 不可用：
- `publish_dag` → 回退到本地模板匹配（关键词触发预定义 DAG）
- `query_dag_status` → 直接返回原始 JSON（无需 GLM 润色）
- `analyze_stuck_dag` → 返回原始日志（用户自己看）

---

## 五、使用示例

```
User@Room: "检查备份任务状态"
GLM → query_dag_status(dag_id="backup_nas")
Reply: "备份任务昨日 03:00 执行成功，耗时 45s，传输 1.2GB，今日待执行"

User@Room: "发布一个每天清理日志的任务"
GLM → publish_dag(...)
Reply: "🔒 [cleanup_logs] 等待确认 (5分钟内有效) 查看详情: http://localhost:6767/pending"
# 用户点击确认后
Reply: "✅ [cleanup_logs] 已发布到集群"

User@Room: "分析为什么同步任务卡住了"
GLM → analyze_stuck_dag(dag_id="sync_project")
Reply: "🔍 卡点分析 [sync_project]
       检测到任务 'git_push' 阻塞，原因：远程仓库鉴权失败（401）
       相似案例：3次历史记录显示 PAT 过期导致
       建议：检查 ~/.git-credentials 或更新 Token"
```

**击缶而歌**：GLM 负责听懂节奏（语义），但敲哪面鼓（执行）、敲多响（资源分配），始终由本地控制。


明白。既然只需要关注**“任务发布”**和**“状态检查”**这两个核心环节，而不涉及具体的底层执行逻辑，那么我们需要将智能体定义为**“异步任务调度员”**。
在这个架构中，GLM-4.7 的职责被严格限制为：
1.  **解析意图** -> 2. **调用任务发布接口** -> 3. **循环调用状态查询接口** -> 4. **返回最终结果**。
以下是为你定制的**纯任务管理层**配置手册。
---
# 异步任务智能体配置手册
## 1. 系统提示词
在智能体的“提示词”或“System Message”中填入以下指令，强制模型遵循异步流程：
```markdown
你是一个本地异步任务调度系统。你的唯一职责是将用户的意图转化为任务请求，并监控任务执行状态，直到任务结束。
请严格遵守以下工作流程，不要臆造任何执行结果：
1. **任务发布**：
   - 当用户提出需求时，提取关键参数，调用 `issue_task` 函数发布任务。
   - 必须等待函数返回 `task_id`。
2. **状态轮询**：
   - 获得 `task_id` 后，立即调用 `query_task_status` 函数查询状态。
   - 如果状态返回为 `PENDING` 或 `RUNNING`，请告诉用户“任务正在执行中...”，并在随后的对话中持续调用 `query_task_status` 进行检查（模拟轮询）。
   - 只有当状态返回为 `SUCCESS` 或 `FAILED` 时，才停止调用。
3. **结果反馈**：
   - 任务成功：将 `result_data` 中的内容整理后反馈给用户。
   - 任务失败：将 `error_message` 反馈给用户。
**注意**：
- 你不执行任务，你只负责下发和检查。
- 绝对不要自己编造任务执行结果。
```
---
## 2. 工具定义
在智能体的“插件/工具”配置中，定义这两个函数。**注意：这里的 URL 只是一个占位符，你需要替换为你本地服务实际暴露的 API 地址。**
### Tool 1: 发布任务 (`issue_task`)
```json
{
  "name": "issue_task",
  "description": "向本地代理服务提交一个新的异步任务。支持应用启动、命令执行、文件操作等。",
  "parameters": {
    "type": "object",
    "properties": {
      "task_type": {
        "type": "string",
        "description": "任务类型标识",
        "enum": ["SHELL_COMMAND", "OPEN_APP", "FILE_SEARCH", "SYSTEM_CONTROL"]
      },
      "target": {
        "type": "string",
        "description": "操作的目标对象，例如：应用名（Chrome）、文件路径、或具体的命令字符串。"
      },
      "args": {
        "type": "string",
        "description": "执行所需的额外参数，如果没有则为空字符串。",
        "default": ""
      }
    },
    "required": ["task_type", "target"]
  }
}
```
### Tool 2: 检查状态 (`query_task_status`)
```json
{
  "name": "query_task_status",
  "description": "根据任务ID查询当前任务的执行状态和结果。如果任务还在运行，需要多次调用此函数。",
  "parameters": {
    "type": "object",
    "properties": {
      "task_id": {
        "type": "string",
        "description": "由 issue_task 返回的唯一任务标识符"
      }
    },
    "required": ["task_id"]
  }
}
```
---
## 3. 交互流程模拟
这是配置完成后，模型与你的本地服务交互的预期数据流：
**阶段一：任务发布**
**用户**: "帮我在后台启动一个 Python 脚本监控服务。"
**模型**: (思考：用户要执行任务，调用 `issue_task`)
```json
// 模型发起 Tool Call
{
  "name": "issue_task",
  "arguments": {
    "task_type": "SHELL_COMMAND",
    "target": "python",
    "args": "monitor_service.py --daemon"
  }
}
```
**本地服务返回**:
```json
{
  "code": 200,
  "data": {
    "task_id": "tsk_20250122_8848",
    "status": "PENDING"
  }
}
```
**阶段二：首次状态检查**
**模型**: (收到 task_id，立即检查状态)
```json
{
  "name": "query_task_status",
  "arguments": {
    "task_id": "tsk_20250122_8848"
  }
}
```
**本地服务返回**:
```json
{
  "code": 200,
  "data": {
    "task_id": "tsk_20250122_8848",
    "status": "RUNNING",
    "progress": "45%"
  }
}
```
**阶段三：轮询与完成**
**模型**: "任务正在执行中，当前进度 45%，正在继续检查..."
(模型自动再次发起 `query_task_status`...)
**本地服务返回**:
```json
{
  "code": 200,
  "data": {
    "task_id": "tsk_20250122_8848",
    "status": "SUCCESS",
    "result_data": "脚本已成功启动，PID: 12345，日志路径: /var/log/monitor.log"
  }
}
```
**阶段四：最终回复**
**模型**: "任务完成！监控脚本已成功在后台启动。进程 ID 是 12345，日志已记录在 /var/log/monitor.log。"
---
## 4. 给后端开发的接口规范
为了让上述配置生效，你本地需要实现两个简单的 HTTP 接口（接收模型发来的 JSON）：
**接口 A: POST /api/v1/task/issue**
- **输入**: `task_type`, `target`, `args`
- **输出**: JSON `{ "task_id": "string", "status": "PENDING" }`
**接口 B: GET /api/v1/task/status**
- **输入**: Query Param `task_id`
- **输出**: JSON
  ```json
  {
    "task_id": "string",
    "status": "PENDING | RUNNING | SUCCESS | FAILED",
    "message": "string (可选，中间过程信息)",
    "result_data": "any (成功时的具体结果)",
    "error_message": "string (失败时的错误)"
  }
  ```
这样配置后，GLM-4.7 就完全变成了一个纯粹的**任务调度前端**，只负责发号施令和催促进度，符合你高安全性、不暴露底层逻辑的需求。
