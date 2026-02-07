# 向量引擎初始化改进

## 问题

之前 `cis init` 时，向量引擎（记忆、语义搜索必需）未正确配置，用户会遇到 "vector feature 未启用" 的提示。

## 解决方案

在 `cis init` 向导中添加了**专门的向量引擎配置步骤**，引导用户完成配置。

## 改进内容

### 1. 新增初始化步骤

```
步骤 1/5: 环境检查
步骤 2/5: 全局配置
步骤 3/5: 向量引擎配置  ← 新增
步骤 4/5: 项目初始化 (可选)
步骤 5/5: 验证
```

### 2. 向量引擎配置流程

#### 交互式配置

```bash
$ cis init

┌─ 步骤 3/5: 向量引擎配置
│
  检查向量引擎状态...

  📚 CIS 向量引擎用于：
     • 语义记忆检索（自然语言搜索）
     • 智能技能匹配
     • 对话上下文理解
     • 项目知识库搜索

  是否现在配置向量引擎? (推荐)
  (Y/n): 
```

#### 配置选项

如果用户选择配置，将调用 `embedding_init::interactive_init()`：

```
请选择 embedding 服务配置方式：

  [1] 下载本地模型 (Nomic Embed v1.5, ~130MB) ⭐ 推荐
      - 优点：离线使用，无需 API Key，隐私性好，语义搜索质量高
      - 缺点：需要下载模型文件 (~130MB)

  [2] 使用 Claude CLI 代理
      - 优点：利用已安装的 Claude CLI，无需下载
      - 缺点：速度较慢，启发式嵌入

  [3] 使用 OpenAI API (text-embedding-3-small)
      - 优点：高质量嵌入
      - 缺点：需要 API Key，消耗 token

  [4] 回退到简单搜索 (SQL LIKE)
      - 优点：无需任何配置，完全离线
      - 缺点：仅支持关键词匹配，无语义搜索

  [5] 跳过配置 (稍后手动设置)
```

#### 非交互式自动配置

```bash
$ cis init --non-interactive

  非交互模式：使用自动配置...
  ✓ 自动配置：本地向量模型
```

自动配置优先级：
1. 本地模型（Nomic Embed v1.5）- 已存在
2. Claude CLI（Agent 工具）- 已安装
3. OpenAI API（需要 API Key）- 环境变量已设置
4. SQL 回退模式

### 3. 生成的配置

在 `config.toml` 中新增 `[vector]` 配置段：

```toml
[vector]
# 向量引擎配置（用于语义搜索和记忆）
# 嵌入维度: 768 (Nomic Embed), 1536 (OpenAI), 384 (MiniLM)
embedding_dim = 768

# 是否启用 HNSW 索引（推荐启用）
use_hnsw = true

# 相似度阈值（0-1，越高越严格）
default_threshold = 0.7

# 向量存储路径（默认使用数据目录）
# storage_path = "/var/lib/cis/vectors"
```

### 4. 验证测试

在验证步骤中新增向量引擎测试：

```
  运行验证测试...

  [1/5] 配置读取... ✅ 通过
  [2/5] 目录写入... ✅ 通过
  [3/5] 节点密钥... ✅ 通过
  [4/5] 向量引擎... ✅ 通过  ← 新增
  [5/5] AI Provider... ✅ 通过
```

如果向量引擎未配置：

```
  [4/5] 向量引擎... ⚠️  警告: 向量引擎未配置
     运行 `cis config vector` 进行配置
```

## 向后兼容

- 已配置向量引擎的用户：自动跳过配置步骤
- 跳过配置的用户：可通过 `cis config vector` 重新配置
- 现有配置文件：无需修改，自动兼容

## 使用指南

### 首次初始化

```bash
# 交互式（推荐）
cis init

# 非交互式（CI/CD）
cis init --non-interactive
```

### 重新配置向量引擎

```bash
# 如果初始化时跳过了配置
cis config vector

# 或重新运行初始化
cis init --force
```

### 检查向量引擎状态

```bash
# 运行诊断
cis doctor

# 检查配置
cat ~/.cis/config.toml | grep -A 10 "\[vector\]"
```

## 故障排查

### 问题：模型下载失败

```bash
# 手动下载模型
mkdir -p ~/.cis/models/nomic-embed-text-v1.5
cd ~/.cis/models/nomic-embed-text-v1.5

# 下载模型文件
wget https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx
wget https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer.json
```

### 问题：内存不足

```toml
# 使用更小的模型
[vector]
embedding_dim = 384  # 使用 MiniLM 替代 Nomic
```

### 问题：跳过配置后如何启用

```bash
# 方法 1：重新初始化
cis init --force

# 方法 2：使用专门的配置命令（如果存在）
cis config vector
```

## 技术细节

### 相关文件

- `cis-core/src/init/wizard.rs` - 初始化向导
- `cis-core/src/ai/embedding_init.rs` - Embedding 初始化逻辑
- `cis-core/src/vector/mod.rs` - 向量引擎模块

### 关键函数

```rust
// 检查是否需要初始化
pub fn needs_init() -> bool

// 交互式初始化
pub fn interactive_init() -> Result<EmbeddingInitConfig>

// 自动初始化
pub fn auto_init() -> Result<EmbeddingInitConfig>
```

## 总结

| 改进项 | 说明 |
|--------|------|
| **新增步骤** | 步骤 3/5 专门用于向量引擎配置 |
| **交互引导** | 清晰的功能说明和配置选项 |
| **自动检测** | 非交互模式自动选择最优方案 |
| **配置模板** | 生成的配置包含 `[vector]` 段 |
| **验证测试** | 新增向量引擎状态检测 |

---

**效果**：用户初始化时会明确知道向量引擎的作用，并得到清晰的配置引导，避免 "vector feature 未启用" 的困惑。
