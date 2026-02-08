use super::TelemetryConfig;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

/// 请求日志记录器
/// 
/// 与私域记忆分离存储，专门用于系统可观测性
/// 
/// # 示例
/// 
/// ```rust,no_run
/// use cis_core::telemetry::{RequestLogger, RequestLogBuilder, RequestResult};
/// use std::path::Path;
/// 
/// let logger = RequestLogger::open(Path::new("telemetry.db"), None).unwrap();
/// 
/// let log = RequestLogBuilder::new("session-1", "帮我查找文档")
///     .with_conversation_id("conv-1")
///     .set_result(RequestResult::Success {
///         skill_id: "file_search".to_string(),
///         output_summary: "找到3个文档".to_string(),
///     })
///     .build();
/// 
/// logger.log_request(&log).unwrap();
/// ```
pub struct RequestLogger {
    conn: Connection,
    config: TelemetryConfig,
}

/// 完整的请求日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLog {
    /// 唯一ID
    pub id: String,
    /// 会话ID
    pub session_id: String,
    /// 对话ID
    pub conversation_id: Option<String>,
    /// 用户输入
    pub user_input: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 处理阶段
    pub stages: Vec<RequestStage>,
    /// 最终结果
    pub result: RequestResult,
    /// 性能指标
    pub metrics: RequestMetrics,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 请求处理阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestStage {
    /// 阶段名称
    pub name: String,
    /// 开始时间
    pub start_time: DateTime<Utc>,
    /// 耗时
    pub duration_ms: u64,
    /// 输入
    pub input: Option<String>,
    /// 输出
    pub output: Option<String>,
    /// 是否成功
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
}

/// 请求结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RequestResult {
    #[serde(rename = "success")]
    Success { skill_id: String, output_summary: String },
    #[serde(rename = "no_match")]
    NoMatch { reason: String },
    #[serde(rename = "error")]
    Error { error: String },
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// 性能指标
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct RequestMetrics {
    /// 总耗时
    pub total_duration_ms: u64,
    /// 意图解析耗时
    pub intent_duration_ms: u64,
    /// 技能路由耗时
    pub routing_duration_ms: u64,
    /// 技能执行耗时
    pub execution_duration_ms: u64,
}


/// 日志查询条件
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    pub session_id: Option<String>,
    pub conversation_id: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub skill_id: Option<String>,
    pub success_only: bool,
    pub limit: usize,
}

impl LogQuery {
    /// 创建新的查询
    pub fn new() -> Self {
        Self {
            limit: 100,
            ..Default::default()
        }
    }
    
    /// 按会话筛选
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
    
    /// 按对话筛选
    pub fn with_conversation(mut self, conversation_id: impl Into<String>) -> Self {
        self.conversation_id = Some(conversation_id.into());
        self
    }
    
    /// 按时间范围筛选
    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }
    
    /// 只显示成功请求
    pub fn success_only(mut self) -> Self {
        self.success_only = true;
        self
    }
    
    /// 设置限制数量
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// 会话统计
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_duration_ms: u64,
}

impl RequestLogger {
    /// 获取配置
    pub fn config(&self) -> &TelemetryConfig {
        &self.config
    }
    
    /// 打开或创建日志数据库
    /// 
    /// # 参数
    /// - `path`: 数据库文件路径
    /// - `config`: 可选的配置
    /// 
    /// # 返回
    /// - `Result<Self>`: 成功返回RequestLogger，失败返回错误
    pub fn open(path: &Path, config: Option<TelemetryConfig>) -> crate::error::Result<Self> {
        let conn = Connection::open(path)?;
        let config = config.unwrap_or_default();
        
        let logger = Self { conn, config };
        logger.init_tables()?;
        Ok(logger)
    }
    
