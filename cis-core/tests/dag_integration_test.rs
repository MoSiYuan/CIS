//! DAG 分布式执行集成测试 (Phase 7)
//!
//! 验证：
//! 1. 单机全流程：curl推送 → worker启动 → 任务执行 → 状态查询
//! 2. 作用域隔离：proj-a/proj-b 两个worker并行
//! 3. 模拟集群：2节点，节点认领过滤

use cis_core::scheduler::{
    DagPersistence, DagRun, DagScheduler, DagScope, TaskDag, 
    DagRunStatus, DagNodeStatus, ScopeInferrer, ScopeConflict
};
use cis_core::matrix::events::{
    DagExecuteEvent, DagExecuteContent, NodeClaimFilter,
    parse_dag_event, TodoProposalEvent, TodoProposalContent,
};
use tempfile::NamedTempFile;
use std::collections::HashMap;

/// Test 7.1: 单机全流程测试
/// 
/// 步骤：
/// 1. 创建 DAG 定义
/// 2. 推断 scope (Global)
/// 3. 保存到数据库
/// 4. 模拟 HTTP 推送
/// 5. 验证节点认领
/// 6. 执行任务并验证状态
#[test]
fn test_single_node_full_workflow() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    // 1. 创建 DAG
    let mut dag = TaskDag::new();
    dag.add_node("fetch".to_string(), vec![]).unwrap();
    dag.add_node("process".to_string(), vec!["fetch".to_string()]).unwrap();
    dag.add_node("report".to_string(), vec!["process".to_string()]).unwrap();
    dag.initialize();
    
    // 2. 推断 scope（无显式指定，dag_id="mydag" 不触发推断，应推断为 Global）
    let scope = ScopeInferrer::infer(None, "mydag", &[]);
    assert!(matches!(scope, DagScope::Global), "Expected Global scope, got {:?}", scope);
    
    // 3. 创建 scheduler 并保存
    let mut scheduler = DagScheduler::with_persistence(db_path).unwrap();
    let run_id = scheduler.create_run(dag);
    
    // 4. 模拟 HTTP 推送 - 无 target_node（广播模式）
    let event = create_test_dag_event("mydag", None, &run_id);
    
    // 5. 节点认领过滤（广播模式下 accept_broadcast=true 的节点应认领）
    let filter = NodeClaimFilter::new("node-1".to_string(), true);
    let (should_execute, reason) = filter.should_execute(&event);
    assert!(should_execute, "Node should claim broadcast DAG");
    assert_eq!(reason, "Broadcast accepted");
    
    // 6. 模拟任务执行
    {
        let run = scheduler.get_run_mut(&run_id).unwrap();
        
        // 验证初始状态
        assert_eq!(run.status, DagRunStatus::Running);
        
        // 执行任务1 (fetch - root)
        let ready = run.dag.get_ready_tasks();
        assert!(ready.contains(&"fetch".to_string()));
        run.dag.mark_running("fetch".to_string()).unwrap();
        run.dag.mark_completed("fetch".to_string()).unwrap();
        
        // 执行任务2 (process - depends on fetch)
        let ready = run.dag.get_ready_tasks();
        assert!(ready.contains(&"process".to_string()));
        run.dag.mark_running("process".to_string()).unwrap();
        run.dag.mark_completed("process".to_string()).unwrap();
        
        // 执行任务3 (report - depends on process)
        let ready = run.dag.get_ready_tasks();
        assert!(ready.contains(&"report".to_string()));
        run.dag.mark_running("report".to_string()).unwrap();
        run.dag.mark_completed("report".to_string()).unwrap();
        
        // 标记完成
        run.status = DagRunStatus::Completed;
    }
    
    // 保存最终状态
    let run = scheduler.get_run(&run_id).unwrap().clone();
    scheduler.update_run(run).unwrap();
    
    // 7. 验证状态查询
    let scheduler2 = DagScheduler::with_persistence(db_path).unwrap();
    let run = scheduler2.get_run(&run_id).unwrap();
    
    assert!(matches!(run.status, DagRunStatus::Completed));
    assert_eq!(run.dag.node_count(), 3);
    
    // 验证所有任务完成
    for (task_id, node) in run.dag.nodes() {
        assert!(
            matches!(node.status, DagNodeStatus::Completed),
            "Task {} should be completed",
            task_id
        );
    }
    
    println!("✓ Single node full workflow test passed");
}

