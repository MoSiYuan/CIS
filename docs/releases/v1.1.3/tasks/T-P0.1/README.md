# T-P0.1: 替换 Embedding Mock 实现

## 任务状态
**状态**: ✅ 已完成  
**执行者**: Agent-F  
**完成时间**: 2026-02-10

## 变更摘要

将 `cis-core/src/ai/embedding.rs` 中的两个 mock 实现替换为真实的 FastEmbed 服务：

### 1. `ClaudeCliEmbeddingService` (line 362-437)
**修改前**: 使用 hash-based 伪嵌入向量生成（模拟实现）  
**修改后**: 内部使用 `FastEmbedService` 提供真实的 Nomic Embed Text v1.5 嵌入

### 2. `SqlFallbackEmbeddingService` (line 439-513)
**修改前**: 返回零向量 `[0.0; 1]`（虚假实现）  
**修改后**: 内部使用 `FastEmbedService` 提供真实的嵌入

### 3. 初始化逻辑更新
**修改前**: 检查 `claude` 命令存在后使用 `ClaudeCliEmbeddingService`  
**修改后**: 
- 移除了检查 `claude` 命令的分支
- 服务回退链简化为：FastEmbed → OpenAI → SQL Fallback (FastEmbed)

## 技术细节

- 两个服务都添加了 `#[cfg(feature = "vector")]` 条件编译
- 当 `vector` feature 启用时，使用真实的 `FastEmbedService`
- 当 `vector` feature 禁用时，返回配置错误
- 服务初始化改为异步 (`async fn new()`)

## 验证

```bash
cargo check --features vector
```

无 embedding 模块相关错误。

## 剩余工作

- P2P 模块有独立编译错误（22 个），需其他任务处理
