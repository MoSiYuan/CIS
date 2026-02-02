-- 飞书 IM Skill 数据库 Schema
--
-- ⚠️ 重要: 这是 IM 信息数据库（feishu_im.db），严格分离于记忆数据库（memory.db）
--
-- 数据用途:
--   - 对话历史管理
--   - 用户信息缓存
--   - 群组信息缓存
--   - Webhook 日志
--
-- 不存储:
--   - 业务记忆 → 存储在 memory.db
--   - 项目知识 → 存储在 memory.db
--   - 技能经验 → 存储在 memory.db

-- ========================================
-- 会话表
-- ========================================
CREATE TABLE IF NOT EXISTS sessions (
    session_id TEXT PRIMARY KEY,
    session_type TEXT NOT NULL,  -- 'private' | 'group'
    user_id TEXT,                -- 私聊: 用户ID
    group_id TEXT,               -- 群聊: 群组ID
    created_at INTEGER NOT NULL,
    last_active INTEGER NOT NULL,
    message_count INTEGER DEFAULT 0
);

-- ========================================
-- 消息表
-- ========================================
CREATE TABLE IF NOT EXISTS messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL UNIQUE,  -- 飞书消息 ID（幂等）
    role TEXT NOT NULL,               -- 'user' | 'assistant'
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
);

-- ========================================
-- 用户缓存表
-- ========================================
CREATE TABLE IF NOT EXISTS users (
    user_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    avatar_url TEXT,
    updated_at INTEGER NOT NULL
);

-- ========================================
-- 群组缓存表
-- ========================================
CREATE TABLE IF NOT EXISTS groups (
    group_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    owner_id TEXT,
    member_count INTEGER,
    updated_at INTEGER NOT NULL
);

-- ========================================
-- Webhook 日志表
-- ========================================
CREATE TABLE IF NOT EXISTS webhook_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL,
    event_id TEXT,
    payload TEXT,
    processed_at INTEGER NOT NULL,
    success INTEGER NOT NULL,  -- 0: 失败, 1: 成功
    error_msg TEXT
);

-- ========================================
-- 索引
-- ========================================
CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_created_at ON messages(created_at);
CREATE INDEX IF NOT EXISTS idx_sessions_last_active ON sessions(last_active);
CREATE INDEX IF NOT EXISTS idx_webhook_logs_event_id ON webhook_logs(event_id);

-- ========================================
-- 触发器
-- ========================================

-- 更新会话的最后活跃时间和消息计数
CREATE TRIGGER IF NOT EXISTS update_session_after_message
AFTER INSERT ON messages
BEGIN
    UPDATE sessions
    SET last_active = NEW.created_at,
        message_count = message_count + 1
    WHERE session_id = NEW.session_id;
END;

-- ========================================
-- 初始化数据（可选）
-- ========================================

-- 插入系统会话（用于调试）
INSERT OR IGNORE INTO sessions (session_id, session_type, created_at, last_active, message_count)
VALUES ('system', 'system', strftime('%s', 'now'), strftime('%s', 'now'), 0);
