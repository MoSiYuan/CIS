//! Skill Compatibility 数据库演示
//!
//! 演示 CVI-010 的数据库功能

use cis_core::skill::{SkillCompatibilityDb, SkillCompatibilityRecord};
use rusqlite::Connection;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Skill Compatibility 数据库演示 ===\n");

    // 创建内存数据库
    let conn = Connection::open_in_memory()?;
    let db = SkillCompatibilityDb::new(&conn);

    // 初始化表
    println!("1. 初始化数据库表...");
    db.init_table()?;
    println!("   ✓ 表创建成功");

    // 插入兼容性记录
    println!("\n2. 插入兼容性记录...");
    
    let records = vec![
        SkillCompatibilityRecord {
            source_skill_id: "data-analyzer".to_string(),
            target_skill_id: "report-gen".to_string(),
            compatibility_score: 0.92,
            data_flow_types: "[\"analysis_result\"]".to_string(),
            discovered_at: chrono::Utc::now().timestamp(),
        },
        SkillCompatibilityRecord {
            source_skill_id: "data-analyzer".to_string(),
            target_skill_id: "visualizer".to_string(),
            compatibility_score: 0.78,
            data_flow_types: "[\"json\", \"csv\"]".to_string(),
            discovered_at: chrono::Utc::now().timestamp(),
        },
        SkillCompatibilityRecord {
            source_skill_id: "file-reader".to_string(),
            target_skill_id: "data-analyzer".to_string(),
            compatibility_score: 0.85,
            data_flow_types: "[\"content\", \"text\"]".to_string(),
            discovered_at: chrono::Utc::now().timestamp(),
        },
    ];

    for record in &records {
        db.upsert(record)?;
        println!(
            "   ✓ {} -> {} (score: {:.2})",
            record.source_skill_id,
            record.target_skill_id,
            record.compatibility_score
        );
    }

    // 查询特定兼容性
    println!("\n3. 查询兼容性...");
    let compat = db.get("data-analyzer", "report-gen")?;
    if let Some(c) = compat {
        println!(
            "   ✓ data-analyzer -> report-gen: {:.2}",
            c.compatibility_score
        );
    }

    // 查找所有兼容目标
    println!("\n4. 查找 data-analyzer 的兼容目标...");
    let targets = db.find_compatible_targets("data-analyzer", 0.7)?;
    for target in &targets {
        println!(
            "   ✓ {} (score: {:.2}, types: {})",
            target.target_skill_id,
            target.compatibility_score,
            target.data_flow_types
        );
    }

    // 查找所有兼容源
    println!("\n5. 查找 data-analyzer 的兼容源...");
    let sources = db.find_compatible_sources("data-analyzer", 0.7)?;
    for source in &sources {
        println!(
            "   ✓ {} (score: {:.2})",
            source.source_skill_id,
            source.compatibility_score
        );
    }

    // 统计记录数
    println!("\n6. 统计...");
    let count = db.count()?;
    println!("   ✓ 总记录数: {}", count);

    // 列出所有记录
    println!("\n7. 所有兼容性记录 (Top 10)...");
    let all = db.list_all(10)?;
    for (i, record) in all.iter().enumerate() {
        println!(
            "   {}. {} -> {}: {:.2}",
            i + 1,
            record.source_skill_id,
            record.target_skill_id,
            record.compatibility_score
        );
    }

    println!("\n=== 演示完成 ===");
    Ok(())
}
