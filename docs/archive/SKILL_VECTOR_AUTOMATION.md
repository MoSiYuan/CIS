# Skill 向量自动化：语义驱动的 Skill 发现与路由

## 一、核心概念

### 1.1 什么是 Skill 向量自动化？

```
传统方式：
  用户输入: "!skill git commit --message='fix bug'"
  → 必须知道 skill 名称 "git"
  → 必须知道具体命令 "commit"
  → 必须知道参数格式

向量自动化方式：
  用户输入: "帮我提交代码，修复了登录bug"
  → 语义嵌入: [0.12, 0.85, -0.33, ...]  # 768维向量
  → 向量匹配: git-skill (相似度 0.92)
  → 意图解析: action=commit, message="fix login bug"
  → 自动调用: git.commit(message="fix login bug")
```

### 1.2 核心能力

| 能力 | 说明 | 示例 |
|------|------|------|
| **意图路由** | 自然语言匹配最佳 Skill | "查天气" → weather-skill |
| **参数提取** | 从文本提取结构化参数 | "明天北京" → {date: "tomorrow", city: "beijing"} |
| **Skill 发现** | 语义搜索已注册 Skills | 找到 "类似文件管理" 的 skills |
| **链式调用** | 基于上下文自动调用多个 Skills | "分析并提交" → analyze → git.commit |

---

## 二、架构设计

### 2.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                            用户输入层                                        │
│  "帮我分析今天的销售数据并生成报表"                                          │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Intent Parser (意图解析器)                          │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐  │
│  │ 文本嵌入        │  │ 意图分类        │  │ 实体提取 (NER)              │  │
│  │                 │  │                 │  │                             │  │
│  │ Input → Vector  │  │ intent: analyze │  │ - entity: sales_data        │  │
│  │ [0.1, 0.8, ...] │  │ confidence: 0.94│  │ - time: today               │  │
│  └────────┬────────┘  │ action: report  │  │ - output: report            │  │
│           │           └────────┬────────┘  └─────────────────────────────┘  │
│           │                    │                                            │
│           └────────────────────┼────────────────────────────────────────────┘
│                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      Skill Vector Router (Skill 向量路由)                     │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  query_vector: [0.1, 0.8, -0.3, ...]                                │   │
│  │                                                                     │   │
│  │  ┌─────────────────────────────────────────────────────────────┐   │   │
│  │  │  skill_vec 表 (sqlite-vec)                                   │   │   │
│  │  │  ┌────────────────┬────────────────┬──────────────────┐     │   │   │
│  │  │  │ skill_id       │ intent_vector  │ capability_vec   │     │   │   │
│  │  │  ├────────────────┼────────────────┼──────────────────┤     │   │   │
│  │  │  │ data-analyzer  │ [0.85, 0.12..] │ [0.90, 0.05..]   │     │   │   │
│  │  │  │ ← 匹配度 0.92  │ analyze data   │ sales, csv, pdf  │     │   │   │
│  │  │  ├────────────────┼────────────────┼──────────────────┤     │   │   │
│  │  │  │ git-skill      │ [0.15, 0.88..] │ [0.20, 0.95..]   │     │   │   │
│  │  │  │ ← 匹配度 0.23  │ version control│ commit, push     │     │   │   │
│  │  │  ├────────────────┼────────────────┼──────────────────┤     │   │   │
│  │  │  │ report-gen     │ [0.88, 0.10..] │ [0.85, 0.15..]   │     │   │   │
│  │  │  │ ← 匹配度 0.89  │ generate report│ pdf, excel, md   │     │   │   │
│  │  │  └────────────────┴────────────────┴──────────────────┘     │   │   │
│  │  └─────────────────────────────────────────────────────────────┘   │   │
│  │                                                                     │   │
│  │  搜索结果:                                                          │   │
│  │  1. data-analyzer (0.92) ✓ 主匹配                                  │   │
│  │  2. report-gen (0.89)    ✓ 辅助匹配 → 链式调用候选                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                    ┌─────────────┴─────────────┐
                    │                           │
                    ▼                           ▼