    fn init_tables(&self) -> crate::error::Result<()> {
        // 主日志表
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS request_logs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                conversation_id TEXT,
                user_input TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                result_type TEXT NOT NULL,
                result_summary TEXT,
                error_message TEXT,
                total_duration_ms INTEGER,
                intent_duration_ms INTEGER,
                routing_duration_ms INTEGER,
                execution_duration_ms INTEGER,
                metadata TEXT
            )",
            [],
        )?;
        
        // 处理阶段表
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS request_stages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                request_id TEXT NOT NULL,
                stage_name TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                duration_ms INTEGER,
                input TEXT,
                output TEXT,
                success BOOLEAN,
                error TEXT,
                FOREIGN KEY (request_id) REFERENCES request_logs(id)
            )",
            [],
        )?;
        
        // 索引
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_session ON request_logs(session_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_conversation ON request_logs(conversation_id)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_time ON request_logs(timestamp)",
            [],
        )?;
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_stages_request ON request_stages(request_id)",
            [],
        )?;
        
        Ok(())
    }
    
    /// 记录完整请求
    /// 
    /// # 参数
    /// - `log`: 要记录的请求日志
    pub fn log_request(&self, log: &RequestLog) -> crate::error::Result<()> {
        // 采样检查
        if self.config.sample_rate < 1.0 {
            let should_sample: f32 = rand::random();
            if should_sample > self.config.sample_rate {
                return Ok(());
            }
        }
        
        let metadata_json = serde_json::to_string(&log.metadata).unwrap_or_default();
        
        // 提取结果信息
        let (result_type, result_summary, error_msg) = match &log.result {
            RequestResult::Success { skill_id, output_summary } => {
                ("success", Some(skill_id.as_str()), Some(output_summary.as_str()))
            }
            RequestResult::NoMatch { reason } => {
                ("no_match", None, Some(reason.as_str()))
            }
            RequestResult::Error { error } => {
                ("error", None, Some(error.as_str()))
            }
            RequestResult::Cancelled => {
                ("cancelled", None, None)
            }
        };
        
        self.conn.execute(
            "INSERT INTO request_logs (
                id, session_id, conversation_id, user_input, timestamp,
                result_type, result_summary, error_message,
                total_duration_ms, intent_duration_ms, routing_duration_ms, execution_duration_ms,
                metadata
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT(id) DO UPDATE SET
                result_type = excluded.result_type,
                result_summary = excluded.result_summary,
                error_message = excluded.error_message,
                total_duration_ms = excluded.total_duration_ms",
            rusqlite::params![
                log.id,
                log.session_id,
                log.conversation_id,
                log.user_input,
                log.timestamp.timestamp(),
                result_type,
                result_summary,
                error_msg,
                log.metrics.total_duration_ms as i64,
                log.metrics.intent_duration_ms as i64,
                log.metrics.routing_duration_ms as i64,
                log.metrics.execution_duration_ms as i64,
                metadata_json,
            ],
        )?;
        
        // 插入阶段
        for stage in &log.stages {
            self.conn.execute(
                "INSERT INTO request_stages (
                    request_id, stage_name, start_time, duration_ms,
                    input, output, success, error
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                ON CONFLICT DO NOTHING",
                rusqlite::params![
                    log.id,
                    stage.name,
                    stage.start_time.timestamp(),
                    stage.duration_ms as i64,
                    stage.input,
                    stage.output,
                    stage.success,
                    stage.error,
                ],
            )?;
        }
        
        Ok(())
    }
    
    /// 查询日志
    /// 
    /// # 参数
    /// - `query`: 查询条件
    /// 
    /// # 返回
    /// - `Result<Vec<RequestLog>>`: 匹配的日志列表
    pub fn query_logs(&self, query: &LogQuery) -> crate::error::Result<Vec<RequestLog>> {
        let mut sql = String::from(
            "SELECT 
                id, session_id, conversation_id, user_input, timestamp,
                result_type, result_summary, error_message,
                total_duration_ms, intent_duration_ms, routing_duration_ms, execution_duration_ms,
                metadata
             FROM request_logs WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        
        if let Some(session_id) = &query.session_id {
            sql.push_str(" AND session_id = ?");
            params.push(Box::new(session_id.clone()));
        }
        
        if let Some(conv_id) = &query.conversation_id {
            sql.push_str(" AND conversation_id = ?");
            params.push(Box::new(conv_id.clone()));
        }
        
        if let Some(start) = query.start_time {
            sql.push_str(" AND timestamp >= ?");
            params.push(Box::new(start.timestamp()));
        }
        
        if let Some(end) = query.end_time {
            sql.push_str(" AND timestamp <= ?");
            params.push(Box::new(end.timestamp()));
        }
        
        if query.success_only {
            sql.push_str(" AND result_type = 'success'");
        }
        
        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");
        params.push(Box::new(query.limit as i64));
        
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Self::row_to_request_log(row)
        })?;
        
        let mut logs = Vec::new();
        for row in rows {
            let mut log = row.map_err(crate::error::CisError::Database)?;
            // 加载阶段
            log.stages = self.load_stages(&log.id)?;
            logs.push(log);
        }
        
        Ok(logs)
    }
    
    /// 获取单条日志详情
    pub fn get_log(&self, id: &str) -> crate::error::Result<Option<RequestLog>> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                id, session_id, conversation_id, user_input, timestamp,
                result_type, result_summary, error_message,
                total_duration_ms, intent_duration_ms, routing_duration_ms, execution_duration_ms,
                metadata
             FROM request_logs WHERE id = ?"
        )?;
        
        let result = stmt.query_row([id], |row| {
            Self::row_to_request_log(row)
        });
        
        match result {
            Ok(mut log) => {
                log.stages = self.load_stages(&log.id)?;
                Ok(Some(log))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    /// 获取会话统计
    pub fn get_session_stats(&self, session_id: &str) -> crate::error::Result<SessionStats> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                COUNT(*),
                SUM(CASE WHEN result_type = 'success' THEN 1 ELSE 0 END),
                AVG(total_duration_ms)
             FROM request_logs WHERE session_id = ?"
        )?;
        
        let (total, success, avg_duration): (i64, i64, Option<f64>) = stmt.query_row(
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        
        Ok(SessionStats {
            total_requests: total as u64,
            successful_requests: success as u64,
            failed_requests: (total - success) as u64,
            average_duration_ms: avg_duration.unwrap_or(0.0) as u64,
        })
    }
    
    /// 获取全局统计
    pub fn get_global_stats(&self) -> crate::error::Result<SessionStats> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                COUNT(*),
                SUM(CASE WHEN result_type = 'success' THEN 1 ELSE 0 END),
                AVG(total_duration_ms)
             FROM request_logs"
        )?;
        
        let (total, success, avg_duration): (i64, i64, Option<f64>) = stmt.query_row(
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        
        Ok(SessionStats {
            total_requests: total as u64,
            successful_requests: success as u64,
            failed_requests: (total - success) as u64,
            average_duration_ms: avg_duration.unwrap_or(0.0) as u64,
        })
    }
    
    /// 清理旧日志
    /// 
    /// # 参数
    /// - `days`: 保留最近N天的日志
    /// 
    /// # 返回
    /// - `Result<usize>`: 清理的日志数量
    pub fn cleanup_old_logs(&self, days: u32) -> crate::error::Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days as i64);
        
        // 先清理旧日志的阶段数据
        self.conn.execute(
            "DELETE FROM request_stages WHERE request_id IN (
                SELECT id FROM request_logs WHERE timestamp < ?
            )",
            [cutoff.timestamp()],
        )?;
        
        // 再清理旧日志
        let result = self.conn.execute(
            "DELETE FROM request_logs WHERE timestamp < ?",
            [cutoff.timestamp()],
        )?;
        
        Ok(result)
    }
    
    /// 获取所有会话ID
    pub fn get_sessions(&self, limit: usize) -> crate::error::Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, COUNT(*) as count 
             FROM request_logs 
             GROUP BY session_id 
             ORDER BY MAX(timestamp) DESC 
             LIMIT ?"
        )?;
        
        let rows = stmt.query_map([limit as i64], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
            .pipe(Ok)
    }
    
    fn row_to_request_log(row: &Row) -> rusqlite::Result<RequestLog> {
        let result_type: String = row.get(5)?;
        let result_summary: Option<String> = row.get(6)?;
        let error_msg: Option<String> = row.get(7)?;
        
        let result = match result_type.as_str() {
            "success" => RequestResult::Success {
                skill_id: result_summary.unwrap_or_default(),
                output_summary: error_msg.unwrap_or_default(),
            },
            "no_match" => RequestResult::NoMatch {
                reason: error_msg.unwrap_or_default(),
            },
            "error" => RequestResult::Error {
                error: error_msg.unwrap_or_default(),
            },
            _ => RequestResult::Cancelled,
        };
        
        let metadata_json: String = row.get(12)?;
        let metadata: HashMap<String, String> = serde_json::from_str(&metadata_json)
            .unwrap_or_default();
        
        let timestamp = row.get::<_, i64>(4)?;
        
        Ok(RequestLog {
            id: row.get(0)?,
            session_id: row.get(1)?,
            conversation_id: row.get(2)?,
            user_input: row.get(3)?,
            timestamp: DateTime::from_timestamp(timestamp, 0).unwrap_or_else(Utc::now),
            stages: Vec::new(), // 单独加载
            result,
            metrics: RequestMetrics {
                total_duration_ms: row.get::<_, Option<i64>>(8)?.unwrap_or(0) as u64,
                intent_duration_ms: row.get::<_, Option<i64>>(9)?.unwrap_or(0) as u64,
                routing_duration_ms: row.get::<_, Option<i64>>(10)?.unwrap_or(0) as u64,
                execution_duration_ms: row.get::<_, Option<i64>>(11)?.unwrap_or(0) as u64,
            },
            metadata,
        })
    }
    
    fn load_stages(&self, request_id: &str) -> crate::error::Result<Vec<RequestStage>> {
        let mut stmt = self.conn.prepare(
            "SELECT stage_name, start_time, duration_ms, input, output, success, error
             FROM request_stages WHERE request_id = ? ORDER BY start_time"
        )?;
        
        let rows = stmt.query_map([request_id], |row| {
            let timestamp = row.get::<_, i64>(1)?;
            Ok(RequestStage {
                name: row.get(0)?,
                start_time: DateTime::from_timestamp(timestamp, 0).unwrap_or_else(Utc::now),
                duration_ms: row.get::<_, i64>(2)? as u64,
                input: row.get(3)?,
                output: row.get(4)?,
                success: row.get(5)?,
                error: row.get(6)?,
            })
        })?;
        
        rows.filter_map(|r| r.ok()).collect::<Vec<_>>()
            .pipe(Ok)
    }
}

