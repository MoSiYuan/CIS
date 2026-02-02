# CIS Vector Intelligence - 上下文摘要

**目的**: 供并行开发团队快速了解项目背景  
**阅读时间**: 5分钟  

---

## 1. 项目现状

### 1.1 已完成 (MATRIX-final)
- ✅ MatrixNucleus + DID身份系统
- ✅ WebSocket联邦 (6768端口)
- ✅ Cloud Anchor服务发现
- ✅ Noise Protocol握手
- ✅ Event联邦广播
- ✅ 断线同步队列
- ✅ IM Skill完整实现
- ✅ CLI peer命令

### 1.2 当前缺失 (本项目要解决的)
- ❌ 语义记忆搜索 (当前search()返回空)
- ❌ 对话持久化 (Agent调用后丢失)
- ❌ Skill自然语言调用 (必须知道!skill格式)
- ❌ 跨项目上下文恢复 (切换目录后丢失)

---

## 2. 核心创新

### 2.1 Skill向量自动化 ⭐
```
传统: !skill git commit --message="fix"
      ↓ 必须知道skill名称和参数

创新: cis skill do "帮我提交代码，修复了登录bug"
      ↓ 意图解析 → 语义匹配 → 参数提取 → 自动执行
```

### 2.2 项目隔离 (无幻觉)
```
项目A: .cis/skill_vectors.db ← 本地搜索（默认）
项目B: .cis/skill_vectors.db ← 完全隔离

搜索"编辑":
- 项目A返回空（无幻觉）
- 显式search-global才查其他项目（带警告）
```

### 2.3 Skill链编排
```
输入: "分析数据并生成PDF报告"
输出: data-analyzer → report-gen（自动发现链）
```

---

## 3. 技术方案

### 3.1 三层向量架构
| 层 | 表 | 用途 |
|---|---|---|
| Memory | memory_embeddings | 语义记忆搜索 |
| Session | message_embeddings, summary_embeddings | 对话持久化+跨项目恢复 |
| Skill | skill_intent_vec, skill_capability_vec | 自然语言调用+链编排 |

### 3.2 关键组件
```rust
// 基础设施
EmbeddingService: embed(text) -> Vec<f32>[768]
VectorStorage: index/search for Memory/Session/Skill

// Session层
ConversationContext: add_message, find_similar(project?), save_with_summary

// Skill层 ⭐核心
IntentParser: 解析"分析今天的数据" -> {action: Analyze, date: today, data: sales}
SkillVectorRouter: route(input) -> SkillMatch (本地优先)
ProjectSkillRegistry: 项目隔离，scope: Local|Global
CisAdminSkill: Skill形式暴露给用户的入口
```

### 3.3 项目隔离机制
```rust
struct ProjectSkillRegistry {
    project_id: String,
    storage: VectorStorage, // 指向.cis/skill_vectors.db
}

// 默认只搜本地
pub async fn search(&self, query: &str) -> Result<Vec<SkillMatch>> {
    self.storage.search_skills(query, Some(&self.project_id), 5).await
}
```

---

## 4. 并行开发主线

| 主线 | 人数 | 周次 | 产出 |
|---|---|---|---|
| A-基础设施 | 2 | 1-2 | EmbeddingService, VectorStorage, HNSW索引 |
| B-Session层 | 2 | 1-4 | ConversationDb, Context, 跨项目恢复 |
| C-Skill自动化 ⭐ | 3 | 3-5 | IntentParser, Router, Chain, CisAdminSkill |
| D-集成测试 | 2 | 5-6 | CLI, RAG, 测试, 文档 |

---

## 5. 关键决策

| 决策 | 选择 | 原因 |
|---|---|---|
| 向量维度 | 768 | MiniLM-L6-v2平衡性能和效果 |
| 相似度阈值 | 0.7 | Skill匹配, <0.7认为不匹配 |
| 嵌入模型 | 本地优先 | 隐私+离线, 云端降级 |
| 项目DB位置 | .cis/skill_vectors.db | 与项目一起版本控制 |
| 跨项目搜索 | 显式+警告 | 防止无意识的幻觉 |

---

## 6. 文件索引

| 文档 | 内容 | 给谁看 |
|---|---|---|
| CIS_VECTOR_IMPLEMENTATION.md | 完整实施文档 | 所有开发者 |
| TASKS_PARALLEL.md | 并行任务清单+依赖 | 开发负责人 |
| CONTEXT_SUMMARY.md | 本文件 | 新加入开发者 |

---

## 7. 快速开始

```bash
# Week 1任务
1. 实现EmbeddingService (T-A1)
2. 创建VectorStorage (T-A2)
3. 创建ConversationDb (T-B1)
4. 实现IntentParser (T-C1)

# 联调点
Week 2结束: A2+B1+C1联调
Week 3结束: B3+C2联调
Week 4结束: C3+C4+C5联调
```

---

**最后更新**: 2026-02-02  
**状态**: Ready for parallel development
