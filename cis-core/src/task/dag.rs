//! # DAG 构建器和依赖解析
//!
//! 提供任务依赖关系管理、DAG 构建和拓扑排序功能。

use super::models::TaskEntity;
use super::repository::TaskRepository;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// DAG 节点
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: i64,
    pub task_id: String,
    pub name: String,
    pub dependencies: Vec<i64>,
    pub dependents: Vec<i64>,
    pub depth: usize,
}

/// DAG 构建器
pub struct DagBuilder {
    task_repo: Arc<TaskRepository>,
    node_cache: HashMap<i64, DagNode>,
}

impl DagBuilder {
    /// 创建新的 DAG 构建器
    pub fn new(task_repo: Arc<TaskRepository>) -> Self {
        Self {
            task_repo,
            node_cache: HashMap::new(),
        }
    }

    /// 为指定任务构建 DAG
    pub async fn build(&mut self, task_ids: &[String]) -> Result<Dag, DagError> {
        // 1. 加载所有任务节点
        let mut nodes = Vec::new();
        for task_id in task_ids {
            let task = self.task_repo.get_by_task_id(task_id).await?
                .ok_or_else(|| DagError::TaskNotFound(task_id.clone()))?;

            let dependencies = self.resolve_dependency_ids(&task).await?;
            let node = DagNode {
                id: task.id,
                task_id: task.task_id.clone(),
                name: task.name.clone(),
                dependencies: dependencies.clone(),
                dependents: Vec::new(),
                depth: 0,
            };

            self.node_cache.insert(task.id, node);
            nodes.push(task.id);
        }

        // 2. 构建依赖关系图
        self.build_dependency_graph().await?;

        // 3. 计算节点深度
        self.calculate_depths();

        // 4. 检测循环依赖
        self.detect_cycles()?;

        Ok(Dag {
            nodes: self.node_cache.clone(),
            roots: self.find_roots(),
        })
    }

    /// 解析依赖任务的 ID
    async fn resolve_dependency_ids(&self, task: &TaskEntity) -> Result<Vec<i64>, DagError> {
        let mut dep_ids = Vec::new();

        for dep_task_id in &task.dependencies {
            let dep_task = self.task_repo.get_by_task_id(dep_task_id).await?
                .ok_or_else(|| DagError::DependencyNotFound(dep_task_id.clone()))?;

            dep_ids.push(dep_task.id);
        }

        Ok(dep_ids)
    }

    /// 构建依赖关系图
    async fn build_dependency_graph(&mut self) -> Result<(), DagError> {
        let mut graph: HashMap<i64, Vec<i64>> = HashMap::new();

        // 构建邻接表
        for (id, node) in &self.node_cache {
            let entry = graph.entry(*id).or_insert_with(Vec::new);
            for dep_id in &node.dependencies {
                entry.push(*dep_id);
            }
        }

        // 更新节点的 dependents 列表
        for (id, node) in &mut self.node_cache {
            node.dependents = graph.get(id).cloned().unwrap_or_default();
        }

        Ok(())
    }

    /// 计算节点深度（用于拓扑排序）
    fn calculate_depths(&mut self) {
        let mut depths: HashMap<i64, usize> = HashMap::new();

        // 使用 BFS 计算深度
        let ids: Vec<_> = self.node_cache.keys().copied().collect();
        for id in ids {
            let depth = self.calculate_depth_recursive(id, &mut depths);
            if let Some(node) = self.node_cache.get_mut(&id) {
                node.depth = depth;
            }
        }
    }

    /// 递归计算节点深度
    fn calculate_depth_recursive(&self, id: i64, memo: &mut HashMap<i64, usize>) -> usize {
        if let Some(&depth) = memo.get(&id) {
            return depth;
        }

        let node = self.node_cache.get(&id);
        let depth = match node {
            Some(n) if n.dependencies.is_empty() => 0,
            Some(n) => {
                let max_dep_depth = n.dependencies.iter()
                    .map(|dep_id| self.calculate_depth_recursive(*dep_id, memo))
                    .max()
                    .unwrap_or(0);
                max_dep_depth + 1
            }
            None => 0,
        };

        memo.insert(id, depth);
        depth
    }