/// 请求日志构建器（便捷API）
/// 
/// # 示例
/// 
/// ```rust,no_run
/// use cis_core::telemetry::RequestLogBuilder;
/// 
/// let mut builder = RequestLogBuilder::new("session-1", "用户输入");
/// builder.start_stage("intent_parse");
/// // ... 执行意图解析
/// builder.end_stage(true, Some("解析结果".to_string()), None);
/// ```
pub struct RequestLogBuilder {
    log: RequestLog,
    current_stage: Option<(String, Instant)>,
    start_time: Instant,
}

impl RequestLogBuilder {
    /// 创建新的日志构建器
    pub fn new(session_id: impl Into<String>, user_input: impl Into<String>) -> Self {
        Self {
            log: RequestLog {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.into(),
                conversation_id: None,
                user_input: user_input.into(),
                timestamp: Utc::now(),
                stages: Vec::new(),
                result: RequestResult::Cancelled,
                metrics: RequestMetrics::default(),
                metadata: HashMap::new(),
            },
            current_stage: None,
            start_time: Instant::now(),
        }
    }
    
    /// 设置对话ID
    pub fn with_conversation_id(mut self, id: impl Into<String>) -> Self {
        self.log.conversation_id = Some(id.into());
        self
    }
    
    /// 开始一个处理阶段
    pub fn start_stage(&mut self, name: impl Into<String>) {
        // 如果之前有未结束的stage，先结束它
        if self.current_stage.is_some() {
            self.end_stage(true, None, None);
        }
        self.current_stage = Some((name.into(), Instant::now()));
    }
    