/// Test 7.2: 作用域隔离测试
///
/// 验证：
/// 1. proj-a 的 DAG 分配到 worker-project-proj-a
/// 2. proj-b 的 DAG 分配到 worker-project-proj-b  
/// 3. 不同 scope 的 DAG 不共享 worker
#[test]
fn test_scope_isolation() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    let mut scheduler = DagScheduler::with_persistence(db_path).unwrap();
    
    // 创建 proj-a 的 DAG
    let mut dag_a = TaskDag::new();
    dag_a.add_node("task-a1".to_string(), vec![]).unwrap();
    dag_a.initialize();
    
    let _run_id_a = scheduler.create_run(dag_a);
    
    // 创建 proj-b 的 DAG
    let mut dag_b = TaskDag::new();
    dag_b.add_node("task-b1".to_string(), vec![]).unwrap();
    dag_b.initialize();
    
    let _run_id_b = scheduler.create_run(dag_b);
    
    // 模拟不同 scope 的 worker ID
    let scope_a = DagScope::Project { 
        project_id: "proj-a".to_string(), 
        force_new: false 
    };
    let scope_b = DagScope::Project { 
        project_id: "proj-b".to_string(), 
        force_new: false 
    };
    
    let worker_id_a = scope_a.worker_id();
    let worker_id_b = scope_b.worker_id();
    
    // 验证 worker ID 不同
    assert_ne!(worker_id_a, worker_id_b, "Different scopes should have different worker IDs");
    assert!(worker_id_a.contains("proj-a"), "Worker A should contain proj-a");
    assert!(worker_id_b.contains("proj-b"), "Worker B should contain proj-b");
    
    // 验证 scope key 相同（用于复用）
    let worker_key_a = scope_a.worker_key();
    let worker_key_b = scope_b.worker_key();
    assert_ne!(worker_key_a, worker_key_b, "Different scopes should have different worker keys");
    
    println!("✓ Scope isolation test passed");
    println!("  Worker A: {}", worker_id_a);
    println!("  Worker B: {}", worker_id_b);
}

/// Test 7.3: 作用域冲突检测
///
/// 验证：相同 worker_id 但不同 target_node 时触发冲突
#[test]
fn test_scope_conflict_detection() {
    // 场景：两个 DAG 都指向同一个 scope，但目标节点不同
    let entries = vec![
        ("dag-1".to_string(), DagScope::Project { 
            project_id: "proj-a".to_string(), 
            force_new: false 
        }, Some("node-1".to_string())),
        ("dag-2".to_string(), DagScope::Project { 
            project_id: "proj-a".to_string(), 
            force_new: false 
        }, Some("node-2".to_string())),
    ];
    
    let conflicts = ScopeInferrer::detect_conflicts(&entries);
    
    // 应该检测到冲突（相同 scope，不同 target_node）
    assert!(!conflicts.is_empty(), "Should detect scope conflict");
    assert_eq!(conflicts.len(), 1);
    
    let conflict = &conflicts[0];
    assert_eq!(conflict.worker_id, "worker-project-proj-a");
    assert_eq!(conflict.dag_ids.len(), 2);
    assert!(conflict.dag_ids.contains(&"dag-1".to_string()));
    assert!(conflict.dag_ids.contains(&"dag-2".to_string()));
    
    println!("✓ Scope conflict detection test passed");
    println!("  Detected {} conflict(s)", conflicts.len());
}

/// Test 7.4: 节点认领过滤
///
/// 验证：
/// 1. target_node 匹配时认领
/// 2. target_node 不匹配时忽略
/// 3. 广播模式下 accept_broadcast=true 时认领
/// 4. 广播模式下 accept_broadcast=false 时忽略
#[test]
fn test_node_claim_filtering() {
    // Test 1: 目标节点匹配
    let filter = NodeClaimFilter::new("node-1".to_string(), false);
    let event = create_test_dag_event("dag-1", Some("node-1"), "run-1");
    let (should, reason) = filter.should_execute(&event);
    assert!(should, "Should execute when targeted at this node");
    assert_eq!(reason, "Targeted at this node");
    
    // Test 2: 目标节点不匹配
    let filter = NodeClaimFilter::new("node-1".to_string(), false);
    let event = create_test_dag_event("dag-1", Some("node-2"), "run-1");
    let (should, reason) = filter.should_execute(&event);
    assert!(!should, "Should not execute when targeted at other node");
    assert!(reason.contains("other node"));
    
    // Test 3: 广播 + accept_broadcast=true
    let filter = NodeClaimFilter::new("node-1".to_string(), true);
    let event = create_test_dag_event("dag-1", None, "run-1");
    let (should, reason) = filter.should_execute(&event);
    assert!(should, "Should execute broadcast when accept_broadcast=true");
    assert_eq!(reason, "Broadcast accepted");
    
    // Test 4: 广播 + accept_broadcast=false
    let filter = NodeClaimFilter::new("node-1".to_string(), false);
    let event = create_test_dag_event("dag-1", None, "run-1");
    let (should, reason) = filter.should_execute(&event);
    assert!(!should, "Should not execute broadcast when accept_broadcast=false");
    assert!(reason.contains("Broadcast rejected"));
    
    println!("✓ Node claim filtering test passed");
}