┌─────────────────────────────────┐   ┌─────────────────────────────────┐
│     Skill Parameter Resolver    │   │      Chain Orchestrator         │
│     (参数解析器)                │   │      (链式编排器)               │
│                                 │   │                                 │
│  intent: analyze_sales          │   │  Step 1: data-analyzer          │
│  entities: {                    │   │          ↓ result               │
│    target: "sales_data",        │   │  Step 2: report-gen             │
│    time: "today",               │   │          ↓ report               │
│    output: "report"             │   │  Step 3: [可选] notify          │
│  }                              │   │                                 │
│                                 │   │  自动编排多 Skill 流水线         │
│  → 转换为 Skill 调用参数         │   │                                 │
└────────────────┬────────────────┘   └────────────────┬────────────────┘
                 │                                    │
                 └────────────────┬───────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Skill Executor (Skill 执行器)                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  调用: data-analyzer.analyze(                                      │   │
│  │           target="sales_data",                                     │   │
│  │           filters={time: "today"}                                  │   │
│  │         )                                                          │   │
│  │                                                                    │   │
│  │  结果: {charts: [...], summary: "今日销售额..."}                   │   │
│  │         ↓                                                          │   │
│  │  调用: report-gen.generate(                                        │   │
│  │           data=prev_result,                                        │   │
│  │           format="pdf"                                             │   │
│  │         )                                                          │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              输出层                                          │
│  "✅ 分析完成！今日销售额同比增长 15%，主要来源..."                          │
│  "📄 报表已生成: /reports/sales_20240115.pdf"                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 三、核心数据结构

### 3.1 Skill 语义注册表

```rust
/// Skill 语义描述（用于向量索引）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSemantics {
    /// Skill ID
    pub skill_id: String,
    
    /// 主要意图向量（描述 Skill 核心功能）
    /// 生成方式: embed("分析数据并生成可视化报告")
    pub intent_vector: Vec<f32>,
    
    /// 能力向量（描述 Skill 能处理的数据类型/格式）
    /// 生成方式: embed("支持 CSV, Excel, JSON 数据格式")
    pub capability_vector: Vec<f32>,
    
    /// 示例短语（用于生成意图向量的源文本）
    pub example_intents: Vec<String>,
    
    /// 支持的参数 Schema
    pub parameter_schema: SkillParameterSchema,
    
    /// 输入/输出数据类型
    pub io_signature: SkillIoSignature,
    
    /// 关联的 Skills（用于链式调用发现）
    pub related_skills: Vec<String>,
    
    /// 注册时间
    pub registered_at: DateTime<Utc>,
}

/// Skill 参数 Schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameterSchema {
    /// 参数定义
    pub parameters: Vec<ParameterDef>,
    
    /// 必需参数列表
    pub required: Vec<String>,
    
    /// 参数提取提示词（用于 NER）
    pub extraction_hints: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterDef {
    pub name: String,
    pub param_type: ParameterType,
    pub description: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    DateTime,
    FilePath,
    Enum { values: Vec<String> },
    Array { item_type: Box<ParameterType> },
    Object { properties: Vec<ParameterDef> },
}

/// Skill IO 签名
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillIoSignature {
    /// 输入数据类型（MIME-like）
    pub input_types: Vec<String>,  // e.g., ["text/csv", "application/json"]
    
    /// 输出数据类型
    pub output_types: Vec<String>, // e.g., ["application/pdf", "image/png"]
    
    /// 能否作为管道中间节点
    pub pipeable: bool,
    
    /// 能否作为管道起点
    pub source: bool,
    
    /// 能否作为管道终点
    pub sink: bool,
}
```

### 3.2 SQLite 表结构

