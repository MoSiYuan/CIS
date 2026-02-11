以下是 OpenClaw 解决 Claude Code 无头模式（Headless Mode）问题的纯技术实现拆解：

## 1. PTY (Pseudo-Terminal) 强制分配层

**核心问题**：Claude Code 即使使用 `-p` 模式，底层仍依赖 TTY/PTY 进行 stdin/stdout 控制，无终端环境（systemd/cron/Docker）触发 `setRawMode failed` 或直接挂起。

**解决方案**：
- **包装脚本**：`scripts/claude_code_run.py` 使用系统 `script` 命令强制分配伪终端
  ```bash
  script -q -c 'claude -p "prompt"' /dev/null
  ```
- **机制**：`script -q -c ... /dev/null` 欺骗 Claude Code 使其认为处于交互式环境，绕过 TTY 检测
- **替代方案**：Java 层使用 `pty4j`，Python 层使用 `pty` 模块，或 `unbuffer`（expect）

## 2. 权限确认的非交互式绕过

**核心问题**：Claude Code 默认会在执行工具前停止等待用户确认（`Y/n`），打断自动化流程。

**解决方案**：
- **计划模式**：`--permission-mode plan` 仅生成执行计划，不实际运行工具，完全避免交互
- **工具白名单**：`--allowedTools "Bash,Read,Edit"` 显式预设可调用工具，范围内自动执行无需确认
- **全局自动批准**：`--auto-approve-once` 或 `--non-interactive` 配合配置文件实现单次/持续自动批准
- **Wingman 机制**：在 `tmux` 中运行 Claude Code，首次手动建立信任会话后，后续权限提示自动批准

## 3. 会话持久化架构（解决单次执行限制）

**核心问题**：`claude -p` 是单次进程，长任务超时或崩溃后状态丢失，无法实现持续对话或长时运行。

**解决方案**：
- **Tmux/Session 管理**：通过 `clawdbot-skill` 在 `tmux` 或 `screen` 会话中启动 Claude Code
  ```bash
  tmux new-session -d -s <session_id> "claude_code_run.py ..."
  ```
- **Detach/Attach 机制**：Gateway 与执行层解耦，Matrix/Telegram 消息触发后 Gateway 立即返回，Claude Code 在后台 tmux 中持续运行
- **状态捕获**：通过 `tmux capture-pane` 或重定向日志文件获取执行输出
- **异步回调**：任务完成后通过 Matrix/Telegram API 主动推送结果，而非同步等待

## 4. Agent Runner 常驻架构

**核心问题**：传统 Serverless/HTTP 请求模式无法维持 Claude Code 的长期状态。

**解决方案**：
- **长期运行进程**：Agent Runner 作为常驻服务（非无状态函数），通过 WebSocket 与 Gateway 保持长连接
- **心跳机制**：Agent 定时自唤醒（Heartbeat），检查队列中的新任务或恢复中断的执行
- **Lane Queue**：默认串行队列管理，防止多任务并发导致的权限竞态条件

## 5. 安全默认与降级策略

- **只读默认**：`--permission-mode plan` 或 Read-Only Skill 作为安全基线，写操作必须通过通知系统请求人工确认
- **环境隔离**：Claude Code 在独立 tmux 会话中运行，与 Gateway 进程隔离，崩溃不影响主服务
- **最小权限**：`--allowedTools` 严格限制可调用工具集，避免 `Write` 等高危操作自动执行

## 6. 工具链封装结构

```
Gateway (WebSocket 长连接)
    ↓
Agent Runner (常驻进程)
    ↓
PTY Wrapper (script/tmux)
    ↓
Claude Code CLI (claude -p)
```

**关键技术点**：不直接调用 `claude` 二进制，而是通过 `claude_code_run.py` 包装器注入 PTY 和权限参数，再由包装器管理子进程生命周期。