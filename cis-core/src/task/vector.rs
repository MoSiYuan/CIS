//! # Task 向量索引
//!
//! 为 Task 提供多字段向量索引，支持标题、描述、结果的语义搜索。
//!
//! ## 功能
//!
//! - 多字段向量索引（标题、描述、结果）
//! - 语义搜索 Task
//! - 查找相似 Task

use std::sync::Arc;

use crate::error::{CisError, Result};
use crate::types::Task;
use crate::vector::VectorStorage;

/// Task 搜索结果
#[derive(Debug, Clone)]
pub struct TaskSearchResult {
    /// Task ID
    pub task_id: String,
    /// Task 标题
    pub title: String,
    /// Task 描述
    pub description: Option<String>,
    /// 匹配字段类型
    pub matched_field: TaskField,
    /// 相似度分数 (0.0 - 1.0)
    pub similarity: f32,
}

/// Task 相似度结果
#[derive(Debug, Clone)]
pub struct TaskSimilarity {
    /// Task ID
    pub task_id: String,
    /// Task 标题
    pub title: String,
    /// 平均相似度（跨字段）
    pub similarity: f32,
    /// 各字段相似度详情
    pub field_similarities: Vec<(TaskField, f32)>,
}

/// Task 字段类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskField {
    /// 标题
    Title,
    /// 描述
    Description,
    /// 结果
    Result,
}

impl std::fmt::Display for TaskField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskField::Title => write!(f, "title"),
            TaskField::Description => write!(f, "description"),
            TaskField::Result => write!(f, "result"),
        }
    }
}

/// Task 向量索引
///
/// 为 Task 建立多字段向量索引，支持语义搜索和相似度匹配。
pub struct TaskVectorIndex {
    vector_storage: Arc<VectorStorage>,
}

impl TaskVectorIndex {
    /// 创建新的 Task 向量索引
    pub fn new(vector_storage: Arc<VectorStorage>) -> Self {
        Self { vector_storage }
    }

    /// 为 Task 建立多字段向量索引
    ///
    /// 索引以下字段：
    /// - 标题（必需）
    /// - 描述（可选）
    /// - 结果（可选，仅当任务完成时）
    ///
    /// # 参数
    /// - `task`: 要索引的 Task
    ///
    /// # 返回
    /// - `Ok(())`: 成功索引
    pub async fn index_task(&self, task: &Task) -> Result<()> {
        // 1. 索引标题
        self.index_task_field(&task.id, &task.title, TaskField::Title).await?;

        // 2. 索引描述（如果存在）
        if let Some(ref desc) = task.description {
            self.index_task_field(&task.id, desc, TaskField::Description).await?;
        }

        // 3. 索引结果（如果存在）
        if let Some(ref result) = task.result {
            self.index_task_field(&task.id, result, TaskField::Result).await?;
        }

        Ok(())
    }

