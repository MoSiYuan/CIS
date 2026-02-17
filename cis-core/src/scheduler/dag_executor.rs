//! DAG Skill 执行器

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::error::{CisError, Result};
use crate::skill::{SkillExecutor, ExecutionContext, ExecutionResult};

/// DAG 节点
#[derive(Debug, Clone)]
pub struct DagNode {
    pub id: String,
    pub skill_name: String,
    pub method: String,
    pub params: Vec<u8>,
    pub dependencies: Vec<String>,
}

/// DAG 定义
#[derive(Debug, Clone)]
pub struct DagDefinition {
    pub id: String,
    pub name: String,
    pub nodes: Vec<DagNode>,
}

/// DAG 执行状态
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    Pending,
    Running,
    Completed(ExecutionResult),
    Failed(String),
}

/// DAG 执行器
pub struct DagExecutor {
    skill_executor: Arc<dyn SkillExecutor>,
}

impl DagExecutor {
    pub fn new(skill_executor: Arc<dyn SkillExecutor>) -> Self {
        Self { skill_executor }
    }
    
    /// 执行 DAG (并行版本 - P0-5 修复)
    ///
    /// ## 性能优化 (P0-5)
    ///
    /// **改进前**: 顺序执行所有节点，未利用并行性
    /// **改进后**: 按依赖层级并行执行，充分利用并发能力
    ///
    /// ### 执行策略
    ///
    /// ```text
    /// 示例 DAG:
    ///   A ─┬─> C ──> E
    ///       │
    ///   B ──┘
    ///
    /// 执行流程:
    /// Level 0: [A, B] 并行执行 ──┐
    /// Level 1: [C]      等待A,B ──┤
    /// Level 2: [E]      等待C   ──┘
    /// ```
    pub async fn execute(&self, dag: DagDefinition, context: ExecutionContext) -> Result<HashMap<String, ExecutionResult>> {
        // 验证 DAG
        self.validate_dag(&dag)?;

        // 构建依赖图和状态
        let mut status: HashMap<String, NodeStatus> = HashMap::new();
        for node in &dag.nodes {
            status.insert(node.id.clone(), NodeStatus::Pending);
        }

        let status = Arc::new(RwLock::new(status));
        let mut results = HashMap::new();

        // 按依赖层级分组执行
        let mut remaining_nodes: HashSet<String> = dag.nodes.iter().map(|n| n.id.clone()).collect();

        loop {
            // 找出所有依赖已满足的节点（可并行执行）
            let ready_nodes: Vec<DagNode> = dag.nodes.iter()
                .filter(|node| {
                    // 节点还未完成
                    if !remaining_nodes.contains(&node.id) {
                        return false;
                    }

                    // 检查所有依赖是否已完成
                    node.dependencies.iter().all(|dep_id| {
                        let status = status.try_read().ok();
                        match status {
                            Some(s) => matches!(s.get(dep_id), Some(NodeStatus::Completed(_))),
                            None => false,
                        }
                    })
                })
                .cloned()
                .collect();

            if ready_nodes.is_empty() {
                // 没有可执行的节点，检查是否全部完成
                if remaining_nodes.is_empty() {
                    break;  // 全部完成
                } else {
                    return Err(CisError::execution(
                        "Deadlock detected: circular dependencies or unresolved dependencies".to_string()
                    ));
                }
            }

            // 并行执行当前层的所有节点
            let mut handles = vec![];
            for node in ready_nodes {
                remaining_nodes.remove(&node.id);

                let executor = self.skill_executor.clone();
                let status = status.clone();
                let context = context.clone();
                let node_id = node.id.clone();

                let handle = tokio::spawn(async move {
                    // 更新状态为运行中
                    {
                        let mut status = status.write().await;
                        status.insert(node_id.clone(), NodeStatus::Running);
                    }

                    // 执行 Skill
                    let result = executor.execute(
                        &node.skill_name,
                        &node.method,
                        &node.params,
                        context
                    ).await;

                    // 更新状态
                    let mut status = status.write().await;
                    match result {
                        Ok(exec_result) => {
                            status.insert(node_id.clone(), NodeStatus::Completed(exec_result.clone()));
                            Ok((node_id, exec_result))
                        }
                        Err(e) => {
                            status.insert(node_id.clone(), NodeStatus::Failed(e.to_string()));
                            Err(e)
                        }
                    }
                });

                handles.push(handle);
            }

            // 等待当前层的所有节点完成
            for handle in handles {
                match handle.await {
                    Ok(Ok((node_id, result))) => {
                        results.insert(node_id, result);
                    }
                    Ok(Err(e)) => {
                        return Err(CisError::execution(format!("Node execution failed: {}", e)));
                    }
                    Err(e) => {
                        return Err(CisError::execution(format!("Node task panicked: {}", e)));
                    }
                }
            }
        }

        Ok(results)
    }
    