```sql
-- ============================================
-- Skill 向量注册表
-- ============================================

-- Skill 语义描述主表
CREATE TABLE skill_semantics (
    skill_id TEXT PRIMARY KEY,
    skill_name TEXT NOT NULL,
    description TEXT,
    example_intents_json TEXT,  -- JSON array of strings
    parameter_schema_json TEXT, -- JSON schema
    io_signature_json TEXT,     -- JSON
    related_skills_json TEXT,   -- JSON array of skill_ids
    registered_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- sqlite-vec: Skill 意图向量表
-- 用于 "我想分析数据" → 匹配 data-analyzer skill
CREATE VIRTUAL TABLE skill_intent_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);

-- sqlite-vec: Skill 能力向量表
-- 用于 "处理 CSV 文件" → 匹配支持 CSV 的 skills
CREATE VIRTUAL TABLE skill_capability_vec USING vec0(
    skill_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);

-- ============================================
-- 意图历史与优化
-- ============================================

-- 用户意图执行历史（用于优化匹配算法）
CREATE TABLE intent_execution_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_input TEXT NOT NULL,
    input_vector BLOB,  -- 用户输入的向量表示
    matched_skill_id TEXT NOT NULL,
    match_score REAL NOT NULL,
    parameters_json TEXT,
    execution_success BOOLEAN,
    execution_result TEXT,
    executed_at INTEGER NOT NULL,
    feedback_score INTEGER,  -- 用户反馈：1-5 星
    
    FOREIGN KEY (matched_skill_id) REFERENCES skill_semantics(skill_id)
);

-- 成功的意图-参数模板（用于 few-shot learning）
CREATE TABLE intent_templates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    template_text TEXT NOT NULL,      -- 规范化后的模板
    template_vector BLOB NOT NULL,    -- 模板向量
    skill_id TEXT NOT NULL,
    parameter_mapping_json TEXT,      -- 参数映射规则
    usage_count INTEGER DEFAULT 0,
    success_rate REAL DEFAULT 1.0,
    
    FOREIGN KEY (skill_id) REFERENCES skill_semantics(skill_id)
);

-- ============================================
-- Skill 链式调用编排
-- ============================================

-- Skill 链定义
CREATE TABLE skill_chains (
    chain_id TEXT PRIMARY KEY,
    chain_name TEXT,
    description TEXT,
    trigger_intent_vector BLOB,  -- 触发此链的意图向量
    steps_json TEXT NOT NULL,    -- [{skill_id, input_map, output_map}]
    created_at INTEGER NOT NULL,
    usage_count INTEGER DEFAULT 0
);

-- Skill 兼容性矩阵（用于链式调用发现）
CREATE TABLE skill_compatibility (
    source_skill_id TEXT NOT NULL,
    target_skill_id TEXT NOT NULL,
    compatibility_score REAL NOT NULL,  -- 0.0 - 1.0
    data_flow_types TEXT,  -- JSON: {"input": "output_type", "output": "input_type"}
    discovered_at INTEGER NOT NULL,
    
    PRIMARY KEY (source_skill_id, target_skill_id),
    FOREIGN KEY (source_skill_id) REFERENCES skill_semantics(skill_id),
    FOREIGN KEY (target_skill_id) REFERENCES skill_semantics(skill_id)
);
```

---

## 四、核心组件实现

### 4.1 Skill Vector Registry

