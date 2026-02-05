**CIS-DAG v1.0 最终方案：独联体执行协议（Commonwealth Execution Protocol）**

为数字游民、边缘计算与隐私优先开发设计的异步任务编排系统。

---

## 1. 核心理念（Philosophy）

**主权自治（Sovereignty）**  
每个节点是独立主权体，持有完整执行上下文，离线时可独立决策与运行，无需请示中央。

**涌现而非控制（Emergence over Control）**  
不追求强一致性，允许临时混乱（债务累积），通过人工干预与事后审查达成最终秩序。

**顺手即正义（Ergonomic）**  
从"想法"到"执行" < 10秒，错误即指南，失败可回滚，工具成为肢体的延伸而非认知的负担。

**零出网（Zero-Outbound）**  
代码与数据永不触碰公网，通信仅限自托管Matrix或本地mDNS，物理隔绝即安全。

---

## 2. 架构原则（Architecture）

| 原则             | 实现                                                 | 反模式（禁止）                      |
| ---------------- | ---------------------------------------------------- | ----------------------------------- |
| **扁平拓扑**     | DAG为固定长度数组，依赖仅指向低索引                  | 动态子任务、递归、运行时修改DAG结构 |
| **节点自治**     | 各节点本地维护状态机，Room仅作异步信标               | 中央调度器、实时RPC、分布式锁       |
| **能力硬匹配**   | 任务声明`Capabilities`（Cuda/Metal等），节点自主认领 | 动态负载均衡、协商式资源分配        |
| **沙盒真隔离**   | chroot + 清空环境变量 + 资源硬限制                   | 伪隔离（仅PATH修改）、继承$HOME     |
| **数据不跨节点** | 任务输入必须为本地路径，跨节点协作通过人工桥接       | NFS挂载、网盘同步、P2P文件传输      |
| **分级决策**     | 四级决策权（机械/推荐/确认/仲裁），可暂停可自动      | 一刀切自动执行或全部人工确认        |

---

## 3. 核心结构（Core Structures）

```rust
pub struct Dag {
    pub run_id: String,           // ULID
    pub name: String,             // 人类可读
    pub tasks: Vec<Task>,         // 固定长度，索引即ID
    pub policy: DagPolicy,        // AllSuccess | FirstSuccess | AllowDebt
}

pub struct Task {
    pub idx: u8,                  // 0-255，物理限制防膨胀
    pub name: String,
    
    // 执行定义（封闭、确定性）
    pub exec: Exec {
        pub cmd: Vec<String>,     // 白名单命令（非Shell）
        pub env: HashMap<String, String>, // 显式注入，无继承
        pub timeout: u16,         // 秒，0为无限制
        pub sandbox: SandboxConfig,
    },
    
    // 依赖：仅允许idx < self.idx，保证无环
    pub deps: Vec<u8>,
    
    // 分级与决策
    pub level: TaskLevel,
    pub on_ambiguity: AmbiguityPolicy,
    
    // 输入输出契约
    pub inputs: Vec<PathBuf>,     // 预检查存在性
    pub outputs: Vec<String>,     // 后验证产出
    
    // 回滚（可选）
    pub rollback: Option<Vec<String>>,
    pub idempotent: bool,
}

pub enum TaskLevel {
    Mechanical { retry: u8 },     // 自动执行，失败重试
    Recommended { default: Action, timeout: u16 }, // 倒计时执行，可干预
    Confirmed,                    // 模态确认，必须人工点击
    Arbitrated { stakeholders: Vec<String> }, // 暂停DAG，等待仲裁
}

pub enum AmbiguityPolicy {
    AutoBest,                     // 文档不清时选最优解
    Suggest { default: Action, timeout: u16 },
    Ask,                          // 必须询问
    Escalate,                     // 升级仲裁
}
```

---

## 4. 执行模型（Execution Model）

### 4.1 速写即执行（Fast Path）
```bash
$ cis run "测试体素算法，cuda和metal对比"
→ 自动生成DAG（基于本地Cargo.toml检测）
→ 打印Dry-run预览
→ 10秒内开始执行首个Ready Task
```

### 4.2 三阶段状态机
```text
Pending → Running → Completed
   ↓         ↓          ↓
Skipped   Failed     Debt（可忽略的失败）
   ↓      （技术债务）   ↓
         （阻塞性）→ Arbitrated（人工介入）
```

### 4.3 失败即债务（Failure as Debt）
- **可忽略债务**（Ignorable）：测试失败但无下游影响，标记为债务继续执行，事后审查
- **阻塞性债务**（Blocking）：编译失败导致无法链接，冻结DAG，等待人工或回滚
- **债务累积**：6767层显示"本次执行累积3项技术债务，预计修复时间30分钟"