    /// 结束当前处理阶段
    /// 
    /// # 参数
    /// - `success`: 是否成功
    /// - `output`: 阶段输出（可选）
    /// - `error`: 错误信息（可选）
    pub fn end_stage(&mut self, success: bool, output: Option<String>, error: Option<String>) {
        if let Some((name, start)) = self.current_stage.take() {
            let duration = start.elapsed();
            let duration_chrono = chrono::Duration::from_std(duration).unwrap_or_default();
            self.log.stages.push(RequestStage {
                name,
                start_time: Utc::now() - duration_chrono,
                duration_ms: duration.as_millis() as u64,
                input: None,
                output,
                success,
                error,
            });
        }
    }
    
    /// 设置结果
    pub fn set_result(mut self, result: RequestResult) -> Self {
        self.log.result = result;
        self
    }
    
    /// 设置性能指标
    pub fn set_metrics(mut self, metrics: RequestMetrics) -> Self {
        self.log.metrics = metrics;
        self
    }
    
    /// 添加元数据
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.log.metadata.insert(key.into(), value.into());
        self
    }
    
    /// 构建最终的RequestLog
    /// 
    /// 自动计算总耗时并确保所有阶段已结束
    pub fn build(mut self) -> RequestLog {
        // 结束任何未结束的stage
        if self.current_stage.is_some() {
            self.end_stage(true, None, None);
        }
        
        // 计算总耗时
        self.log.metrics.total_duration_ms = self.start_time.elapsed().as_millis() as u64;
        
        self.log
    }
    
    /// 获取当前日志引用（用于查看中间状态）
    pub fn current_log(&self) -> &RequestLog {
        &self.log
    }
}