```rust
// cis-core/src/skill/vector_registry.rs

use sqlite_vec::VectorIndex;

/// Skill 向量注册表
pub struct SkillVectorRegistry {
    conn: Connection,
    embedding_service: Arc<dyn EmbeddingService>,
}

impl SkillVectorRegistry {
    /// 注册 Skill 语义描述
    pub async fn register_semantics(&self, semantics: SkillSemantics) -> Result<()> {
        // 1. 保存到主表
        self.conn.execute(
            "INSERT INTO skill_semantics 
             (skill_id, skill_name, description, example_intents_json, 
              parameter_schema_json, io_signature_json, related_skills_json,
              registered_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(skill_id) DO UPDATE SET
             description = excluded.description,
             example_intents_json = excluded.example_intents_json,
             updated_at = excluded.updated_at",
            params![
                semantics.skill_id,
                semantics.skill_name,
                semantics.description,
                serde_json::to_string(&semantics.example_intents)?,
                serde_json::to_string(&semantics.parameter_schema)?,
                serde_json::to_string(&semantics.io_signature)?,
                serde_json::to_string(&semantics.related_skills)?,
                semantics.registered_at.timestamp(),
                semantics.updated_at.timestamp(),
            ],
        )?;
        
        // 2. 生成并保存意图向量
        let intent_text = semantics.example_intents.join("; ");
        let intent_vector = self.embedding_service.embed(&intent_text).await?;
        
        self.conn.execute(
            "INSERT INTO skill_intent_vec (skill_id, embedding) VALUES (?1, ?2)
             ON CONFLICT(skill_id) DO UPDATE SET embedding = excluded.embedding",
            (&semantics.skill_id, &intent_vector as &[f32]),
        )?;
        
        // 3. 生成并保存能力向量
        let capability_text = format!("{} supports {:?}", 
            semantics.skill_name,
            semantics.io_signature.input_types
        );
        let capability_vector = self.embedding_service.embed(&capability_text).await?;
        
        self.conn.execute(
            "INSERT INTO skill_capability_vec (skill_id, embedding) VALUES (?1, ?2)
             ON CONFLICT(skill_id) DO UPDATE SET embedding = excluded.embedding",
            (&semantics.skill_id, &capability_vector as &[f32]),
        )?;
        
        tracing::info!("Registered skill semantics: {}", semantics.skill_id);
        Ok(())
    }
    
    /// 语义搜索 Skills
    pub async fn search_by_intent(
        &self,
        user_input: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<SkillMatchResult>> {
        let query_vector = self.embedding_service.embed(user_input).await?;
        
        let mut stmt = self.conn.prepare(
            "SELECT s.skill_id, s.skill_name, s.description, v.distance
             FROM skill_intent_vec vec
             JOIN skill_semantics s ON vec.skill_id = s.skill_id
             JOIN vec_skill_intent_vec v ON vec.skill_id = v.skill_id
             WHERE v.embedding MATCH ?1
             AND k = ?2
             ORDER BY v.distance
             LIMIT ?2"
        )?;
        
        let results = stmt.query_map((&query_vector as &[f32], limit as i32), |row| {
            let distance: f32 = row.get(3)?;
            let similarity = 1.0 - distance;  // 转换为相似度
            
            Ok(SkillMatchResult {
                skill_id: row.get(0)?,
                skill_name: row.get(1)?,
                description: row.get(2)?,
                similarity,
                match_type: MatchType::Intent,
            })
        })?;
        
        results
            .filter_map(|r| r.ok())
            .filter(|r| r.similarity >= threshold)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }
    
    /// 基于能力匹配（例如："处理 CSV"）
    pub async fn search_by_capability(
        &self,
        capability_desc: &str,
        limit: usize,
    ) -> Result<Vec<SkillMatchResult>> {
        let query_vector = self.embedding_service.embed(capability_desc).await?;
        
        // 类似 search_by_intent，但查询 skill_capability_vec
        // ...
    }
    
    /// 发现 Skill 链（多步调用）
    pub async fn discover_skill_chain(
        &self,
        user_input: &str,
        max_depth: usize,
    ) -> Result<Option<SkillChain>> {
        // 1. 解析用户意图，识别是否需要多步
        let primary_matches = self.search_by_intent(user_input, 3, 0.7).await?;
        
        if primary_matches.is_empty() {
            return Ok(None);
        }
        
        let primary_skill = &primary_matches[0];
        
        // 2. 检查是否需要后续步骤（基于 IO 签名）
        let io_sig: SkillIoSignature = self.conn.query_row(
            "SELECT io_signature_json FROM skill_semantics WHERE skill_id = ?1",
            [&primary_skill.skill_id],
            |row| {
                let json: String = row.get(0)?;
                Ok(serde_json::from_str(&json).unwrap_or_default())
            }
        )?;
        
        // 3. 如果 primary skill 不是 sink，寻找后续 skills
        if !io_sig.sink {
            let output_type = &io_sig.output_types[0];
            
            // 查找能接收此输出类型的 skills
            let next_skills = self.find_compatible_skills(&primary_skill.skill_id, output_type).await?;
            
            if !next_skills.is_empty() {
                return Ok(Some(SkillChain {
                    steps: vec![
                        ChainStep {
                            skill_id: primary_skill.skill_id.clone(),
                            input_mapping: InputMapping::Direct,
                            output_mapping: OutputMapping::PassThrough,
                        },
                        ChainStep {
                            skill_id: next_skills[0].skill_id.clone(),
                            input_mapping: InputMapping::FromPrevious("data".to_string()),
                            output_mapping: OutputMapping::Final,
                        },
                    ],
                }));
            }
        }
        
        // 单步调用
        Ok(Some(SkillChain {
            steps: vec![ChainStep {
                skill_id: primary_skill.skill_id.clone(),
                input_mapping: InputMapping::Direct,
                output_mapping: OutputMapping::Final,
            }],
        }))
    }
    
    /// 发现兼容的 Skills（用于链式调用）
    async fn find_compatible_skills(
        &self,
        source_skill_id: &str,
        output_type: &str,
    ) -> Result<Vec<SkillMatchResult>> {
        // 查询 skill_compatibility 表
        let mut stmt = self.conn.prepare(
            "SELECT s.skill_id, s.skill_name, c.compatibility_score
             FROM skill_compatibility c
             JOIN skill_semantics s ON c.target_skill_id = s.skill_id
             WHERE c.source_skill_id = ?1
             AND c.data_flow_types LIKE ?2
             ORDER BY c.compatibility_score DESC
             LIMIT 5"
        )?;
        
        let pattern = format!("%{}%", output_type);
        
        let results = stmt.query_map((source_skill_id, pattern), |row| {
            Ok(SkillMatchResult {
                skill_id: row.get(0)?,
                skill_name: row.get(1)?,
                description: String::new(),
                similarity: row.get(2)?,
                match_type: MatchType::Compatibility,
            })
        })?;
        
        results.filter_map(|r| r.ok()).collect::<Vec<_>>().pipe(Ok)
    }
    
    /// 自动发现 Skill 兼容性（后台任务）
    pub async fn auto_discover_compatibility(&self) -> Result<()> {
        let skills: Vec<(String, SkillIoSignature)> = self.conn.prepare(
            "SELECT skill_id, io_signature_json FROM skill_semantics"
        )?.query_map([], |row| {
            let id: String = row.get(0)?;
            let json: String = row.get(1)?;
            let sig: SkillIoSignature = serde_json::from_str(&json).unwrap_or_default();
            Ok((id, sig))
        })?.filter_map(|r| r.ok()).collect();
        
        for (source_id, source_sig) in &skills {
            for (target_id, target_sig) in &skills {
                if source_id == target_id {
                    continue;
                }
                
                // 检查输出/输入兼容性
                for output_type in &source_sig.output_types {
                    if target_sig.input_types.contains(output_type) {
                        let score = 0.85; // 可以计算更复杂的分数
                        
                        self.conn.execute(
                            "INSERT INTO skill_compatibility 
                             (source_skill_id, target_skill_id, compatibility_score, 
                              data_flow_types, discovered_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)
                             ON CONFLICT(source_skill_id, target_skill_id) DO UPDATE SET
                             compatibility_score = excluded.compatibility_score",
                            params![
                                source_id,
                                target_id,
                                score,
                                serde_json::json!({"input": output_type, "output": target_sig.output_types[0]}).to_string(),
                                chrono::Utc::now().timestamp(),
                            ],
                        )?;
                    }
                }
            }
        }
        
        Ok(())
    }
}

/// Skill 匹配结果
#[derive(Debug, Clone)]
pub struct SkillMatchResult {
    pub skill_id: String,
    pub skill_name: String,
    pub description: String,
    pub similarity: f32,
    pub match_type: MatchType,
}

#[derive(Debug, Clone)]
pub enum MatchType {
    Intent,        // 意图匹配
    Capability,    // 能力匹配
    Compatibility, // 链式兼容性匹配
}
```

