#!/bin/zsh
# CIS Shell Integration for Zsh
# 安装方法: source /usr/local/share/cis/cis.zsh
# 或添加到 ~/.zshrc: echo 'source /usr/local/share/cis/cis.zsh' >> ~/.zshrc

# ===== 命令补全 =====

# 检查 cis-node 是否存在
(( $+commands[cis-node] )) || return

# 生成并加载补全脚本
if cis-node completions zsh &>/dev/null; then
    eval "$(cis-node completions zsh 2>/dev/null)"
fi

if (( $+commands[cis-cli] )) && cis-cli completions zsh &>/dev/null; then
    eval "$(cis-cli completions zsh 2>/dev/null)"
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
    if [[ -z "$1" ]]; then
        echo "Usage: cis-run <dag-name> [args...]"
        return 1
    fi
    local dag_name="$1"
    shift
    cis dag run "$dag_name" "$@"
}

# 快速搜索记忆
cis-search() {
    if [[ -z "$1" ]]; then
        echo "Usage: cis-search <query>"
        return 1
    fi
    cis memory search "$1"
}

# 查看任务状态
cis-watch() {
    local task_id="${1:-}"
    if [[ -z "$task_id" ]]; then
        watch -n 2 'cis task list --limit 20'
    else
        watch -n 2 "cis task show $task_id"
    fi
}

# 进入 CIS 项目目录并自动设置环境
cis-cd() {
    local project="$1"
    if [[ -z "$project" ]]; then
        cd ~/.cis/projects 2>/dev/null || cd ~/.cis
    elif [[ -d "$HOME/.cis/projects/$project" ]]; then
        cd "$HOME/.cis/projects/$project"
        # 自动加载项目环境变量
        if [[ -f ".cisrc" ]]; then
            source .cisrc
            echo "已加载项目环境: $project"
        fi
    else
        echo "项目不存在: $project"
        return 1
    fi
}

# 补全函数
_cis_complete_dags() {
    local -a dags
    dags=(${(f)"$(cis dag list --format plain 2>/dev/null | cut -d' ' -f1)"})
    _describe -t dags 'DAGs' dags
}

# 注册补全
compdef _cis_complete_dags cis-run
compdef _cis_complete_dags cis-dag-run

# ===== chpwd 钩子 =====

autoload -U add-zsh-hook

__cis_chpwd() {
    # 检查当前目录是否有 .cis 配置文件
    if [[ -f ".cis/config.toml" ]]; then
        # 可选：显示 CIS 项目信息
        if [[ "${CIS_CHPWD_VERBOSE:-0}" == "1" ]]; then
            echo "📦 CIS 项目目录: $(basename $(pwd))"
        fi
        
        # 自动设置 CIS_HOME（如果未设置）
        if [[ -z "$CIS_HOME" ]]; then
            export CIS_HOME="$(pwd)/.cis"
        fi
    fi
}

add-zsh-hook chpwd __cis_chpwd

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

# 启用提示符集成（取消注释下面）
# setopt prompt_subst
# PROMPT='$(__cis_prompt)'$PROMPT

# ===== fzf 集成（如果安装了 fzf） =====

if (( $+commands[fzf] )); then
    # 交互式选择并运行 DAG
    cis-dag-fzf() {
        local dag=$(cis dag list --format plain 2>/dev/null | fzf --preview 'cis dag show {1}' --preview-window=right:50%)
        if [[ -n "$dag" ]]; then
            local dag_name=$(echo "$dag" | cut -d' ' -f1)
            cis dag run "$dag_name"
        fi
    }
    
    # 交互式搜索记忆
    cis-mem-fzf() {
        local query="${1:-}"
        local results
        if [[ -n "$query" ]]; then
            results=$(cis memory search "$query" --format plain 2>/dev/null)
        else
            results=$(cis memory list --limit 100 --format plain 2>/dev/null)
        fi
        
        local selected=$(echo "$results" | fzf --preview 'cis memory show {1}' --preview-window=right:50%)
        if [[ -n "$selected" ]]; then
            local mem_id=$(echo "$selected" | cut -d' ' -f1)
            cis memory show "$mem_id"
        fi
    }
    
    alias cdf='cis-dag-fzf'
    alias cmf='cis-mem-fzf'
fi

# ===== 环境变量 =====

export CIS_EDITOR="${CIS_EDITOR:-${EDITOR:-nano}}"
export CIS_LOG_LEVEL="${CIS_LOG_LEVEL:-info}"

# ===== 欢迎信息 =====

if [[ "${CIS_SHELL_SILENT:-0}" != "1" ]]; then
    if [[ ! -d "$HOME/.cis" ]]; then
        echo "💡 提示: CIS 尚未初始化，运行 'cis init' 开始"
    fi
fi
