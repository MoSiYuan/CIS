//! # DAG 管理
//!
//! 负责任务依赖关系的管理、拓扑排序和循环检测。
//!
//! ## 核心职责
//! - DAG 节点管理
//! - 依赖关系维护
//! - 拓扑排序 (Kahn 算法)
//! - 循环依赖检测
//!
//! ## 设计原则
//! - 使用 task 模块的 DagBuilder 进行构建，避免重复实现
//! - 仅负责调度层面的 DAG 管理，不涉及持久化

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::scheduler::DagNodeStatus;
use crate::task::{Dag, DagNode as TaskDagNode, repository::TaskRepository};

/// DAG 调度器错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerDagError {
    /// 循环依赖检测
    CycleDetected(Vec<String>),
    /// 节点未找到
    NodeNotFound(String),
    /// 依赖节点未找到
    DependencyNotFound(String),
    /// 无效操作
    InvalidOperation(String),
}

impl std::fmt::Display for SchedulerDagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CycleDetected(cycle) => {
                write!(f, "Cycle detected in dependency graph: {:?}", cycle)
            }
            Self::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            Self::DependencyNotFound(dep) => write!(f, "Dependency not found: {}", dep),
            Self::InvalidOperation(op) => write!(f, "Invalid operation: {}", op),
        }
    }
}

impl std::error::Error for SchedulerDagError {}

/// DAG 调度器
///
/// 负责任务的依赖关系管理和执行顺序计算。
pub struct DagScheduler {
    /// DAG 节点映射
    nodes: HashMap<String, SchedulerDagNode>,
    /// 根节点（无依赖）
    root_nodes: Vec<String>,
    /// 任务仓储（用于加载任务信息）
    task_repo: Arc<TaskRepository>,
}

/// DAG 调度节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerDagNode {
    /// 节点 ID
    pub id: String,
    /// 依赖节点列表
    pub dependencies: Vec<String>,
    /// 依赖此节点的节点列表
    pub dependents: Vec<String>,
    /// 节点状态
    pub status: DagNodeStatus,
    /// 节点深度（用于拓扑层级计算）
    pub depth: usize,
}

impl DagScheduler {
    /// 创建新的 DAG 调度器
    pub fn new(task_repo: Arc<TaskRepository>) -> Self {
        Self {
            nodes: HashMap::new(),
            root_nodes: Vec::new(),
            task_repo,
        }
    }

    /// 从 TaskDag 构建调度 DAG
    ///
    /// 复用 task 模块的 DagBuilder 进行构建，仅做状态转换。
    pub async fn from_task_dag(&mut self, task_dag: Dag) -> Result<(), SchedulerDagError> {
        // 清空现有状态
        self.nodes.clear();
        self.root_nodes.clear();

        // 转换 TaskDag 节点
        for (id, task_node) in task_dag.nodes {
            let scheduler_node = SchedulerDagNode {
                id: id.clone(),
                dependencies: task_node.dependencies.iter().map(|d| d.to_string()).collect(),
                dependents: task_node.dependents.iter().map(|d| d.to_string()).collect(),
                status: DagNodeStatus::Pending,
                depth: task_node.depth,
            };

            self.nodes.insert(id.clone(), scheduler_node);
        }

        // 找出根节点
        for id in self.nodes.keys() {
            if self.is_root_node(id) {
                self.root_nodes.push(id.clone());
            }
        }

        // 验证 DAG
        self.validate()?;

        Ok(())
    }

    /// 检查是否为根节点（无依赖）
    fn is_root_node(&self, node_id: &str) -> bool {
        self.nodes
            .get(node_id)
            .map(|node| node.dependencies.is_empty())
            .unwrap_or(false)
    }