### 4.2 Intent Parser（意图解析器）

```rust
// cis-core/src/intent/mod.rs

/// 解析后的用户意图
#[derive(Debug, Clone)]
pub struct ParsedIntent {
    /// 原始输入
    pub raw_input: String,
    
    /// 规范化后的意图文本
    pub normalized_intent: String,
    
    /// 向量表示
    pub embedding: Vec<f32>,
    
    /// 提取的实体
    pub entities: HashMap<String, EntityValue>,
    
    /// 置信度
    pub confidence: f32,
    
    /// 识别的动作类型
    pub action_type: ActionType,
}

#[derive(Debug, Clone)]
pub enum EntityValue {
    String(String),
    Number(f64),
    DateTime(chrono::DateTime<chrono::Utc>),
    FilePath(std::path::PathBuf),
    List(Vec<EntityValue>),
}

pub struct IntentParser {
    embedding_service: Arc<dyn EmbeddingService>,
    ner_model: Option<Arc<dyn NERModel>>,  // 命名实体识别
}

impl IntentParser {
    /// 解析用户输入
    pub async fn parse(&self, input: &str) -> Result<ParsedIntent> {
        // 1. 生成嵌入向量
        let embedding = self.embedding_service.embed(input).await?;
        
        // 2. 命名实体识别 (NER)
        let entities = self.extract_entities(input).await?;
        
        // 3. 规范化意图文本（去除实体后的核心意图）
        let normalized = self.normalize_intent(input, &entities);
        
        // 4. 判断动作类型
        let action_type = self.classify_action(input);
        
        Ok(ParsedIntent {
            raw_input: input.to_string(),
            normalized_intent: normalized,
            embedding,
            entities,
            confidence: 0.9, // 可以基于模型输出计算
            action_type,
        })
    }
    
    /// 实体提取
    async fn extract_entities(&self, input: &str) -> Result<HashMap<String, EntityValue>> {
        let mut entities = HashMap::new();
        
        // 时间实体
        if let Some(date) = self.extract_datetime(input) {
            entities.insert("time".to_string(), EntityValue::DateTime(date));
        }
        
        // 文件路径
        if let Some(path) = self.extract_file_path(input) {
            entities.insert("file".to_string(), EntityValue::FilePath(path));
        }
        
        // 数字
        for (i, num) in self.extract_numbers(input).iter().enumerate() {
            entities.insert(format!("number_{}", i), EntityValue::Number(*num));
        }
        
        // 使用 NER 模型提取更多实体
        if let Some(ner) = &self.ner_model {
            let ner_results = ner.extract(input).await?;
            for (key, value) in ner_results {
                entities.insert(key, EntityValue::String(value));
            }
        }
        
        Ok(entities)
    }
    
    /// 动作分类
    fn classify_action(&self, input: &str) -> ActionType {
        let input_lower = input.to_lowercase();
        
        if input_lower.contains("分析") || input_lower.contains("analyze") {
            ActionType::Analyze
        } else if input_lower.contains("生成") || input_lower.contains("create") || input_lower.contains("generate") {
            ActionType::Generate
        } else if input_lower.contains("提交") || input_lower.contains("commit") {
            ActionType::Commit
        } else if input_lower.contains("查询") || input_lower.contains("search") || input_lower.contains("find") {
            ActionType::Query
        } else {
            ActionType::Execute
        }
    }
}
```