// Helper trait for pipe
trait Pipe<T> {
    fn pipe<R, F: FnOnce(T) -> R>(self, f: F) -> R;
}

impl<T> Pipe<T> for T {
    fn pipe<R, F: FnOnce(T) -> R>(self, f: F) -> R {
        f(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    struct TestLogger {
        logger: RequestLogger,
        _dir: TempDir,
        path: PathBuf,
    }

    fn create_test_logger() -> TestLogger {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test_telemetry.db");
        let logger = RequestLogger::open(&path, None).unwrap();
        TestLogger { logger, _dir: dir, path }
    }

    fn create_test_log(session_id: &str, user_input: &str) -> RequestLog {
        RequestLogBuilder::new(session_id, user_input)
            .with_conversation_id("test-conv-1")
            .set_result(RequestResult::Success {
                skill_id: "test_skill".to_string(),
                output_summary: "测试成功".to_string(),
            })
            .set_metrics(RequestMetrics {
                total_duration_ms: 100,
                intent_duration_ms: 20,
                routing_duration_ms: 30,
                execution_duration_ms: 50,
            })
            .add_metadata("test_key", "test_value")
            .build()
    }

    #[test]
    fn test_request_logger_open() {
        let test = create_test_logger();
        assert!(test.path.exists());
    }

    #[test]
    fn test_log_request() {
        let test = create_test_logger();
        let log = create_test_log("session-1", "测试输入");
        
        let result = test.logger.log_request(&log);
        assert!(result.is_ok());
    }

    #[test]
    fn test_query_logs() {
        let test = create_test_logger();
        let log = create_test_log("session-1", "测试输入");
        test.logger.log_request(&log).unwrap();
        
        let query = LogQuery::new()
            .with_session("session-1")
            .with_limit(10);
        
        let logs = test.logger.query_logs(&query).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].user_input, "测试输入");
    }

    #[test]
    fn test_session_stats() {
        let test = create_test_logger();
        
        // 添加多个日志
        for i in 0..5 {
            let log = create_test_log("session-stats", &format!("输入 {}", i));
            test.logger.log_request(&log).unwrap();
        }
        
        let stats = test.logger.get_session_stats("session-stats").unwrap();
        assert_eq!(stats.total_requests, 5);
        assert_eq!(stats.successful_requests, 5);
        assert_eq!(stats.failed_requests, 0);
    }

