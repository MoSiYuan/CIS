# CIS GUI 模块设计方案

## 1. 设计目标

解决 Element 客户端的局限：
- ❌ Element 无法配置 CIS 网络节点
- ❌ Element 无法管理本地 Skill/Agent 配置
- ❌ Element 无法查看节点状态和联邦连接

提供一体化界面：
- ✅ 节点可视化管理和切换
- ✅ 集成 Claude/Kimi Code 交互
- ✅ 本地记忆浏览和搜索
- ✅ 网络拓扑可视化

---

## 2. 技术选型

### 方案对比

| 方案 | 体积 | 开发效率 | 跨平台 | 维护成本 | 推荐度 |
|------|------|---------|--------|---------|--------|
| **Tauri** | 3-5MB | ⭐⭐⭐ 高 | ✅ | ⭐⭐ 低 | ⭐⭐⭐ 首选 |
| egui | 2MB | ⭐⭐ 中 | ✅ | ⭐⭐⭐ 高 | 备选 |
| Iced | 5MB | ⭐⭐ 中 | ✅ | ⭐⭐ 中 | 备选 |
| Electron | 100MB+ | ⭐⭐⭐ 高 | ✅ | ⭐ 高 | ❌ 排除 |

### 推荐：Tauri 架构

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS GUI (Tauri)                         │
├─────────────────────────────────────────────────────────────┤
│  Frontend (WebView)          │  Backend (Rust)              │
│  ─────────────────           │  ─────────────               │
│  • React/Vue UI              │  • cis-core 库               │
│  • 节点标签页组件             │  • Matrix 联邦客户端          │
│  • IM 聊天界面               │  • P2P 网络管理              │
│  • Agent 交互面板            │  • Skill 管理器              │
│  • 记忆浏览器                │  • 本地 SQLite 存储          │
├─────────────────────────────────────────────────────────────┤
│  IPC 通信 (Command/Event)                                   │
└─────────────────────────────────────────────────────────────┘
```

**选择理由**：
- 前端技术栈成熟，UI 开发效率高
- Rust 后端直接复用 `cis-core`
- 打包体积小（3-5MB），启动快
- 原生系统托盘、通知、快捷键支持

---

## 3. 架构设计

### 3.1 模块结构

```
cis-gui/                          # 新建 GUI 模块
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs              # 入口
│   │   ├── commands/            # IPC 命令处理
│   │   │   ├── node.rs          # 节点管理命令
│   │   │   ├── chat.rs          # IM 聊天命令
│   │   │   ├── agent.rs         # Agent 交互命令
│   │   │   └── memory.rs        # 记忆管理命令
│   │   ├── state.rs             # 应用状态管理
│   │   └── tray.rs              # 系统托盘
│   └── Cargo.toml
├── src/                          # 前端 (React)
│   ├── components/
│   │   ├── NodeTabs/            # 节点标签页（如图）
│   │   ├── ChatPanel/           # 聊天面板
│   │   ├── AgentPanel/          # Agent 交互面板
│   │   ├── MemoryBrowser/       # 记忆浏览器
│   │   └── NetworkGraph/        # 网络拓扑图
│   ├── stores/
│   │   ├── nodeStore.ts         # 节点状态管理
│   │   ├── chatStore.ts         # 聊天记录管理
│   │   └── agentStore.ts        # Agent 状态管理
│   └── App.tsx
└── package.json
```

### 3.2 IPC 接口设计

```rust
// src-tauri/src/commands/node.rs

/// 获取所有已知节点
#[tauri::command]
async fn get_nodes() -> Result<Vec<NodeInfo>, String> {
    // 调用 cis-core 获取节点列表
}

/// 添加静态节点
#[tauri::command]
async fn add_node(address: String, name: Option<String>) -> Result<(), String> {
    // 添加节点到配置
}

/// 切换当前活动节点
#[tauri::command]
async fn switch_node(node_id: String) -> Result<(), String> {
    // 切换 Matrix 客户端连接到指定节点
}

/// 获取节点连接状态
#[tauri::command]
async fn get_node_status(node_id: String) -> Result<NodeStatus, String> {
    // 返回在线/离线状态、延迟等
}