### 4.3 Skill Parameter Resolver（参数解析器）

```rust
// cis-core/src/skill/parameter_resolver.rs

pub struct ParameterResolver {
    skill_registry: Arc<SkillVectorRegistry>,
}

impl ParameterResolver {
    /// 将解析的意图映射到 Skill 参数
    pub async fn resolve(
        &self,
        skill_id: &str,
        parsed_intent: &ParsedIntent,
    ) -> Result<ResolvedParameters> {
        // 1. 获取 Skill 的参数 Schema
        let schema: SkillParameterSchema = self.skill_registry.get_parameter_schema(skill_id).await?;
        
        let mut resolved = HashMap::new();
        let mut missing_required = Vec::new();
        
        // 2. 遍历参数定义，尝试从实体中提取
        for param in &schema.parameters {
            if let Some(value) = self.extract_param_value(param, parsed_intent).await? {
                resolved.insert(param.name.clone(), value);
            } else if schema.required.contains(&param.name) {
                missing_required.push(param.name.clone());
            }
        }
        
        // 3. 如果有缺失的必需参数，尝试交互式询问
        if !missing_required.is_empty() {
            return Err(CisError::skill(format!(
                "Missing required parameters: {}",
                missing_required.join(", ")
            )));
        }
        
        Ok(ResolvedParameters {
            params: resolved,
            skill_id: skill_id.to_string(),
        })
    }
    
    /// 提取单个参数值
    async fn extract_param_value(
        &self,
        param: &ParameterDef,
        intent: &ParsedIntent,
    ) -> Result<Option<serde_json::Value>> {
        // 1. 直接匹配实体名称
        if let Some(entity) = intent.entities.get(&param.name) {
            return Ok(Some(self.entity_to_json(entity)?));
        }
        
        // 2. 基于描述的语义匹配
        for (entity_key, entity_value) in &intent.entities {
            let entity_desc = format!("{}: {:?}", entity_key, entity_value);
            let similarity = self.semantic_similarity(&param.description, &entity_desc).await?;
            
            if similarity > 0.8 {
                return Ok(Some(self.entity_to_json(entity_value)?));
            }
        }
        
        // 3. 类型推断
        match param.param_type {
            ParameterType::DateTime => {
                if let Some(date) = intent.entities.get("time") {
                    return Ok(Some(self.entity_to_json(date)?));
                }
            }
            ParameterType::FilePath => {
                if let Some(path) = intent.entities.get("file") {
                    return Ok(Some(self.entity_to_json(path)?));
                }
            }
            _ => {}
        }
        
        Ok(None)
    }
    
    fn entity_to_json(&self, entity: &EntityValue) -> Result<serde_json::Value> {
        match entity {
            EntityValue::String(s) => Ok(serde_json::json!(s)),
            EntityValue::Number(n) => Ok(serde_json::json!(n)),
            EntityValue::DateTime(dt) => Ok(serde_json::json!(dt.to_rfc3339())),
            EntityValue::FilePath(p) => Ok(serde_json::json!(p.to_string_lossy().to_string())),
            EntityValue::List(l) => {
                let arr: Vec<serde_json::Value> = l.iter()
                    .map(|e| self.entity_to_json(e))
                    .collect::<Result<Vec<_>>>()?;
                Ok(serde_json::json!(arr))
            }
        }
    }
}
```

