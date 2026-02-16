#!/bin/bash
# Memory Conflicts CLI 演示脚本
#
# 本脚本演示如何使用 Memory Conflicts CLI 命令

set -e

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║     CIS Memory Conflicts CLI 演示                            ║"
echo "║     版本: v1.1.6                                              ║"
echo "║     日期: 2026-02-15                                          ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# 颜色定义
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_section() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}📌 $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

# 检查 CIS 是否可用
check_cis() {
    print_section "检查 CIS 环境"

    if ! command -v cis &> /dev/null; then
        print_error "CIS 命令未找到"
        echo "   请确保 CIS 已安装并在 PATH 中"
        echo "   或者使用: cargo run --bin cis-node --"
        exit 1
    fi

    print_success "CIS 命令已找到"

    # 显示版本
    echo ""
    print_info "CIS 版本信息:"
    cis --version 2>&1 || echo "   (无法获取版本信息)"
}

# 演示 1: 列出冲突
demo_list() {
    print_section "演示 1: 列出所有未解决的冲突"

    print_info "执行命令: cis memory conflicts list"
    echo ""

    cis memory conflicts list

    echo ""
    print_success "命令执行完成"
}

# 演示 2: 检测冲突
demo_detect() {
    print_section "演示 2: 检测特定键的冲突"

    print_info "执行命令: cis memory conflicts detect -k user/preference/theme"
    echo ""

    cis memory conflicts detect -k user/preference/theme

    echo ""
    print_info "执行命令: cis memory conflicts detect -k key1,key2,key3"
    echo ""

    cis memory conflicts detect -k key1,key2,key3

    echo ""
    print_success "命令执行完成"
}

# 演示 3: 解决冲突
demo_resolve() {
    print_section "演示 3: 解决冲突"

    print_info "场景: 解决一个假设的冲突"
    echo ""

    # 演示不同的解决选项
    print_info "选项 1: 保留本地版本"
    echo "   命令: cis memory conflicts resolve -i conflict-abc-123 -c 1"
    cis memory conflicts resolve -i conflict-abc-123 -c 1
    echo ""

    print_info "选项 2: 保留远程版本"
    echo "   命令: cis memory conflicts resolve -i conflict-def-456 -c KeepRemote"
    cis memory conflicts resolve -i conflict-def-456 -c KeepRemote
    echo ""

    print_info "选项 3: 保留两个版本"
    echo "   命令: cis memory conflicts resolve -i conflict-ghi-789 -c 3"
    cis memory conflicts resolve -i conflict-ghi-789 -c 3
    echo ""

    print_info "选项 4: AI 智能合并"
    echo "   命令: cis memory conflicts resolve -i conflict-jkl-012 -c AIMerge"
    cis memory conflicts resolve -i conflict-jkl-012 -c AIMerge

    echo ""
    print_success "命令执行完成"
}

# 演示 4: 查看帮助
demo_help() {
    print_section "演示 4: 查看帮助信息"

    print_info "查看总体帮助: cis memory conflicts --help"
    echo ""

    cis memory conflicts --help

    echo ""
    print_info "查看 list 子命令帮助: cis memory conflicts list --help"
    echo ""

    cis memory conflicts list --help

    echo ""
    print_success "帮助信息显示完成"
}

# 主函数
main() {
    check_cis

    # 询问用户要演示哪些功能
    echo ""
    print_section "选择演示内容"

    echo "请选择要演示的功能:"
    echo "  1) 列出冲突"
    echo "  2) 检测冲突"
    echo "  3) 解决冲突"
    echo "  4) 查看帮助"
    echo "  5) 全部演示"
    echo "  0) 退出"
    echo ""

    read -p "请输入选项 [0-5]: " choice

    case $choice in
        1)
            demo_list
            ;;
        2)
            demo_detect
            ;;
        3)
            demo_resolve
            ;;
        4)
            demo_help
            ;;
        5)
            demo_list
            demo_detect
            demo_resolve
            demo_help
            ;;
        0)
            print_info "退出演示"
            exit 0
            ;;
        *)
            print_error "无效的选项"
            exit 1
            ;;
    esac

    # 总结
    print_section "演示总结"

    print_success "所有演示已完成!"
    echo ""
    echo "📚 更多信息:"
    echo "   - 快速参考: docs/plan/v1.1.6/MEMORY_CONFLICTS_CLI_QUICK_START.md"
    echo "   - 集成报告: docs/plan/v1.1.6/MEMORY_CONFLICTS_CLI_INTEGRATION.md"
    echo "   - 使用帮助: cis memory conflicts --help"
    echo ""
}

# 运行主函数
main "$@"