    #[test]
    fn test_log_with_stages() {
        let test = create_test_logger();
        
        let mut builder = RequestLogBuilder::new("session-stages", "测试分阶段");
        builder.start_stage("intent_parse");
        std::thread::sleep(std::time::Duration::from_millis(10));
        builder.end_stage(true, Some("意图解析完成".to_string()), None);
        
        builder.start_stage("skill_route");
        std::thread::sleep(std::time::Duration::from_millis(10));
        builder.end_stage(true, Some("路由成功".to_string()), None);
        
        let log = builder
            .set_result(RequestResult::Success {
                skill_id: "test_skill".to_string(),
                output_summary: "完成".to_string(),
            })
            .build();
        
        test.logger.log_request(&log).unwrap();
        
        // 查询并验证阶段
        let query = LogQuery::new().with_session("session-stages");
        let logs = test.logger.query_logs(&query).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].stages.len(), 2);
        assert_eq!(logs[0].stages[0].name, "intent_parse");
        assert_eq!(logs[0].stages[1].name, "skill_route");
    }

    #[test]
    fn test_cleanup_old_logs() {
        let test = create_test_logger();
        
        // 创建一个2天前的日志
        let old_log = RequestLog {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: "session-cleanup".to_string(),
            conversation_id: Some("test-conv".to_string()),
            user_input: "测试清理".to_string(),
            timestamp: Utc::now() - chrono::Duration::days(2),
            stages: vec![],
            result: RequestResult::Success {
                skill_id: "test_skill".to_string(),
                output_summary: "测试".to_string(),
            },
            metrics: RequestMetrics::default(),
            metadata: HashMap::new(),
        };
        test.logger.log_request(&old_log).unwrap();
        
        // 验证日志已添加
        let query = LogQuery::new().with_session("session-cleanup");
        let logs = test.logger.query_logs(&query).unwrap();
        assert_eq!(logs.len(), 1);
        
        // 清理1天前的日志（应该清理2天前的日志）
        let count = test.logger.cleanup_old_logs(1).unwrap();
        assert_eq!(count, 1);
        
        // 验证已清理
        let logs = test.logger.query_logs(&query).unwrap();
        assert!(logs.is_empty());
    }

    #[test]
    fn test_request_result_variants() {
        let test = create_test_logger();
        
        // 成功
        let log1 = RequestLogBuilder::new("session-result", "成功测试")
            .set_result(RequestResult::Success {
                skill_id: "skill1".to_string(),
                output_summary: "成功".to_string(),
            })
            .build();
        test.logger.log_request(&log1).unwrap();
        
        // 无匹配
        let log2 = RequestLogBuilder::new("session-result", "无匹配测试")
            .set_result(RequestResult::NoMatch {
                reason: "没有匹配的技能".to_string(),
            })
            .build();
        test.logger.log_request(&log2).unwrap();
        
        // 错误
        let log3 = RequestLogBuilder::new("session-result", "错误测试")
            .set_result(RequestResult::Error {
                error: "执行失败".to_string(),
            })
            .build();
        test.logger.log_request(&log3).unwrap();
        
        // 取消
        let log4 = RequestLogBuilder::new("session-result", "取消测试")
            .set_result(RequestResult::Cancelled)
            .build();
        test.logger.log_request(&log4).unwrap();
        
        let query = LogQuery::new().with_session("session-result");
        let logs = test.logger.query_logs(&query).unwrap();
        assert_eq!(logs.len(), 4);
    }

    #[test]
    fn test_log_builder() {
        let log = RequestLogBuilder::new("test-session", "用户输入")
            .with_conversation_id("conv-123")
            .add_metadata("key1", "value1")
            .add_metadata("key2", "value2")
            .set_result(RequestResult::Success {
                skill_id: "my_skill".to_string(),
                output_summary: "执行成功".to_string(),
            })
            .build();
        
        assert_eq!(log.session_id, "test-session");
        assert_eq!(log.user_input, "用户输入");
        assert_eq!(log.conversation_id, Some("conv-123".to_string()));
        assert_eq!(log.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(log.metadata.get("key2"), Some(&"value2".to_string()));
        
        match log.result {
            RequestResult::Success { skill_id, output_summary } => {
                assert_eq!(skill_id, "my_skill");
                assert_eq!(output_summary, "执行成功");
            }
            _ => panic!("Unexpected result type"),
        }
    }
}