---

## 五、使用示例

### 5.1 注册带有语义描述的 Skill

```rust
// 在 Skill 定义中添加语义描述
pub struct DataAnalyzerSkill;

impl Skill for DataAnalyzerSkill {
    fn name(&self) -> &str { "data-analyzer" }
    
    fn description(&self) -> &str { "分析各种格式的数据并生成洞察" }
    
    /// 提供语义描述用于向量注册
    fn semantics(&self) -> SkillSemantics {
        SkillSemantics {
            skill_id: "data-analyzer".to_string(),
            skill_name: "数据分析器".to_string(),
            example_intents: vec![
                "分析今天的销售数据".to_string(),
                "帮我看看这份CSV文件".to_string(),
                "统计一下用户增长趋势".to_string(),
                "生成数据可视化报告".to_string(),
                "对比上月和本月的业绩".to_string(),
            ],
            parameter_schema: SkillParameterSchema {
                parameters: vec![
                    ParameterDef {
                        name: "data_source".to_string(),
                        param_type: ParameterType::FilePath,
                        description: "数据源文件路径".to_string(),
                        examples: vec!["sales.csv".to_string(), "/data/users.json".to_string()],
                    },
                    ParameterDef {
                        name: "analysis_type".to_string(),
                        param_type: ParameterType::Enum {
                            values: vec!["summary".to_string(), "trend".to_string(), "comparison".to_string()],
                        },
                        description: "分析类型".to_string(),
                        examples: vec!["summary".to_string()],
                    },
                ],
                required: vec!["data_source".to_string()],
                extraction_hints: HashMap::new(),
            },
            io_signature: SkillIoSignature {
                input_types: vec!["text/csv".to_string(), "application/json".to_string()],
                output_types: vec!["application/json".to_string(), "image/png".to_string()],
                pipeable: true,
                source: true,
                sink: false,
            },
            related_skills: vec!["report-gen".to_string(), "chart-viz".to_string()],
            ..Default::default()
        }
    }
}

// 注册时自动索引向量
let registry = SkillVectorRegistry::open_default()?;
registry.register_semantics(skill.semantics()).await?;
```

### 5.2 自然语言调用 Skill

```rust
// cis-node/src/commands/skill.rs

/// 自然语言调用 Skill
pub async fn call_by_intent(query: &str, confirm: bool) -> Result<()> {
    let vector_registry = SkillVectorRegistry::open_default()?;
    let intent_parser = IntentParser::new();
    let param_resolver = ParameterResolver::new();
    
    // 1. 解析用户意图
    let parsed = intent_parser.parse(query).await?;
    println!("🎯 识别意图: {} (置信度: {:.1}%)", 
        parsed.normalized_intent, 
        parsed.confidence * 100.0
    );
    
    // 2. 语义搜索匹配的 Skills
    let matches = vector_registry.search_by_intent(query, 5, 0.6).await?;
    
    if matches.is_empty() {
        println!("❌ 未找到匹配的 Skill");
        return Ok(());
    }
    
    // 显示匹配结果
    println!("\n📋 匹配的 Skills:");
    for (i, m) in matches.iter().enumerate() {
        println!("  {}. {} (相似度: {:.1}%) - {}", 
            i + 1, m.skill_name, m.similarity * 100.0, m.description);
    }
    
    let best_match = &matches[0];
    
    // 3. 解析参数
    let params = param_resolver.resolve(&best_match.skill_id, &parsed).await?;
    
    println!("\n🔧 解析参数:");
    for (k, v) in &params.params {
        println!("  {}: {}", k, v);
    }
    
    // 4. 确认执行
    if confirm {
        print!("\n确认执行? [Y/n]: ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            println!("已取消");
            return Ok(());
        }
    }
    
    // 5. 执行 Skill
    let skill_manager = SkillManager::new()?;
    let result = skill_manager.execute(&best_match.skill_id, "execute", params.to_json()).await?;
    
    println!("\n✅ 执行结果: {:?}", result);
    
    // 6. 记录执行历史（用于优化）
    vector_registry.record_execution(query, &best_match.skill_id, &params, true).await?;
    
    Ok(())
}
```

