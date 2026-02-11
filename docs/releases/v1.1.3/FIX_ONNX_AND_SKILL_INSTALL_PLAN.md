# CIS v1.1.1 问题修复规划

## 问题概述

### 问题1: ONNX 向量引擎未实现
- **现状**: 模型可以下载，但 `encode_internal()` 返回错误
- **影响**: 用户下载 130MB 模型后无法使用语义搜索
- **要求**: 必须实现真正的本地 ONNX 推理，不能依赖 OpenAI 降级

### 问题2: Skill 安装繁琐
- **现状**: 初始化时不安装 skill，需要手动逐个安装
- **影响**: 用户需要执行多个命令才能使用完整功能
- **要求**: 初始化时自动安装内置 skill

---

## 修复方案规划

### 阶段1: ONNX 本地推理实现

#### 方案选择

| 方案 | 优点 | 缺点 | 评估 |
|------|------|------|------|
| A. 使用 `ort` crate (ONNX Runtime) | 完整 ONNX 支持，性能好 | 依赖 C++ 库，编译复杂 | ❌ 原计划，未实现 |
| B. 使用 `rust-bert` | 纯 Rust，BERT 原生支持 | 模型格式不兼容，体积大 | ❌ 需要转换模型 |
| C. 使用 `candle` (ML Framework) | 纯 Rust，轻量级，HuggingFace 原生 | 需要手动实现模型前向传播 | ✅ 推荐 |
| D. 使用 `fastembed-rs` | 专为嵌入设计，多模型支持 | 额外依赖 | ✅ 备选 |

**决策**: 采用 **方案 C (candle)** + **方案 D (fastembed-rs)** 双轨制

#### 实现规划

1. **candle 实现** (默认)
   - 使用 `candle-transformers` 库
   - 支持 Nomic Embed Text v1.5
   - 纯 Rust，无需外部依赖
   - 实现路径: `cis-core/src/ai/embedding_candle.rs`

2. **fastembed-rs 备选**
   - 使用 `fastembed` crate
   - 自动模型管理
   - 多模型支持 (Nomic, BGE, etc.)
   - 实现路径: `cis-core/src/ai/embedding_fastembed.rs`

3. **统一接口**
   - 保持 `EmbeddingService` trait 不变
   - 运行时选择实现
   - feature flag 控制编译

#### 技术细节

```rust
// 新的模块结构
cis-core/src/ai/
├── embedding.rs           # 统一接口 (保持兼容)
├── embedding_candle.rs    # candle 实现 (默认)
├── embedding_fastembed.rs # fastembed 实现 (备选)
├── embedding_onnx.rs      # 保留 ort 实现 (未来)
├── embedding_download.rs  # 模型下载 (修改)
└── embedding_init.rs      # 初始化向导 (修改)

// Cargo.toml 配置
[features]
default = ["embedding-candle"]
embedding-candle = ["candle-transformers", "candle-nn"]
embedding-fastembed = ["fastembed"]
embedding-onnx = ["ort"]  # 未来实现
```

#### 开发步骤

1. **Day 1**: 添加 candle 依赖，实现基础推理
2. **Day 2**: 集成 Nomic Embed 模型，测试向量化
3. **Day 3**: 性能优化，添加缓存
4. **Day 4**: 测试验证，文档更新

---

### 阶段2: 内置 Skill 自动安装

#### 内置 Skill 列表

| Skill | 用途 | 必需性 |
|-------|------|--------|
| `init-wizard` | 初始化向导 | ✅ 必需 |
| `memory-organizer` | 记忆整理 | ✅ 必需 |
| `dag-executor` | DAG 任务执行 | ✅ 必需 |
| `ai-executor` | AI 任务执行 | ✅ 必需 |
| `push-client` | 推送客户端 | ⚪ 可选 |
| `im` | 即时消息 | ⚪ 可选 |

#### 实现规划

1. **Skill Registry 预配置**
   ```rust
   // cis-core/src/skill/builtin.rs
   pub const BUILTIN_SKILLS: &[BuiltinSkill] = &[
       BuiltinSkill {
           name: "init-wizard",
           path: "skills/init-wizard",
           auto_install: true,
       },
       // ...
   ];
   ```

2. **初始化流程修改**
   ```rust
   // cis-core/src/init/wizard.rs
   async fn run(&self, project_mode: bool) -> Result<WizardResult> {
       // ... 现有步骤 ...
       
       // 新增: Step 6 - 安装内置 Skill
       self.install_builtin_skills().await?;
       
       Ok(result)
   }
   ```

3. **自动编译安装**
   - 检测 skill 源码目录
   - 自动执行 `cargo build`
   - 注册到 skill registry
   - 错误处理和回滚

#### 用户界面

```
Step 4/6: 安装内置技能
  📦 发现 6 个内置技能
    ✅ init-wizard        (必需) - 初始化向导
    ✅ memory-organizer   (必需) - 记忆整理
    ✅ dag-executor       (必需) - DAG 执行器
    ✅ ai-executor        (必需) - AI 执行器
    ⚪ push-client        (可选) - 推送客户端
    ⚪ im                 (可选) - 即时消息

  正在编译安装... (这可能需要几分钟)
    [####################] 100% 6/6 完成

  ✅ 所有必需技能安装完成！
```

---

### 阶段3: 初始化流程优化

#### 当前流程 vs 优化后

```
当前流程 (6步):
1. 环境检查 → 2. 全局配置 → 3. 向量引擎 → 4. 项目初始化 → 5. 验证 → 6. 完成
                                                    ↓
                                              缺少 skill 安装

优化后流程 (6步):
1. 环境检查 → 2. 全局配置 → 3. 向量引擎 → 4. 项目初始化 → 5. 安装技能 → 6. 验证
                                                    ↓
                                              新增技能自动安装
```

#### 优化点

1. **并行下载**: 模型下载和 skill 编译并行执行
2. **进度显示**: 详细的进度条和 ETA
3. **错误恢复**: 单点失败不影响整体流程
4. **非交互模式**: 支持 `cis init --yes` 一键安装

---

## 时间规划

| 阶段 | 任务 | 预计时间 | 优先级 |
|------|------|----------|--------|
| 1 | candle 嵌入实现 | 3天 | P0 |
| 2 | 内置 skill 自动安装 | 2天 | P0 |
| 3 | 集成测试 | 1天 | P1 |
| 4 | 文档更新 | 0.5天 | P2 |
| **总计** | | **6.5天** | |

---

## 风险评估

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| candle 不支持 Nomic Embed | 中 | 高 | 使用 fastembed-rs 备选 |
| Skill 编译失败 | 低 | 中 | 提供预编译二进制选项 |
| 性能不达标 | 中 | 中 | 添加 GPU 支持，优化缓存 |
| 向后兼容问题 | 低 | 高 | 保持 trait 接口不变 |

---

## 验收标准

### ONNX 实现
- [ ] 无需 OpenAI API Key 即可使用语义搜索
- [ ] 首次加载 < 3秒，后续推理 < 100ms
- [ ] 支持 Nomic Embed Text v1.5 (768维)
- [ ] 纯 Rust 实现，无外部 C++ 依赖

### Skill 自动安装
- [ ] `cis init` 自动安装 4 个必需 skill
- [ ] 安装失败有明确错误提示
- [ ] 非交互模式支持 `--yes` 参数
- [ ] 安装后 `cis skill list` 显示已安装

---

## 下一步行动

1. **确认方案**: 审查此规划文档
2. **创建分支**: `fix/onnx-and-skill-install`
3. **开始实现**: 按阶段执行
4. **每日同步**: 进度更新和调整
