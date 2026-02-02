//! Telemetry commands for CIS CLI
//!
//! Provides CLI interface for request logging and system observability.

use chrono::{Duration, Utc};
use crate::TelemetryAction;
use std::path::PathBuf;

/// 获取默认遥测数据库路径
fn default_telemetry_path() -> PathBuf {
    let data_dir = cis_core::storage::paths::Paths::data_dir();
    data_dir.join("telemetry.db")
}

pub fn handle_telemetry(action: TelemetryAction) -> anyhow::Result<()> {
    use cis_core::telemetry::{LogQuery, RequestLogger};
    
    let path = default_telemetry_path();
    let logger = RequestLogger::open(&path, None)
        .map_err(|e| anyhow::anyhow!("Failed to open telemetry database: {}", e))?;
    
    match action {
        TelemetryAction::Logs { limit, success_only, hours, session, verbose } => {
            let mut query = LogQuery::new()
                .with_limit(limit);
            
            if success_only {
                query = query.success_only();
            }
            
            if let Some(session_id) = session {
                query = query.with_session(session_id);
            }
            
            if let Some(h) = hours {
                query = query.with_time_range(
                    Utc::now() - Duration::hours(h),
                    Utc::now()
                );
            }
            
            let logs = logger.query_logs(&query)
                .map_err(|e| anyhow::anyhow!("Failed to query logs: {}", e))?;
            
            if logs.is_empty() {
                println!("📊 没有找到请求日志");
                return Ok(());
            }
            
            println!("📊 最近 {} 条请求日志\n", logs.len());
            
            for (i, log) in logs.iter().enumerate() {
                let (status_icon, status_text) = match &log.result {
                    cis_core::telemetry::RequestResult::Success { .. } => ("✅", "成功"),
                    cis_core::telemetry::RequestResult::NoMatch { .. } => ("⚠️", "无匹配"),
                    cis_core::telemetry::RequestResult::Error { .. } => ("❌", "错误"),
                    cis_core::telemetry::RequestResult::Cancelled => ("🚫", "取消"),
                };
                
                let input_preview: String = log.user_input.chars().take(50).collect();
                let input_display = if log.user_input.len() > 50 {
                    format!("{}...", input_preview)
                } else {
                    input_preview
                };
                
                println!("{}. {} {} - {} ({}ms)",
                    i + 1,
                    status_icon,
                    status_text,
                    input_display,
                    log.metrics.total_duration_ms
                );
                
                // 显示详细信息
                if verbose {
                    println!("   ID:     {}", log.id);
                    println!("   会话:   {}", log.session_id);
                    if let Some(ref conv_id) = log.conversation_id {
                        println!("   对话:   {}", conv_id);
                    }
                    println!("   时间:   {}", log.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
                    
                    if let cis_core::telemetry::RequestResult::Success { skill_id, output_summary } = &log.result {
                        println!("   技能:   {}", skill_id);
                        println!("   结果:   {}", output_summary);
                    }
                    if let cis_core::telemetry::RequestResult::Error { error } = &log.result {
                        println!("   错误:   {}", error);
                    }
                    if let cis_core::telemetry::RequestResult::NoMatch { reason } = &log.result {
                        println!("   原因:   {}", reason);
                    }
                }
                
                // 显示处理阶段
                if !log.stages.is_empty() {
                    for stage in &log.stages {
                        let icon = if stage.success { "✓" } else { "✗" };
                        println!("   {} {}: {}ms", icon, stage.name, stage.duration_ms);
                        
                        if verbose {
                            if let Some(ref output) = stage.output {
                                println!("      输出: {}", output.chars().take(100).collect::<String>());
                            }
                            if let Some(ref error) = stage.error {
                                println!("      错误: {}", error);
                            }
                        }
                    }
                }
                
                println!();
            }
        }
        
        TelemetryAction::Stats { session } => {
            let stats = if let Some(session_id) = session {
                println!("📈 会话统计: {}\n", session_id);
                logger.get_session_stats(&session_id)
                    .map_err(|e| anyhow::anyhow!("Failed to get session stats: {}", e))?
            } else {
                println!("📈 全局统计\n");
                logger.get_global_stats()
                    .map_err(|e| anyhow::anyhow!("Failed to get global stats: {}", e))?
            };
            
            println!("总请求数:      {}", stats.total_requests);
            println!("成功:          {} ({:.1}%)", 
                stats.successful_requests,
                if stats.total_requests > 0 {
                    (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0
                } else { 0.0 }
            );
            println!("失败:          {}", stats.failed_requests);
            println!("平均耗时:      {}ms", stats.average_duration_ms);
        }
        
        TelemetryAction::Sessions { limit } => {
            let sessions = logger.get_sessions(limit)
                .map_err(|e| anyhow::anyhow!("Failed to get sessions: {}", e))?;
            
            if sessions.is_empty() {
                println!("📊 没有找到会话");
                return Ok(());
            }
            
            println!("📊 最近 {} 个会话\n", sessions.len());
            for (i, (session_id, count)) in sessions.iter().enumerate() {
                println!("{}. {} ({} 请求)", i + 1, session_id, count);
            }
        }
        
        TelemetryAction::Cleanup { days } => {
            let count = logger.cleanup_old_logs(days)
                .map_err(|e| anyhow::anyhow!("Failed to cleanup logs: {}", e))?;
            println!("🧹 清理了 {} 条旧日志（{}天前）", count, days);
        }
    }
    
    Ok(())
}