    /// 验证 DAG 是否有循环依赖
    fn validate_dag(&self, dag: &DagDefinition) -> Result<()> {
        let mut visited = HashSet::new();
        let mut recursion_stack = HashSet::new();
        
        fn has_cycle(
            node_id: &str,
            dag: &DagDefinition,
            visited: &mut HashSet<String>,
            recursion_stack: &mut HashSet<String>,
        ) -> bool {
            visited.insert(node_id.to_string());
            recursion_stack.insert(node_id.to_string());
            
            if let Some(node) = dag.nodes.iter().find(|n| n.id == node_id) {
                for dep_id in &node.dependencies {
                    if !visited.contains(dep_id) {
                        if has_cycle(dep_id, dag, visited, recursion_stack) {
                            return true;
                        }
                    } else if recursion_stack.contains(dep_id) {
                        return true;
                    }
                }
            }
            
            recursion_stack.remove(node_id);
            false
        }
        
        for node in &dag.nodes {
            if !visited.contains(&node.id) {
                if has_cycle(&node.id, dag, &mut visited, &mut recursion_stack) {
                    return Err(CisError::validation("DAG contains cycle"));
                }
            }
        }
        
        // 检查所有依赖是否存在
        for node in &dag.nodes {
            for dep_id in &node.dependencies {
                if !dag.nodes.iter().any(|n| n.id == *dep_id) {
                    return Err(CisError::validation(format!(
                        "Node {} depends on non-existent node {}",
                        node.id, dep_id
                    )));
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::mocks::MockSkillExecutor;
    
    #[tokio::test]
    async fn test_dag_execution() {
        let mock = MockSkillExecutor::new();
        mock.mock_execute(|skill, method, params, _| {
            Ok(ExecutionResult {
                success: true,
                data: format!("{}:{} executed", skill, method).into_bytes(),
                error: None,
            })
        });
        
        let executor = DagExecutor::new(Arc::new(mock));
        
        let dag = DagDefinition {
            id: "test-dag".to_string(),
            name: "Test DAG".to_string(),
            nodes: vec![
                DagNode {
                    id: "node-1".to_string(),
                    skill_name: "skill-a".to_string(),
                    method: "execute".to_string(),
                    params: vec![],
                    dependencies: vec![],
                },
                DagNode {
                    id: "node-2".to_string(),
                    skill_name: "skill-b".to_string(),
                    method: "execute".to_string(),
                    params: vec![],
                    dependencies: vec!["node-1".to_string()],
                },
            ],
        };
        
        let results = executor.execute(dag, ExecutionContext::default()).await.unwrap();
        
        assert_eq!(results.len(), 2);
        assert!(results.contains_key("node-1"));
        assert!(results.contains_key("node-2"));
    }
    
    #[test]
    fn test_cycle_detection() {
        let mock = MockSkillExecutor::new();
        let executor = DagExecutor::new(Arc::new(mock));
        
        let dag = DagDefinition {
            id: "cycle-dag".to_string(),
            name: "Cycle DAG".to_string(),
            nodes: vec![
                DagNode {
                    id: "a".to_string(),
                    skill_name: "skill".to_string(),
                    method: "execute".to_string(),
                    params: vec![],
                    dependencies: vec!["b".to_string()],
                },
                DagNode {
                    id: "b".to_string(),
                    skill_name: "skill".to_string(),
                    method: "execute".to_string(),
                    params: vec![],
                    dependencies: vec!["a".to_string()],
                },
            ],
        };
        
        let result = executor.validate_dag(&dag);
        assert!(result.is_err());
    }
}