/// 测试节点连通性
#[tauri::command]
async fn ping_node(address: String) -> Result<u64, String> {
    // 返回 RTT (ms)
}
```

```rust
// src-tauri/src/commands/chat.rs

/// 获取房间列表
#[tauri::command]
async fn get_rooms() -> Result<Vec<Room>, String> {}

/// 获取房间消息历史
#[tauri::command]
async fn get_room_messages(room_id: String, limit: u32) -> Result<Vec<Message>, String> {}

/// 发送消息
#[tauri::command]
async fn send_message(room_id: String, content: String) -> Result<String, String> {}

/// 监听新消息（通过 Tauri Event）
#[tauri::command]
async fn listen_messages(window: Window) -> Result<(), String> {
    // 设置回调，收到新消息时向前端发送事件
}
```

```rust
// src-tauri/src/commands/agent.rs

/// 执行 Agent 提示
#[tauri::command]
async fn execute_prompt(provider: String, prompt: String, session: Option<String>) 
    -> Result<StreamResponse, String> {}

/// 获取可用 Agent 列表
#[tauri::command]
async fn list_agents() -> Result<Vec<AgentInfo>, String> {}

/// 语义搜索记忆
#[tauri::command]
async fn search_memory(query: String, limit: u32) -> Result<Vec<MemoryItem>, String> {}
```

---

## 4. UI 设计

### 4.1 主界面布局

```
┌─────────────────────────────────────────────────────────────────┐
│  CIS GUI                                          [_] [□] [×]  │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    Chat / Agent Panel                    │   │
│  │                                                          │   │
│  │  ┌─────────────┐                                         │   │
│  │  │ Claude      │  Hello! How can I help you today?       │   │
│  │  │             │                                         │   │
│  │  │ [Code]      │  ```rust                                │   │
│  │  │             │  fn main() {                            │   │
│  │  └─────────────┘      println!("Hello CIS!");            │   │
│  │                       ```                               │   │
│  │                                                         │   │
│  │  ┌─────────────┐                                         │   │
│  │  │ User        │  Analyze this code for me               │   │
│  │  │             │                                         │   │
│  │  └─────────────┘                                         │   │
│  │                                                          │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  [Munin-macmini●] [Hugin-pc○] [Hugin-mbp○] [Munin-cloud○] [+] │
│                                           [🚀 Send]            │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 组件设计

#### 节点标签页 (NodeTabs)

参考用户提供的图片设计：

```typescript
// src/components/NodeTabs/NodeTabs.tsx

interface NodeTab {
  id: string;           // 节点 ID
  name: string;         // 显示名称
  address: string;      // host:port
  status: 'online' | 'offline' | 'connecting';
  unreadCount?: number; // 未读消息数
  isActive: boolean;    // 当前选中
}

// 视觉样式
// - 在线: ● 绿色圆点
// - 离线: ○ 灰色圆点
// - 选中: 橙色背景 (如图)
// - 未选中: 灰色背景
```

**交互**：
- 点击标签切换当前节点
- 右键菜单：查看详情、编辑、删除、ping 测试
- 拖拽排序
- "+" 按钮添加新节点

#### 聊天/Agent 面板 (ChatPanel)

```typescript
// src/components/ChatPanel/ChatPanel.tsx

interface Message {
  id: string;
  sender: 'user' | 'agent' | 'remote';
  content: string;
  timestamp: number;
  type: 'text' | 'code' | 'image' | 'file';
  metadata?: {
    agent?: 'claude' | 'kimi' | 'aider';
    codeLang?: string;
    fileName?: string;
  };
}

// 输入框支持：
// - @agent 提及选择 Agent
// - /skill 调用本地 Skill
// - #memory 引用记忆
// - 粘贴图片/文件
```

#### 侧边栏 (Sidebar)

```
┌──────────┐
│  💬 Chat │  ← 房间列表 (Matrix Room)
│  🤖 Agent│  ← Agent 交互面板
│  🧠 Mem  │  ← 记忆浏览器
│  🌐 Net  │  ← 网络拓扑图
│  ⚙️ 设置 │  ← 节点配置、Skill 管理
└──────────┘
```

