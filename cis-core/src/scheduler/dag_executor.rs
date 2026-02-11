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
    
    /// 执行 DAG
    pub async fn execute(&self, dag: DagDefinition, context: ExecutionContext) -> Result<HashMap<String, ExecutionResult>> {
        // 验证 DAG
        self.validate_dag(&dag)?;
        
        // 构建依赖图
        let mut status: HashMap<String, NodeStatus> = HashMap::new();
        for node in &dag.nodes {
            status.insert(node.id.clone(), NodeStatus::Pending);
        }
        
        let status = Arc::new(RwLock::new(status));
        let mut handles = vec![];
        
        // 执行节点（简化版：顺序执行）
        for node in dag.nodes {
            let executor = self.skill_executor.clone();
            let status = status.clone();
            let context = context.clone();
            
            let handle = tokio::spawn(async move {
                // 检查依赖是否完成
                let deps_completed = {
                    let status = status.read().await;
                    node.dependencies.iter().all(|dep_id| {
                        matches!(status.get(dep_id), Some(NodeStatus::Completed(_)))
                    })
                };
                
                if !deps_completed {
                    return Err(CisError::execution(format!(
                        "Dependencies not completed for node {}", node.id
                    )));
                }
                
                // 更新状态为运行中
                {
                    let mut status = status.write().await;
                    status.insert(node.id.clone(), NodeStatus::Running);
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
                        status.insert(node.id.clone(), NodeStatus::Completed(exec_result.clone()));
                        Ok(exec_result)
                    }
                    Err(e) => {
                        status.insert(node.id.clone(), NodeStatus::Failed(e.to_string()));
                        Err(e)
                    }
                }
            });
            
            handles.push((node.id, handle));
        }
        
        // 收集结果
        let mut results = HashMap::new();
        for (node_id, handle) in handles {
            match handle.await {
                Ok(Ok(result)) => {
                    results.insert(node_id, result);
                }
                Ok(Err(e)) => {
                    return Err(CisError::execution(format!(
                        "Node {} execution failed: {}", node_id, e
                    )));
                }
                Err(e) => {
                    return Err(CisError::execution(format!(
                        "Node {} task panicked: {}", node_id, e
                    )));
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
