#!/bin/bash
# CIS Shell Integration for Bash
# 安装方法: source /usr/local/share/cis/cis.bash
# 或添加到 ~/.bashrc: echo 'source /usr/local/share/cis/cis.bash' >> ~/.bashrc

# ===== 命令补全 =====

# 检查 cis-node 是否存在
if ! command -v cis-node &> /dev/null; then
    return
fi

# 生成并加载补全脚本（如果支持）
if cis-node completions bash &>/dev/null; then
    eval "$(cis-node completions bash 2>/dev/null)"
fi

if command -v cis-cli &>/dev/null && cis-cli completions bash &>/dev/null; then
    eval "$(cis-cli completions bash 2>/dev/null)"
fi

# ===== 别名定义 =====

# 基础别名
alias cis-start='cis node start'
alias cis-stop='cis node stop'
alias cis-status='cis node status'
alias cis-init='cis init'
alias cis-config='${EDITOR:-nano} ~/.cis/config.toml'

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
cis-run() {
    if [ -z "$1" ]; then
        echo "Usage: cis-run <dag-name> [args...]"
        return 1
    fi
    local dag_name="$1"
    shift
    cis dag run "$dag_name" "$@"
}

# 快速搜索记忆
cis-search() {
    if [ -z "$1" ]; then
        echo "Usage: cis-search <query>"
        return 1
    fi
    cis memory search "$1"
}

# 查看任务状态
cis-watch() {
    local task_id="${1:-}"
    if [ -z "$task_id" ]; then
        watch -n 2 'cis task list --limit 20'
    else
        watch -n 2 "cis task show $task_id"
    fi
}

# 进入 CIS 项目目录并自动设置环境
cis-cd() {
    local project="$1"
    if [ -z "$project" ]; then
        cd ~/.cis/projects 2>/dev/null || cd ~/.cis
    elif [ -d "$HOME/.cis/projects/$project" ]; then
        cd "$HOME/.cis/projects/$project"
        # 自动加载项目环境变量
        if [ -f ".cisrc" ]; then
            source .cisrc
            echo "已加载项目环境: $project"
        fi
    else
        echo "项目不存在: $project"
        return 1
    fi
}

# DAG 自动补全函数
_cis_complete_dags() {
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local dags=$(cis dag list --format plain 2>/dev/null | cut -d' ' -f1)
    COMPREPLY=($(compgen -W "$dags" -- "$cur"))
}

# 为 cis-run 添加补全
complete -F _cis_complete_dags cis-run
complete -F _cis_complete_dags cis-dag-run

# ===== chpwd 钩子 =====

# 保存原始的 chpwd 函数
if declare -f __cis_original_chpwd &>/dev/null; then
    : # 已经加载过
else
    # 定义 chpwd 钩子
    __cis_chpwd_hook() {
        # 检查当前目录是否有 .cis 配置文件
        if [ -f ".cis/config.toml" ]; then
            # 可选：显示 CIS 项目信息
            if [ "${CIS_CHPWD_VERBOSE:-0}" = "1" ]; then
                echo "📦 CIS 项目目录: $(basename $(pwd))"
            fi
            
            # 自动设置 CIS_HOME（如果未设置）
            if [ -z "$CIS_HOME" ]; then
                export CIS_HOME="$(pwd)/.cis"
            fi
        fi
        
        # 调用原始的 chpwd（如果有）
        if declare -f __cis_original_chpwd &>/dev/null; then
            __cis_original_chpwd "$@"
        fi
    }
    
    # 保存原始的 cd 命令
    if ! declare -f __cis_original_cd &>/dev/null; then
        eval "__cis_original_cd() { $(declare -f cd | tail -n +2 | head -n -1); }"
    fi
    
    # 包装 cd 命令
    cd() {
        builtin cd "$@" && __cis_chpwd_hook
    }
fi

# ===== 提示符集成 =====

# 可选：在提示符中显示 CIS 节点状态
__cis_prompt() {
    local cis_status=""
    
    # 检查节点是否运行
    if cis node status &>/dev/null; then
        local node_count=$(cis network list --format json 2>/dev/null | grep -c '"id"' || echo "0")
        cis_status=" 🟢[${node_count}]"
    fi
    
    echo "$cis_status"
}

# 启用提示符集成（取消注释下面两行）
# PS1='$(__cis_prompt)'$PS1

# ===== 环境变量 =====

# 设置默认编辑器
export CIS_EDITOR="${CIS_EDITOR:-${EDITOR:-nano}}"

# 设置日志级别
export CIS_LOG_LEVEL="${CIS_LOG_LEVEL:-info}"

# ===== 欢迎信息 =====

if [ "${CIS_SHELL_SILENT:-0}" != "1" ]; then
    # 检查 CIS 是否已初始化
    if [ ! -d "$HOME/.cis" ]; then
        echo "💡 提示: CIS 尚未初始化，运行 'cis init' 开始"
    fi
fi