    /// 检测循环依赖
    fn detect_cycles(&self) -> Result<(), DagError> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for id in self.node_cache.keys() {
            if !visited.contains(id) {
                if self.detect_cycle_recursive(*id, &mut visited, &mut rec_stack)? {
                    let cycle_path = self.extract_cycle_path(*id);
                    return Err(DagError::CycleDetected(cycle_path));
                }
            }
        }

        Ok(())
    }

    /// 递归检测循环
    fn detect_cycle_recursive(
        &self,
        id: i64,
        visited: &mut HashSet<i64>,
        rec_stack: &mut HashSet<i64>,
    ) -> Result<bool, DagError> {
        visited.insert(id);
        rec_stack.insert(id);

        if let Some(node) = self.node_cache.get(&id) {
            for &dep_id in &node.dependencies {
                if !visited.contains(&dep_id) {
                    if self.detect_cycle_recursive(dep_id, visited, rec_stack)? {
                        return Ok(true);
                    }
                } else if rec_stack.contains(&dep_id) {
                    return Ok(true);
                }
            }
        }

        rec_stack.remove(&id);
        Ok(false)
    }

    /// 提取循环路径
    fn extract_cycle_path(&self, start_id: i64) -> Vec<String> {
        let mut path = Vec::new();
        let mut visited = HashSet::new();
        let mut current = Some(start_id);

        while let Some(id) = current {
            if visited.contains(&id) {
                break;
            }

            visited.insert(id);

            if let Some(node) = self.node_cache.get(&id) {
                path.push(node.task_id.clone());
                current = node.dependencies.first().copied();
            } else {
                break;
            }
        }

        path
    }

    /// 找到所有根节点（没有依赖的节点）
    fn find_roots(&self) -> Vec<i64> {
        self.node_cache.iter()
            .filter(|(_, node)| node.dependencies.is_empty())
            .map(|(id, _)| *id)
            .collect()
    }
}

/// DAG 结构
#[derive(Debug, Clone)]
pub struct Dag {
    pub nodes: HashMap<i64, DagNode>,
    pub roots: Vec<i64>,
}

impl Dag {
    /// 拓扑排序（Kahn 算法）
    pub fn topological_sort(&self) -> Result<Vec<i64>, DagError> {
        let mut in_degree: HashMap<i64, usize> = HashMap::new();
        let mut result = Vec::new();
        let mut queue = Vec::new();

        // 1. 计算入度
        for (&id, node) in &self.nodes {
            let degree = node.dependencies.len();
            in_degree.insert(id, degree);
            if degree == 0 {
                queue.push(id);
            }
        }

        // 2. 处理队列
        while let Some(id) = queue.pop() {
            result.push(id);

            if let Some(node) = self.nodes.get(&id) {
                for &dependent_id in &node.dependents {
                    let degree = in_degree.get_mut(&dependent_id).unwrap();
                    *degree -= 1;
                    if *degree == 0 {
                        queue.push(dependent_id);
                    }
                }
            }
        }

        // 3. 检查是否有环
        if result.len() != self.nodes.len() {
            return Err(DagError::CycleDetected(vec![]));
        }

        Ok(result)
    }

    /// 获取可并行执行的层级
    pub fn get_execution_levels(&self) -> Vec<Vec<i64>> {
        let mut levels = Vec::new();
        let mut processed = HashSet::new();
        let mut current_level = self.roots.clone();

        while !current_level.is_empty() {
            levels.push(current_level.clone());

            for id in &current_level {
                processed.insert(*id);
            }

            let mut next_level = Vec::new();
            for id in &current_level {
                if let Some(node) = self.nodes.get(id) {
                    for &dep_id in &node.dependents {
                        // 检查所有依赖是否已处理
                        if let Some(dep_node) = self.nodes.get(&dep_id) {
                            let all_deps_processed = dep_node.dependencies.iter()
                                .all(|d| processed.contains(d));

                            if all_deps_processed && !processed.contains(&dep_id) {
                                next_level.push(dep_id);
                            }
                        }
                    }
                }
            }

            current_level = next_level;
        }

        levels
    }