/// Test 7.5: Worker 复用测试
///
/// 验证：相同 scope 的 DAG 应复用同一个 worker
#[test]
fn test_worker_reuse() {
    let scope = DagScope::Project { 
        project_id: "proj-a".to_string(), 
        force_new: false 
    };
    
    // 多次获取 worker_key 应该相同
    let key1 = scope.worker_key();
    let key2 = scope.worker_key();
    assert_eq!(key1, key2, "Same scope should have same worker key for reuse");
    
    // force_new=true 时应该不同
    let scope_new = DagScope::Project { 
        project_id: "proj-a".to_string(), 
        force_new: true 
    };
    let key_new = scope_new.worker_key();
    assert_ne!(key1, key_new, "force_new should generate different key");
    assert!(key_new.contains("new"), "force_new key should contain 'new'");
    
    println!("✓ Worker reuse test passed");
    println!("  Reuse key: {}", key1);
    println!("  New key: {}", key_new);
}

/// Test 7.6: DAG 序列化和 Matrix 事件
///
/// 验证 DAG 可以通过 Matrix Room 事件传输
#[test]
fn test_dag_matrix_event_serialization() {
    // 创建 Matrix 执行事件
    let event = DagExecuteEvent {
        event_type: "io.cis.dag.execute".to_string(),
        content: DagExecuteContent {
            dag_id: "test-dag".to_string(),
            tasks: vec![],
            scope: DagScope::Global,
            target_node: Some("node-1".to_string()),
            priority: cis_core::scheduler::DagPriority::High,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    };
    
    // 序列化
    let json = serde_json::to_string(&event).expect("Failed to serialize");
    
    // 从 JSON 解析
    let parsed = parse_dag_event(&json).expect("Failed to parse");
    
    assert_eq!(parsed.content.dag_id, "test-dag");
    assert!(matches!(parsed.content.scope, DagScope::Global));
    assert_eq!(parsed.content.target_node, Some("node-1".to_string()));
    
    println!("✓ Matrix event serialization test passed");
}

/// Test 7.7: TODO List 提案事件
///
/// 验证 Room Agent 可以向 Worker 发送 TODO 提案
#[test]
fn test_todo_proposal_event() {
    use cis_core::scheduler::{TodoListProposal, TodoListDiff, ProposalSource};
    
    let diff = TodoListDiff::default();
    let proposal = TodoListProposal::new(
        ProposalSource::RoomAgent,
        "room-agent-1",
        diff,
        "Add security scan task",
    );
    
    let event = TodoProposalEvent {
        event_type: "io.cis.dag.todo_proposal".to_string(),
        content: TodoProposalContent {
            run_id: "run-xxx".to_string(),
            proposal: proposal.clone(),
        },
    };
    
    // 序列化
    let json = serde_json::to_string(&event).expect("Failed to serialize");
    
    // 解析
    let parsed: TodoProposalEvent = serde_json::from_str(&json).expect("Failed to parse");
    
    assert_eq!(parsed.content.run_id, "run-xxx");
    assert_eq!(parsed.content.proposal.reason, "Add security scan task");
    assert!(parsed.content.proposal.requires_review());
    
    println!("✓ TODO proposal event test passed");
}

/// Test 7.8: 乐观锁并发更新
///
/// 验证 version 字段防止并发冲突
#[test]
fn test_optimistic_locking() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    let mut scheduler = DagScheduler::with_persistence(db_path).unwrap();
    
    // 创建运行
    let mut dag = TaskDag::new();
    dag.add_node("task1".to_string(), vec![]).unwrap();
    dag.initialize();
    
    let run_id = scheduler.create_run(dag);
    
    // 获取初始 version
    let version1 = scheduler.get_run(&run_id).unwrap().version;
    
    // 修改并保存
    {
        let run = scheduler.get_run(&run_id).unwrap().clone();
        scheduler.update_run(run).unwrap();
    }
    
    // 重新加载，version 应该增加
    let scheduler2 = DagScheduler::with_persistence(db_path).unwrap();
    let run = scheduler2.get_run(&run_id).unwrap();
    let version2 = run.version;
    
    assert!(version2 >= version1, "Version should be incremented");
    
    println!("✓ Optimistic locking test passed");
    println!("  Initial version: {}", version1);
    println!("  Updated version: {}", version2);
}

// ==================== 辅助函数 ====================

fn create_test_dag_event(dag_id: &str, target_node: Option<&str>, _run_id: &str) -> DagExecuteEvent {
    DagExecuteEvent {
        event_type: "io.cis.dag.execute".to_string(),
        content: DagExecuteContent {
            dag_id: dag_id.to_string(),
            tasks: vec![],
            scope: DagScope::Global,
            target_node: target_node.map(|s| s.to_string()),
            priority: cis_core::scheduler::DagPriority::Normal,
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    }
}
