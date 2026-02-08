#!/usr/bin/env fish
# CIS Shell Integration for Fish
# 安装方法: source /usr/local/share/cis/cis.fish
# 或添加到 config.fish: echo 'source /usr/local/share/cis/cis.fish' >> ~/.config/fish/config.fish

# ===== 命令补全 =====

# 检查 cis-node 是否存在
if not command -q cis-node
    exit
end

# 生成并加载补全脚本
if cis-node completions fish &>/dev/null
    cis-node completions fish 2>/dev/null | source
end

if command -q cis-cli; and cis-cli completions fish &>/dev/null
    cis-cli completions fish 2>/dev/null | source
end

# ===== 别名定义 =====

# 基础别名
alias cis-start='cis node start'
alias cis-stop='cis node stop'
alias cis-status='cis node status'
alias cis-init='cis init'
alias cis-config='$EDITOR ~/.cis/config.toml'

# DAG 相关
alias cis-dag-list='cis dag list'
alias cis-dag-run='cis dag run'
alias cis-dag-status='cis dag status'
alias cis-dag-logs='cis dag logs'

# 任务相关
alias cis-task-list='cis task list'
alias cis-task-show='cis task show'
alias cis-task-logs='cis task logs'

# 记忆相关
alias cis-mem-search='cis memory search'
alias cis-mem-list='cis memory list'
alias cis-mem-stat='cis memory stat'

# 网络相关
alias cis-peers='cis network list'
alias cis-allow='cis network allow'
alias cis-deny='cis network deny'

# 系统相关
alias cis-health='cis doctor'
alias cis-logs='tail -f ~/.cis/logs/cis.log'
alias cis-top='cis system top'

# Skill 相关
alias cis-skills='cis skill list'
alias cis-do='cis skill do'

# ===== 快捷函数 =====

# 快速执行 DAG
function cis-run
    if test -z "$argv[1]"
        echo "Usage: cis-run <dag-name> [args...]"
        return 1
    end
    set -l dag_name $argv[1]
    set -e argv[1]
    cis dag run $dag_name $argv
end

# 快速搜索记忆
function cis-search
    if test -z "$argv[1]"
        echo "Usage: cis-search <query>"
        return 1
    end
    cis memory search $argv[1]
end

# 查看任务状态
function cis-watch
    set -l task_id $argv[1]
    if test -z "$task_id"
        watch -n 2 'cis task list --limit 20'
    else
        watch -n 2 "cis task show $task_id"
    end
end

# 进入 CIS 项目目录并自动设置环境
function cis-cd
    set -l project $argv[1]
    if test -z "$project"
        cd ~/.cis/projects 2>/dev/null; or cd ~/.cis
    else if test -d "$HOME/.cis/projects/$project"
        cd "$HOME/.cis/projects/$project"
        # 自动加载项目环境变量
        if test -f ".cisrc"
            source .cisrc
            echo "已加载项目环境: $project"
        end
    else
        echo "项目不存在: $project"
        return 1
    end
end

# ===== chpwd 钩子 =====

function __cis_chpwd --on-variable PWD
    # 检查当前目录是否有 .cis 配置文件
    if test -f ".cis/config.toml"
        # 可选：显示 CIS 项目信息
        if test "$CIS_CHPWD_VERBOSE" = "1"
            echo "📦 CIS 项目目录: "(basename (pwd))
        end
        
        # 自动设置 CIS_HOME（如果未设置）
        if test -z "$CIS_HOME"
            set -gx CIS_HOME (pwd)/.cis
        end
    end
end

# ===== 提示符集成 =====

# 可选：在提示符中显示 CIS 节点状态
function __cis_prompt
    set -l cis_status ""
    
    # 检查节点是否运行
    if cis node status &>/dev/null
        set -l node_count (cis network list --format json 2>/dev/null | grep -c '"id"'; or echo "0")
        set cis_status " 🟢[$node_count]"
    end
    
    echo $cis_status
end

# 启用提示符集成（添加到 fish_prompt 函数）
# function fish_prompt
#     printf '%s%s' (__cis_prompt) $PWD ' > '
# end

# ===== fzf 集成（如果安装了 fzf） =====

if command -q fzf
    # 交互式选择并运行 DAG
    function cis-dag-fzf
        set -l dag (cis dag list --format plain 2>/dev/null | fzf --preview 'cis dag show {1}' --preview-window=right:50%)
        if test -n "$dag"
            set -l dag_name (echo $dag | cut -d' ' -f1)
            cis dag run $dag_name
        end
    end
    
    # 交互式搜索记忆
    function cis-mem-fzf
        set -l query $argv[1]
        set -l results
        if test -n "$query"
            set results (cis memory search $query --format plain 2>/dev/null)
        else
            set results (cis memory list --limit 100 --format plain 2>/dev/null)
        end
        
        set -l selected (echo $results | fzf --preview 'cis memory show {1}' --preview-window=right:50%)
        if test -n "$selected"
            set -l mem_id (echo $selected | cut -d' ' -f1)
            cis memory show $mem_id
        end
    end
    
    alias cdf='cis-dag-fzf'
    alias cmf='cis-mem-fzf'
end

# ===== 环境变量 =====

set -gx CIS_EDITOR "$CIS_EDITOR"
if test -z "$CIS_EDITOR"
    set -gx CIS_EDITOR "$EDITOR"
end
if test -z "$CIS_EDITOR"
    set -gx CIS_EDITOR "nano"
end

set -gx CIS_LOG_LEVEL "${CIS_LOG_LEVEL:-info}"

# ===== 欢迎信息 =====

if test "$CIS_SHELL_SILENT" != "1"
    if not test -d "$HOME/.cis"
        echo "💡 提示: CIS 尚未初始化，运行 'cis init' 开始"
    end
end