    /// 索引单个 Task 字段
    async fn index_task_field(
        &self,
        task_id: &str,
        text: &str,
        field: TaskField,
    ) -> Result<()> {
        let embedding = self.vector_storage.embedding_service().embed(text).await?;
        let vec_bytes = serialize_f32_vec(&embedding);

        let table_name = match field {
            TaskField::Title => "task_title_vec",
            TaskField::Description => "task_description_vec",
            TaskField::Result => "task_result_vec",
        };

        let conn = self.vector_storage.conn();
        
        // 虚拟表不支持 UPSERT，先 DELETE 后 INSERT
        conn.execute(
            &format!("DELETE FROM {} WHERE task_id = ?1", table_name),
            [task_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete old task {}: {}", field, e)))?;
        
        conn.execute(
            &format!(
                "INSERT INTO {} (task_id, embedding) VALUES (?1, ?2)",
                table_name
            ),
            rusqlite::params![task_id, &vec_bytes],
        ).map_err(|e| CisError::storage(format!("Failed to index task {}: {}", field, e)))?;

        Ok(())
    }

    /// 语义搜索 Task
    ///
    /// 搜索所有字段（标题、描述、结果），返回最相似的结果。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `limit`: 返回结果数量限制
    /// - `threshold`: 相似度阈值 (0.0 - 1.0)
    ///
    /// # 返回
    /// - `Ok(Vec<TaskSearchResult>)`: 搜索结果列表
    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<TaskSearchResult>> {
        let query_vec = self.vector_storage.embedding_service().embed(query).await?;

        // 搜索所有字段
        let mut all_results = Vec::new();

        // 搜索标题
        let title_results = self.search_field(&query_vec, TaskField::Title, limit, threshold).await?;
        for (task_id, similarity) in title_results {
            all_results.push((task_id, TaskField::Title, similarity));
        }

        // 搜索描述
        let desc_results = self.search_field(&query_vec, TaskField::Description, limit, threshold).await?;
        for (task_id, similarity) in desc_results {
            all_results.push((task_id, TaskField::Description, similarity));
        }

        // 搜索结果
        let result_results = self.search_field(&query_vec, TaskField::Result, limit, threshold).await?;
        for (task_id, similarity) in result_results {
            all_results.push((task_id, TaskField::Result, similarity));
        }

        // 按相似度排序并去重（保留每个 Task 的最高相似度字段）
        all_results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

        let mut seen_tasks = std::collections::HashSet::new();
        let mut final_results = Vec::new();

        for (task_id, field, similarity) in all_results {
            if seen_tasks.insert(task_id.clone()) {
                // 获取任务基本信息（这里简化处理，实际应该从 TaskStorage 获取）
                final_results.push(TaskSearchResult {
                    task_id: task_id.clone(),
                    title: task_id.clone(), // 简化：使用 ID 作为标题占位
                    description: None,
                    matched_field: field,
                    similarity,
                });

                if final_results.len() >= limit {
                    break;
                }
            }
        }

        Ok(final_results)
    }

    /// 搜索特定字段
    async fn search_field(
        &self,
        query_vec: &[f32],
        field: TaskField,
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<(String, f32)>> {
        let query_bytes = serialize_f32_vec(query_vec);

        let table_name = match field {
            TaskField::Title => "task_title_vec",
            TaskField::Description => "task_description_vec",
            TaskField::Result => "task_result_vec",
        };

        let conn = self.vector_storage.conn();
        let mut stmt = conn.prepare(
            &format!(
                "SELECT task_id, distance
                 FROM {}
                 WHERE embedding MATCH ?1 AND k = ?2
                 ORDER BY distance",
                table_name
            )
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![&query_bytes, limit as i64],
            |row| {
                let task_id: String = row.get(0)?;
                let distance: f64 = row.get(1)?;
                // 将距离转换为相似度 (假设使用余弦距离，范围[0, 2])
                let similarity = (2.0 - distance as f32) / 2.0;
                Ok((task_id, similarity))
            },
        ).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            let (task_id, similarity) = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            if similarity >= threshold {
                results.push((task_id, similarity));
            }
        }

        Ok(results)
    }

    /// 查找相似 Task
    ///
    /// 基于指定 Task 的所有字段，查找相似的 Task。
    ///
    /// # 参数
    /// - `task_id`: 参考 Task 的 ID
    /// - `threshold`: 相似度阈值 (0.0 - 1.0)
    ///
    /// # 返回
    /// - `Ok(Vec<TaskSimilarity>)`: 相似 Task 列表
    pub async fn find_similar(
        &self,
        task_id: &str,
        threshold: f32,
    ) -> Result<Vec<TaskSimilarity>> {
        // 获取参考 Task 的各字段向量
        let title_vec = self.get_task_vector(task_id, TaskField::Title).await?;
        let desc_vec = self.get_task_vector(task_id, TaskField::Description).await?;
        let result_vec = self.get_task_vector(task_id, TaskField::Result).await?;

        // 搜索各字段的相似 Task
        let mut all_matches: std::collections::HashMap<String, Vec<(TaskField, f32)>> =
            std::collections::HashMap::new();

        // 标题匹配
        if let Some(ref vec) = title_vec {
            let matches = self.search_field(vec, TaskField::Title, 20, threshold).await?;
            for (id, sim) in matches {
                if id != task_id {
                    all_matches.entry(id).or_default().push((TaskField::Title, sim));
                }
            }
        }

        // 描述匹配
        if let Some(ref vec) = desc_vec {
            let matches = self.search_field(vec, TaskField::Description, 20, threshold).await?;
            for (id, sim) in matches {
                if id != task_id {
                    all_matches.entry(id).or_default().push((TaskField::Description, sim));
                }
            }
        }

        // 结果匹配
        if let Some(ref vec) = result_vec {
            let matches = self.search_field(vec, TaskField::Result, 20, threshold).await?;
            for (id, sim) in matches {
                if id != task_id {
                    all_matches.entry(id).or_default().push((TaskField::Result, sim));
                }
            }
        }

        // 计算平均相似度
        let mut similarities: Vec<TaskSimilarity> = all_matches
            .into_iter()
            .map(|(id, field_sims)| {
                let avg_sim: f32 = field_sims.iter().map(|(_, sim)| sim).sum::<f32>() / field_sims.len() as f32;
                TaskSimilarity {
                    task_id: id.clone(),
                    title: id, // 简化：使用 ID 作为标题占位
                    similarity: avg_sim,
                    field_similarities: field_sims,
                }
            })
            .filter(|ts| ts.similarity >= threshold)
            .collect();

        // 按平均相似度排序
        similarities.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

        Ok(similarities)
    }

    /// 获取 Task 字段的向量
    async fn get_task_vector(
        &self,
        task_id: &str,
        field: TaskField,
    ) -> Result<Option<Vec<f32>>> {
        let table_name = match field {
            TaskField::Title => "task_title_vec",
            TaskField::Description => "task_description_vec",
            TaskField::Result => "task_result_vec",
        };

        let conn = self.vector_storage.conn();
        let result: std::result::Result<Option<Vec<u8>>, rusqlite::Error> = conn.query_row(
            &format!("SELECT embedding FROM {} WHERE task_id = ?1", table_name),
            [task_id],
            |row| row.get(0),
        );

        match result {
            Ok(Some(bytes)) => Ok(Some(deserialize_f32_vec(&bytes))),
            Ok(None) => Ok(None),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(CisError::storage(format!("Failed to get task vector: {}", e))),
        }
    }

    /// 删除 Task 的向量索引
    pub fn delete_task_index(&self, task_id: &str) -> Result<bool> {
        let conn = self.vector_storage.conn();
        let mut deleted = false;

        // 删除标题索引
        let rows = conn.execute(
            "DELETE FROM task_title_vec WHERE task_id = ?1",
            [task_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete task title index: {}", e)))?;
        deleted |= rows > 0;

        // 删除描述索引
        let rows = conn.execute(
            "DELETE FROM task_description_vec WHERE task_id = ?1",
            [task_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete task description index: {}", e)))?;
        deleted |= rows > 0;

        // 删除结果索引
        let rows = conn.execute(
            "DELETE FROM task_result_vec WHERE task_id = ?1",
            [task_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete task result index: {}", e)))?;
        deleted |= rows > 0;

        Ok(deleted)
    }
}

/// 序列化 f32 向量为字节
fn serialize_f32_vec(vec: &[f32]) -> Vec<u8> {
    vec.iter()
        .flat_map(|&f| f.to_le_bytes())
        .collect()
}

/// 反序列化字节为 f32 向量
fn deserialize_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use crate::types::{TaskStatus, TaskPriority};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::collections::HashMap;

    /// 模拟 embedding service（用于测试）
    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            // 简单的确定性模拟：根据文本哈希生成向量
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let hash = text.bytes().fold(0u64, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as u64)
            });
            for i in 0..DEFAULT_EMBEDDING_DIM {
                let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                vec[i] = val;
            }
            // 归一化
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            Ok(vec)
        }

        async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text).await?);
            }
            Ok(results)
        }
    }

    fn create_test_task(id: &str, title: &str, description: Option<&str>) -> Task {
        Task {
            id: id.to_string(),
            parent_id: None,
            title: title.to_string(),
            description: description.map(|s| s.to_string()),
            group_name: "test".to_string(),
            completion_criteria: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Medium,
            dependencies: Vec::new(),
            result: None,
            error: None,
            workspace_dir: None,
            sandboxed: true,
            allow_network: false,
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            node_id: None,
            metadata: HashMap::new(),
            level: crate::types::TaskLevel::Mechanical { retry: 3 },
            on_ambiguity: crate::types::AmbiguityPolicy::AutoBest,
            inputs: Vec::new(),
            outputs: Vec::new(),
            rollback: None,
            idempotent: false,
            failure_type: None,
            skill_id: None,
            skill_params: None,
            skill_result: None,
        }
    }

    fn setup_test_index() -> (TaskVectorIndex, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = VectorStorage::open_with_service(&db_path, embedding).unwrap();
        let index = TaskVectorIndex::new(Arc::new(storage));
        (index, temp_dir)
    }

    #[tokio::test]
    async fn test_index_task() {
        let (index, _temp) = setup_test_index();

        let task = create_test_task("task-1", "优化数据库查询性能", Some("使用索引加速查询"));
        
        // 索引 Task
        index.index_task(&task).await.unwrap();

        // 验证可以搜索到（使用较低阈值因为 mock embedding 的相似度可能不高）
        let results = index.semantic_search("数据库优化", 5, 0.0).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_semantic_search() {
        let (index, _temp) = setup_test_index();

        // 创建并索引多个 Task
        let task1 = create_test_task("task-1", "优化数据库查询性能", Some("使用索引加速查询"));
        let task2 = create_test_task("task-2", "修复前端 CSS 样式", Some("调整按钮颜色"));
        let task3 = create_test_task("task-3", "数据库备份脚本", Some("定期备份 MySQL"));

        index.index_task(&task1).await.unwrap();
        index.index_task(&task2).await.unwrap();
        index.index_task(&task3).await.unwrap();

        // 搜索数据库相关
        let results = index.semantic_search("数据库", 5, 0.5).await.unwrap();
        assert!(!results.is_empty());
        
        // 应该找到 task-1 和 task-3
        let task_ids: Vec<_> = results.iter().map(|r| r.task_id.clone()).collect();
        assert!(task_ids.contains(&"task-1".to_string()));
        assert!(task_ids.contains(&"task-3".to_string()));
    }

    #[tokio::test]
    async fn test_find_similar() {
        let (index, _temp) = setup_test_index();

        // 创建相似的任务
        let task1 = create_test_task("task-1", "优化数据库查询性能", Some("使用索引加速查询"));
        let task2 = create_test_task("task-2", "优化 API 响应速度", Some("使用缓存减少查询"));
        let task3 = create_test_task("task-3", "修复前端 CSS 样式", Some("调整按钮颜色"));

        index.index_task(&task1).await.unwrap();
        index.index_task(&task2).await.unwrap();
        index.index_task(&task3).await.unwrap();

        // 查找与 task-1 相似的任务
        let similar = index.find_similar("task-1", 0.5).await.unwrap();
        
        // task-2 应该与 task-1 相似（都是优化相关）
        assert!(!similar.is_empty());
    }
}
