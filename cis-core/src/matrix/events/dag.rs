//! # DAG Matrix 事件处理
//!
//! 处理 Matrix Room 中的 DAG 执行事件，实现节点认领过滤。

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::scheduler::{DagScope, DagPriority, DagTodoList, DagTaskSpec, TodoListProposal, ProposalResult};

/// DAG 执行事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecuteEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: DagExecuteContent,
}

/// DAG 执行事件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagExecuteContent {
    pub dag_id: String,
    pub tasks: Vec<DagTaskSpec>,
    pub scope: DagScope,
    #[serde(default)]
    pub target_node: Option<String>,
    #[serde(default)]
    pub priority: DagPriority,
    pub timestamp: String,
}

/// 节点认领过滤器
pub struct NodeClaimFilter {
    node_id: String,
    accept_broadcast: bool,
}

impl NodeClaimFilter {
    pub fn new(node_id: String, accept_broadcast: bool) -> Self {
        Self {
            node_id,
            accept_broadcast,
        }
    }

    /// 检查是否应该认领此 DAG
    pub fn should_execute(&self, event: &DagExecuteEvent) -> (bool, String) {
        let content = &event.content;

        match &content.target_node {
            Some(target) => {
                if target == &self.node_id {
                    info!("DAG {} targeted at this node, claiming", content.dag_id);
                    (true, "Targeted at this node".to_string())
                } else {
                    debug!("DAG {} targeted at other node, ignoring", content.dag_id);
                    (false, format!("Targeted at other node: {}", target))
                }
            }
            None => {
                if self.accept_broadcast {
                    info!("DAG {} broadcast received, claiming", content.dag_id);
                    (true, "Broadcast accepted".to_string())
                } else {
                    debug!("DAG {} broadcast received, ignoring", content.dag_id);
                    (false, "Broadcast rejected".to_string())
                }
            }
        }
    }
}

/// DAG 状态报告事件（Worker → Room）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagStatusEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: DagStatusContent,
}

/// DAG 状态报告内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagStatusContent {
    pub dag_id: String,
    pub run_id: String,
    pub worker_id: String,
    pub node_id: String,
    pub status: String,
    #[serde(default)]
    pub todo_list: Option<DagTodoList>,
    pub timestamp: String,
}

/// TODO List 提案事件（Room Agent → Worker）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoProposalEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: TodoProposalContent,
}

/// TODO List 提案事件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoProposalContent {
    pub run_id: String,
    pub proposal: TodoListProposal,
}

/// TODO List 提案回复事件（Worker → Room Agent）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoProposalResponseEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub content: TodoProposalResponseContent,
}

/// TODO List 提案回复内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoProposalResponseContent {
    pub run_id: String,
    pub result: ProposalResult,
    pub worker_id: String,
    pub timestamp: String,
}

/// 从消息内容解析 DAG 事件
pub fn parse_dag_event(content: &str) -> Option<DagExecuteEvent> {
    if let Ok(event) = serde_json::from_str::<DagExecuteEvent>(content) {
        if event.event_type == "io.cis.dag.execute" {
            return Some(event);
        }
    }
    None
}

/// 从消息内容解析 TODO 提案事件
pub fn parse_todo_proposal_event(content: &str) -> Option<TodoProposalEvent> {
    if let Ok(event) = serde_json::from_str::<TodoProposalEvent>(content) {
        if event.event_type == "io.cis.dag.todo_proposal" {
            return Some(event);
        }
    }
    None
}

/// 从消息内容解析 TODO 提案回复事件
pub fn parse_todo_proposal_response(content: &str) -> Option<TodoProposalResponseEvent> {
    if let Ok(event) = serde_json::from_str::<TodoProposalResponseEvent>(content) {
        if event.event_type == "io.cis.dag.todo_response" {
            return Some(event);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event(target_node: Option<String>) -> DagExecuteEvent {
        DagExecuteEvent {
            event_type: "io.cis.dag.execute".to_string(),
            content: DagExecuteContent {
                dag_id: "test-dag".to_string(),
                tasks: vec![],
                scope: DagScope::Global,
                target_node,
                priority: DagPriority::Normal,
                timestamp: chrono::Utc::now().to_rfc3339(),
            },
        }
    }

    #[test]
    fn test_targeted_match() {
        let filter = NodeClaimFilter::new("node-1".to_string(), false);
        let event = create_test_event(Some("node-1".to_string()));
        let (should, _) = filter.should_execute(&event);
        assert!(should);
    }

    #[test]
    fn test_targeted_mismatch() {
        let filter = NodeClaimFilter::new("node-1".to_string(), false);
        let event = create_test_event(Some("node-2".to_string()));
        let (should, _) = filter.should_execute(&event);
        assert!(!should);
    }

    #[test]
    fn test_broadcast_accepted() {
        let filter = NodeClaimFilter::new("node-1".to_string(), true);
        let event = create_test_event(None);
        let (should, _) = filter.should_execute(&event);
        assert!(should);
    }

    #[test]
    fn test_broadcast_rejected() {
        let filter = NodeClaimFilter::new("node-1".to_string(), false);
        let event = create_test_event(None);
        let (should, _) = filter.should_execute(&event);
        assert!(!should);
    }
}