### 4.4 热修改（Hot Amendment）
执行中可修改未运行Task：
```bash
$ cis amend --task 5 --env "RUST_LOG=debug"
→ Task 5（Pending状态）立即更新
→ Task 5（Running状态）收到SIGTERM，Checkpoint后重启
→ Task 5（Completed状态）标记为"需重跑"，不自动重跑（避免副作用）
```

---

## 5. 决策与交互（Decision & Interaction）

### 5.1 四级决策界面

| 级别            | 6767层表现                   | 用户操作             | 默认行为              |
| --------------- | ---------------------------- | -------------------- | --------------------- |
| **Mechanical**  | 后台日志，仅失败弹窗         | 无                   | 立即执行，失败重试3次 |
| **Recommended** | 通知栏："即将执行X（30s后）" | 点击"修改/跳过/立即" | 倒计时结束自动执行    |
| **Confirmed**   | 模态弹窗，显示风险分析       | 必须"确认"或"取消"   | 等待人工，无超时      |
| **Arbitrated**  | 冻结界面，打开决策工作区     | 手动解决后点击"继续" | 暂停整个DAG           |

### 5.2 模糊处理运行时
当`inputs`缺失或`env`变量无法解析：
1. **Mechanical**：按`AutoBest`策略填充（如选最新文件）
2. **Recommended**：弹出选项，30秒倒计时后选默认
3. **Confirmed/Arbitrated**：暂停，询问用户

---

## 6. 失败与债务（Failure & Debt）

### 6.1 错误即指南（Friendly Error）
```rust
pub struct FriendlyError {
    pub what: String,           // "CUDA编译失败"
    pub why: Vec<String>,       // ["nvcc未找到"]
    pub how: Vec<String>,       // ["1. 跳过此Task 2. 切换CPU模式 3. 查看详情"]
    pub context: ContextSnapshot, // 环境变量、工作目录、输入哈希
}
```

### 6.2 回滚机制
- **自动回滚**：Task声明`rollback`命令，失败时自动执行
- **生成Undo脚本**：执行前生成`/tmp/cis/undo-{run_id}.sh`，包含反向操作
- **Checkpoint**：Sandbox销毁前保留现场（只读），支持从失败点重启

---

## 7. 人机接口（CLI/GUI）

### 7.1 CLI设计（顺手原则）
```bash
cis run <sketch>          # 速写执行
cis status <run_id>       # TUI进度条（类似cargo）
cis amend <run_id> ...    # 热修改
cis debt                  # 查看技术债务列表
cis doctor                # 环境诊断与修复建议
cis verify-sandbox        # 验证沙盒隔离性
```

### 7.2 6767层（Web TUI）
- **进度可见**：ETA、当前子步骤、资源占用（CPU/内存）
- **债务看板**：累积失败、可自动修复项、需人工介入项
- **决策工作区**：仲裁级别Task的对比视图、决策记录、回滚预览

### 7.3 Coffee Mode（弱网自适应）
- **网络检测**：ping Bootstrap或mDNS，延迟>500ms自动降级
- **Island Mode**：完全离线时，事件写入本地`sled` WAL，恢复后批量同步
- **后台化**：Mechanical任务自动 detach，通过`cis attach`召回

---

## 8. 商业化定位（Commercial Positioning）

### 8.1 模式：咨询式开源（Consulting Model）
- **CIS-Core**：Apache 2.0开源，作为技术名片与交付基座
- **CIS-Custom**：客户特定需求（军工合规、异构农场）Fork改装，按人天收费
- **不追求PMF**：需求找上门才做，不主动营销，不背长期维护债

### 8.2 与VF（体素引擎）关系
- **定位**：CIS是VF的"基础设施溢价"，非独立产品
- **协同**：VF展示技术实力，CIS解决客户基建痛点（零出网渲染农场）
- **资源分配**：80%精力VF（资产），20%精力CIS（现金流）

### 8.3 客户边界
- **接**：Matrix联邦架构、零出网、异构硬件（CUDA/Metal）、弱网环境
- **不接**：简单CI/CD（推荐GitLab）、K8s原生需求（除非高价）、长期SaaS运维

---

## 9. 红线约束（Non-Negotiable）

1. **无动态DAG**：禁止运行时`tasks.push()`
2. **无跨节点文件**：禁止NFS/网盘路径，数据本地性强制
3. **无自主性**：Agent不"智能"决策，仅按显式策略执行
4. **无大文件传输**：Matrix消息<16KB，文件仅传路径
5. **无外部依赖**：零Docker，零K8s，零云端API，纯本地Rust

---

**结语**  
CIS-DAG v1.0 是**数字游民的铁锤**——不顺手时砸自己脚，顺手时 extensions of will。它不追求分布式系统的学术优雅，只追求**在咖啡馆WiFi中断时，依然能让5090和M4默默跑完测试，等你落地后查看结果**的确定性与自由。