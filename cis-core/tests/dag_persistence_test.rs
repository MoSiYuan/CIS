//! DAG 持久化集成测试
//!
//! 验证 DAG 运行状态可以保存到 SQLite 并在重启后恢复

use cis_core::scheduler::{DagPersistence, DagRun, DagScheduler, TaskDag, DagRunStatus};
use tempfile::NamedTempFile;

#[test]
fn test_dag_run_serialization() {
    // 创建简单 DAG
    let mut dag = TaskDag::new();
    dag.add_node("task1".to_string(), vec![]).unwrap();
    dag.add_node("task2".to_string(), vec!["task1".to_string()]).unwrap();
    dag.initialize();
    
    // 创建运行
    let run = DagRun::new(dag);
    let run_id = run.run_id.clone();
    
    // 序列化
    let json = run.to_json().expect("Failed to serialize");
    assert!(!json.is_empty());
    
    // 反序列化
    let restored = DagRun::from_json(&json).expect("Failed to deserialize");
    assert_eq!(restored.run_id, run_id);
    assert_eq!(restored.dag.node_count(), 2);
}

#[test]
fn test_persistence_save_and_load() {
    let temp_file = NamedTempFile::new().unwrap();
    let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();
    
    // 创建 DAG 运行
    let mut dag = TaskDag::new();
    dag.add_node("task1".to_string(), vec![]).unwrap();
    dag.initialize();
    
    let run = DagRun::new(dag);
    let run_id = run.run_id.clone();
    
    // 保存
    persistence.save_run_simple(&run).unwrap();
    
    // 加载
    let loaded = persistence.load_run(&run_id).unwrap().unwrap();
    assert_eq!(loaded.run_id, run_id);
    assert_eq!(loaded.dag.node_count(), 1);
    assert!(matches!(loaded.status, DagRunStatus::Running));
}

#[test]
fn test_persistence_list_runs() {
    let temp_file = NamedTempFile::new().unwrap();
    let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();
    
    // 创建多个运行
    for i in 0..3 {
        let mut dag = TaskDag::new();
        dag.add_node(format!("task{}", i), vec![]).unwrap();
        dag.initialize();
        
        let run = DagRun::new(dag);
        persistence.save_run_simple(&run).unwrap();
    }
    
    // 列出所有运行
    let runs = persistence.list_runs().unwrap();
    assert_eq!(runs.len(), 3);
}

#[test]
fn test_persistence_delete_run() {
    let temp_file = NamedTempFile::new().unwrap();
    let persistence = DagPersistence::new(temp_file.path().to_str().unwrap()).unwrap();
    
    let mut dag = TaskDag::new();
    dag.add_node("task1".to_string(), vec![]).unwrap();
    dag.initialize();
    
    let run = DagRun::new(dag);
    let run_id = run.run_id.clone();
    
    persistence.save_run_simple(&run).unwrap();
    assert!(persistence.load_run(&run_id).unwrap().is_some());
    
    persistence.delete_run(&run_id).unwrap();
    assert!(persistence.load_run(&run_id).unwrap().is_none());
}

#[test]
fn test_scheduler_with_persistence() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    // 创建第一个调度器并添加运行
    {
        let mut scheduler = DagScheduler::with_persistence(db_path).unwrap();
        
        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();
        
        let run_id = scheduler.create_run(dag);
        assert!(!run_id.is_empty());
        assert_eq!(scheduler.run_count(), 1);
    }
    
    // 创建新的调度器实例，应该能恢复之前的运行
    {
        let scheduler = DagScheduler::with_persistence(db_path).unwrap();
        assert_eq!(scheduler.run_count(), 1);
        
        // 验证可以获取恢复的运行
        let run_id = scheduler.run_ids().next().unwrap().clone();
        let run = scheduler.get_run(&run_id).unwrap();
        assert_eq!(run.dag.node_count(), 1);
    }
}

#[test]
fn test_dag_run_update_and_persist() {
    let temp_file = NamedTempFile::new().unwrap();
    let db_path = temp_file.path().to_str().unwrap();
    
    let mut scheduler = DagScheduler::with_persistence(db_path).unwrap();
    
    // 创建运行
    let mut dag = TaskDag::new();
    dag.add_node("task1".to_string(), vec![]).unwrap();
    dag.initialize();
    
    let run_id = scheduler.create_run(dag);
    
    // 修改运行状态
    {
        let run = scheduler.get_run_mut(&run_id).unwrap();
        run.status = DagRunStatus::Completed;
    }
    
    // 更新并持久化
    let run = scheduler.get_run(&run_id).unwrap().clone();
    scheduler.update_run(run).unwrap();
    
    // 创建新调度器验证持久化
    let scheduler2 = DagScheduler::with_persistence(db_path).unwrap();
    let run = scheduler2.get_run(&run_id).unwrap();
    assert!(matches!(run.status, DagRunStatus::Completed));
}