---

## 5. 核心功能实现

### 5.1 节点切换逻辑

```rust
// src-tauri/src/state.rs

pub struct AppState {
    /// 当前活跃的 Matrix 客户端
    current_client: Arc<RwLock<Option<MatrixClient>>>,
    /// 所有配置的节点
    nodes: Arc<RwLock<HashMap<String, NodeConfig>>>,
    /// 当前选中节点 ID
    active_node: Arc<RwLock<String>>,
}

impl AppState {
    /// 切换到指定节点
    pub async fn switch_node(&self, node_id: &str) -> Result<()> {
        // 1. 断开当前连接
        if let Some(client) = self.current_client.read().await.as_ref() {
            client.disconnect().await?;
        }
        
        // 2. 获取新节点配置
        let node = self.nodes.read().await
            .get(node_id)
            .cloned()
            .ok_or("Node not found")?;
        
        // 3. 创建新客户端
        let client = MatrixClient::new(&node.address)?;
        client.connect().await?;
        
        // 4. 更新状态
        *self.current_client.write().await = Some(client);
        *self.active_node.write().await = node_id.to_string();
        
        // 5. 通知前端
        self.emit_event("node:switched", node_id).await?;
        
        Ok(())
    }
}
```

### 5.2 Agent 流式响应

```rust
// src-tauri/src/commands/agent.rs

use tauri::Window;

#[tauri::command]
async fn execute_prompt_stream(
    window: Window,
    provider: String,
    prompt: String,
) -> Result<(), String> {
    let agent = AgentProvider::new(&provider)?;
    
    // 流式回调
    let callback = move |chunk: String| {
        window.emit("agent:chunk", chunk).unwrap();
    };
    
    agent.execute_stream(&prompt, callback).await
        .map_err(|e| e.to_string())?;
    
    Ok(())
}
```

```typescript
// 前端接收流式响应
useEffect(() => {
  const unlisten = listen('agent:chunk', (event) => {
    setResponse(prev => prev + event.payload);
  });
  return () => unlisten.then(f => f());
}, []);
```

---

## 6. 与现有 CLI 集成

### 6.1 共享配置

```
~/.cis/
├── config.toml          # CLI 和 GUI 共享
├── data/
│   ├── core.db
│   └── federation.db
└── gui/
    └── window-state.json  # GUI 特有的窗口状态
```

### 6.2 互斥启动

```rust
// 防止 CLI 和 GUI 同时写入数据库
fn check_single_instance() -> Result<()> {
    let lock_file = Paths::data_dir().join(".cis.lock");
    // 使用文件锁或 socket
}
```

---

## 7. 打包和发布

### 7.1 构建命令

```bash
# 开发模式
cd cis-gui
npm run tauri dev

# 生产构建
npm run tauri build

# 输出
src-tauri/target/release/bundle/
├── dmg/              # macOS
├── deb/              # Linux
├── msi/              # Windows
└── appimage/         # Linux AppImage
```

### 7.2 集成到 CI

```yaml
# .github/workflows/gui-release.yml
- name: Build GUI
  run: |
    cd cis-gui
    npm install
    npm run tauri build
```

---

## 8. 开发计划

### Phase 1: 基础框架 (1周)
- [ ] 初始化 Tauri 项目
- [ ] 基础 IPC 接口
- [ ] 节点标签页组件

### Phase 2: IM 功能 (1周)
- [ ] Matrix 客户端集成
- [ ] 聊天界面
- [ ] 消息收发

### Phase 3: Agent 集成 (1周)
- [ ] Claude/Kimi 面板
- [ ] 流式响应
- [ ] 代码高亮

### Phase 4: 高级功能 (1周)
- [ ] 记忆浏览器
- [ ] 网络拓扑图
- [ ] 系统托盘

---

## 9. 技术栈总结

| 层级 | 技术 |
|------|------|
| 后端 | Rust + Tauri + cis-core |
| 前端 | React + TypeScript + TailwindCSS |
| 状态 | Zustand / Redux Toolkit |
| 通信 | Tauri IPC + Events |
| 打包 | Tauri CLI |

---

需要我开始实现 Phase 1 的基础框架吗？