    /// 验证 DAG 结构
    fn validate(&self) -> Result<(), SchedulerDagError> {
        // 检测循环依赖
        self.detect_cycles()?;

        // 验证依赖关系完整性
        for (id, node) in &self.nodes {
            for dep_id in &node.dependencies {
                if !self.nodes.contains_key(dep_id) {
                    return Err(SchedulerDagError::DependencyNotFound(format!(
                        "Node {} depends on non-existent node {}",
                        id, dep_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// 检测循环依赖（使用 DFS）
    fn detect_cycles(&self) -> Result<(), SchedulerDagError> {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        let mut path = Vec::new();

        for node_id in self.nodes.keys() {
            if !visited.contains(node_id) {
                if let Some(cycle) = self.detect_cycle_dfs(node_id, &mut visited, &mut rec_stack, &mut path) {
                    return Err(SchedulerDagError::CycleDetected(cycle));
                }
            }
        }

        Ok(())
    }

    /// DFS 检测循环
    fn detect_cycle_dfs(
        &self,
        node_id: &str,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        visited.insert(node_id.to_string());
        rec_stack.insert(node_id.to_string());
        path.push(node_id.to_string());

        if let Some(node) = self.nodes.get(node_id) {
            for dep_id in &node.dependencies {
                if !visited.contains(dep_id) {
                    if let Some(cycle) = self.detect_cycle_dfs(dep_id, visited, rec_stack, path) {
                        return Some(cycle);
                    }
                } else if rec_stack.contains(dep_id) {
                    // 找到循环，返回循环路径
                    let cycle_start = path.iter().position(|id| id == dep_id).unwrap_or(0);
                    return Some(path[cycle_start..].to_vec());
                }
            }
        }

        path.pop();
        rec_stack.remove(node_id);
        None
    }

    /// 获取可执行节点（所有依赖已完成）
    pub fn get_ready_nodes(&self) -> Vec<String> {
        let mut ready = Vec::new();

        for (id, node) in &self.nodes {
            if node.status == DagNodeStatus::Pending || node.status == DagNodeStatus::Ready {
                // 检查所有依赖是否完成
                if self.are_dependencies_completed(id) {
                    ready.push(id.clone());
                }
            }
        }

        ready
    }

    /// 检查节点的所有依赖是否完成
    fn are_dependencies_completed(&self, node_id: &str) -> bool {
        self.nodes
            .get(node_id)
            .map(|node| {
                node
                    .dependencies
                    .iter()
                    .all(|dep_id| self.is_completed(dep_id))
            })
            .unwrap_or(false)
    }

    /// 检查节点是否完成
    fn is_completed(&self, node_id: &str) -> bool {
        self.nodes
            .get(node_id)
            .map(|node| {
                matches!(
                    node.status,
                    DagNodeStatus::Completed | DagNodeStatus::Skipped
                )
            })
            .unwrap_or(false)
    }

    /// 更新节点状态
    pub fn update_node_status(&mut self, node_id: &str, status: DagNodeStatus) -> Result<(), SchedulerDagError> {
        let node = self
            .nodes
            .get_mut(node_id)
            .ok_or_else(|| SchedulerDagError::NodeNotFound(node_id.to_string()))?;

        node.status = status;
        Ok(())
    }

    /// 获取节点信息
    pub fn get_node(&self, node_id: &str) -> Option<&SchedulerDagNode> {
        self.nodes.get(node_id)
    }

    /// 获取所有节点
    pub fn get_all_nodes(&self) -> &HashMap<String, SchedulerDagNode> {
        &self.nodes
    }

    /// 获取根节点
    pub fn get_root_nodes(&self) -> &[String] {
        &self.root_nodes
    }

    /// 计算拓扑排序层级
    ///
    /// 返回按层级组织的任务 ID 列表，同一层的任务可以并行执行。
    pub fn topological_levels(&self) -> Result<Vec<Vec<String>>, SchedulerDagError> {
        // 按深度分组
        let mut levels_map: HashMap<usize, Vec<String>> = HashMap::new();

        for (id, node) in &self.nodes {
            levels_map
                .entry(node.depth)
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        // 按层级排序
        let mut levels: Vec<_> = levels_map.into_iter().collect();
        levels.sort_by_key(|(depth, _)| *depth);

        let result = levels.into_iter().map(|(_, ids)| ids).collect();

        Ok(result)
    }

    /// Kahn 算法进行拓扑排序
    ///
    /// 返回按依赖顺序排列的任务 ID 列表。
    pub fn topological_sort(&self) -> Result<Vec<String>, SchedulerDagError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result = Vec::new();

        // 计算入度
        for (id, node) in &self.nodes {
            in_degree.insert(id.clone(), node.dependencies.len());
            if node.dependencies.is_empty() {
                queue.push_back(id.clone());
            }
        }

        // Kahn 算法主循环
        while let Some(node_id) = queue.pop_front() {
            result.push(node_id.clone());

            // 减少依赖此节点的其他节点的入度
            if let Some(node) = self.nodes.get(&node_id) {
                for dependent_id in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(dependent_id) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(dependent_id.clone());
                        }
                    }
                }
            }
        }

        // 检查是否所有节点都被处理（检测循环）
        if result.len() != self.nodes.len() {
            return Err(SchedulerDagError::CycleDetected(vec![
                "Cycle detected during topological sort".to_string()
            ]));
        }

        Ok(result)
    }

    /// 检查 DAG 是否已完成（所有节点都完成或跳过）
    pub fn is_completed(&self) -> bool {
        self.nodes.values().all(|node| {
            matches!(
                node.status,
                DagNodeStatus::Completed | DagNodeStatus::Skipped | DagNodeStatus::Failed
            )
        })
    }

    /// 获取失败节点列表
    pub fn get_failed_nodes(&self) -> Vec<String> {
        self.nodes
            .iter()
            .filter(|(_, node)| matches!(node.status, DagNodeStatus::Failed))
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// 重置 DAG 状态（用于重试）
    pub fn reset(&mut self) {
        for node in self.nodes.values_mut() {
            node.status = DagNodeStatus::Pending;
        }
    }

    /// 获取节点统计信息
    pub fn get_stats(&self) -> DagStats {
        let mut stats = DagStats::default();

        for node in self.nodes.values() {
            match node.status {
                DagNodeStatus::Pending => stats.pending += 1,
                DagNodeStatus::Ready => stats.ready += 1,
                DagNodeStatus::Running => stats.running += 1,
                DagNodeStatus::Completed => stats.completed += 1,
                DagNodeStatus::Failed => stats.failed += 1,
                DagNodeStatus::Skipped => stats.skipped += 1,
                DagNodeStatus::Arbitrated => stats.arbitrated += 1,
                DagNodeStatus::Debt(_) => stats.debt += 1,
            }
        }

        stats.total = self.nodes.len();

        stats
    }
}

/// DAG 统计信息
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DagStats {
    /// 总节点数
    pub total: usize,
    /// 待执行
    pub pending: usize,
    /// 就绪
    pub ready: usize,
    /// 运行中
    pub running: usize,
    /// 已完成
    pub completed: usize,
    /// 失败
    pub failed: usize,
    /// 跳过
    pub skipped: usize,
    /// 仲裁中
    pub arbitrated: usize,
    /// 技术债务
    pub debt: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TaskLevel;

    #[test]
    fn test_topological_sort() {
        // 简单的 DAG: 1 -> 2 -> 3
        let mut scheduler = DagScheduler::new(Arc::new(TaskRepository::new_in_memory()));

        // 手动添加节点用于测试
        scheduler.nodes.insert(
            "1".to_string(),
            SchedulerDagNode {
                id: "1".to_string(),
                dependencies: vec![],
                dependents: vec!["2".to_string()],
                status: DagNodeStatus::Pending,
                depth: 0,
            },
        );
        scheduler.nodes.insert(
            "2".to_string(),
            SchedulerDagNode {
                id: "2".to_string(),
                dependencies: vec!["1".to_string()],
                dependents: vec!["3".to_string()],
                status: DagNodeStatus::Pending,
                depth: 1,
            },
        );
        scheduler.nodes.insert(
            "3".to_string(),
            SchedulerDagNode {
                id: "3".to_string(),
                dependencies: vec!["2".to_string()],
                dependents: vec![],
                status: DagNodeStatus::Pending,
                depth: 2,
            },
        );
        scheduler.root_nodes.push("1".to_string());

        let sorted = scheduler.topological_sort().unwrap();
        assert_eq!(sorted, vec!["1", "2", "3"]);
    }

    #[test]
    fn test_cycle_detection() {
        let mut scheduler = DagScheduler::new(Arc::new(TaskRepository::new_in_memory()));

        // 创建循环: 1 -> 2 -> 1
        scheduler.nodes.insert(
            "1".to_string(),
            SchedulerDagNode {
                id: "1".to_string(),
                dependencies: vec!["2".to_string()],
                dependents: vec!["2".to_string()],
                status: DagNodeStatus::Pending,
                depth: 0,
            },
        );
        scheduler.nodes.insert(
            "2".to_string(),
            SchedulerDagNode {
                id: "2".to_string(),
                dependencies: vec!["1".to_string()],
                dependents: vec!["1".to_string()],
                status: DagNodeStatus::Pending,
                depth: 0,
            },
        );

        let result = scheduler.validate();
        assert!(result.is_err());
    }
}