    /// 获取指定节点的依赖链
    pub fn get_dependency_chain(&self, task_id: i64) -> Vec<i64> {
        let mut chain = Vec::new();
        let mut current = Some(task_id);

        while let Some(id) = current {
            chain.push(id);
            current = self.nodes.get(&id)
                .and_then(|n| n.dependencies.first().copied());
        }

        chain.reverse();
        chain
    }
}

/// DAG 错误类型
#[derive(Debug, thiserror::Error)]
pub enum DagError {
    #[error("任务不存在: {0}")]
    TaskNotFound(String),

    #[error("依赖任务不存在: {0}")]
    DependencyNotFound(String),

    #[error("检测到循环依赖: {0:?}")]
    CycleDetected(Vec<String>),

    #[error("数据库错误: {0}")]
    DatabaseError(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::db::create_database_pool;
    use crate::task::models::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_dag_build_simple() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let mut builder = DagBuilder::new(repo.clone());

        // 创建任务: A -> B -> C
        let task_a = create_test_task("A", vec![]);
        let task_b = create_test_task("B", vec!["A".to_string()]);
        let task_c = create_test_task("C", vec!["B".to_string()]);

        repo.create(&task_a).await.unwrap();
        repo.create(&task_b).await.unwrap();
        repo.create(&task_c).await.unwrap();

        let dag = builder.build(&["A".to_string(), "B".to_string(), "C".to_string()])
            .await
            .unwrap();

        assert_eq!(dag.roots.len(), 1); // 只有 A 是根节点
        assert!(dag.roots.contains(&task_a.id)); // A 的 ID

        // 拓扑排序
        let sorted = dag.topological_sort().unwrap();
        assert!(sorted.len() == 3);
        // A 必须在 B 前面，B 必须在 C 前面
        let pos_a = sorted.iter().position(|&id| id == task_a.id).unwrap();
        let pos_b = sorted.iter().position(|&id| id == task_b.id).unwrap();
        let pos_c = sorted.iter().position(|&id| id == task_c.id).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[tokio::test]
    async fn test_dag_cycle_detection() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let mut builder = DagBuilder::new(repo.clone());

        // 创建循环依赖: A -> B -> C -> A
        let task_a = create_test_task("A", vec!["C".to_string()]);
        let task_b = create_test_task("B", vec!["A".to_string()]);
        let task_c = create_test_task("C", vec!["B".to_string()]);

        repo.create(&task_a).await.unwrap();
        repo.create(&task_b).await.unwrap();
        repo.create(&task_c).await.unwrap();

        let result = builder.build(&["A".to_string(), "B".to_string(), "C".to_string()]).await;

        assert!(matches!(result, Err(DagError::CycleDetected(_))));
    }

    #[tokio::test]
    async fn test_execution_levels() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = create_database_pool(Some(db_path), 5).await;
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let mut builder = DagBuilder::new(repo.clone());

        // 创建任务:
        // Level 0: A, B
        // Level 1: C (依赖 A)
        // Level 2: D (依赖 B, C)
        let task_a = create_test_task("A", vec![]);
        let task_b = create_test_task("B", vec![]);
        let task_c = create_test_task("C", vec!["A".to_string()]);
        let task_d = create_test_task("D", vec!["B".to_string(), "C".to_string()]);

        repo.create(&task_a).await.unwrap();
        repo.create(&task_b).await.unwrap();
        repo.create(&task_c).await.unwrap();
        repo.create(&task_d).await.unwrap();

        let dag = builder.build(&["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string()])
            .await
            .unwrap();

        let levels = dag.get_execution_levels();
        assert_eq!(levels.len(), 3);

        // Level 0: A, B
        assert_eq!(levels[0].len(), 2);
        // Level 1: C
        assert_eq!(levels[1].len(), 1);
        // Level 2: D
        assert_eq!(levels[2].len(), 1);
    }

    fn create_test_task(task_id: &str, dependencies: Vec<String>) -> TaskEntity {
        TaskEntity {
            id: 0,
            task_id: task_id.to_string(),
            name: format!("Task {}", task_id),
            task_type: TaskType::CodeReview,
            priority: TaskPriority::P0,
            prompt_template: "Test".to_string(),
            context_variables: serde_json::json!({}),
            description: None,
            estimated_effort_days: None,
            dependencies,
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: None,
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        }
    }
}