**CLI 使用**:
```bash
# 自然语言调用（自动匹配 Skill）
cis skill do "分析今天的销售数据"
cis skill do "把这份CSV转成PDF报告"
cis skill do "提交代码，修复了登录bug"
cis skill do "查一下北京明天的天气"

# 查看匹配到的 Skill 链
cis skill chain "分析数据并生成可视化报告" --preview
```

---

## 六、高级功能

### 6.1 自动 Skill 链发现

```rust
// 用户输入: "分析数据并发送邮件"
// 系统自动发现:
// data-analyzer (分析) → report-gen (生成PDF) → email-skill (发送)

let chain = vector_registry.discover_skill_chain("分析数据并发送邮件", 3).await?;

// 执行链
for step in chain.steps {
    let skill = skill_manager.get(&step.skill_id)?;
    let output = skill.execute(context).await?;
    context = context.with_input(output);
}
```

### 6.2 基于反馈的意图优化

```rust
// 用户可以对匹配结果评分
pub async fn feedback_execution(execution_id: &str, score: i32) -> Result<()> {
    // 更新 intent_execution_history
    vector_registry.update_feedback(execution_id, score).await?;
    
    // 如果评分低，可能更新向量索引权重
    if score < 3 {
        // 降低此意图-技能配对的权重
        vector_registry.downgrade_intent_match(execution_id).await?;
    }
    
    Ok(())
}

// CLI: cis skill feedback <execution_id> --score 4
```

### 6.3 项目特定的 Skill 上下文

```rust
// 不同项目可能有不同的 Skill 偏好
// 例如：前端项目常用 "npm-build", "eslint-fix"
//       后端项目常用 "cargo-build", "sql-migrate"

let project_context = ProjectSkillContext::load("my-project")?;

// 优先匹配项目历史使用过的 Skill 组合
let matches = vector_registry
    .search_with_context(query, &project_context, 5)
    .await?;
```

---

## 七、总结

### 7.1 解决的问题

| 问题 | 解决方案 |
|------|----------|
| 必须记住 Skill 名称 | 自然语言语义匹配 |
| 必须知道参数格式 | 智能参数提取和映射 |
| 单步操作效率低 | 自动 Skill 链编排 |
| Skill 发现困难 | 语义搜索 + 能力匹配 |
| 缺乏上下文感知 | 项目特定的 Skill 上下文 |

### 7.2 集成关系

```
Skill Vector Automation
    ├── 扩展: sqlite-vec (Task + Session 向量存储)
    ├── 增强: Skill Registry (语义描述 + 向量索引)
    ├── 新增: Intent Parser (意图解析 + NER)
    ├── 新增: Parameter Resolver (参数映射)
    └── 增强: CLI (自然语言命令)
```

### 7.3 实施路径

**Phase 1: Skill 语义注册** (3天)
- 扩展 Skill trait 添加 semantics()
- 创建 SkillVectorRegistry
- 实现意图向量索引

**Phase 2: 意图解析** (3天)
- 实现 IntentParser
- NER 实体提取
- 参数映射器

**Phase 3: 自动化路由** (2天)
- 语义搜索匹配
- Skill 链发现
- CLI `skill do` 命令

**Phase 4: 优化学习** (2天)
- 执行历史记录
- 反馈优化
- 项目上下文感知

完整文档: `docs/SKILL_VECTOR_AUTOMATION.md`
