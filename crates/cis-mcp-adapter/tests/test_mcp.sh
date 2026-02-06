#!/bin/bash
# MCP 服务测试脚本

set -e

echo "========================================"
echo "CIS MCP Server 测试"
echo "========================================"

# 检查二进制文件是否存在
if [ ! -f "../target/debug/cis-mcp" ]; then
    echo "构建 cis-mcp..."
    cd ..
    cargo build -p cis-mcp-adapter
    cd tests
fi

# 测试 1: 检查工具列表
echo ""
echo "测试 1: 获取工具列表"
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | ../target/debug/cis-mcp 2>/dev/null | head -1 | jq '.result.tools | length'

# 测试 2: 初始化
echo ""
echo "测试 2: 初始化"
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' | ../target/debug/cis-mcp 2>/dev/null | head -1 | jq '.result.serverInfo.name'

# 测试 3: 上下文提取
echo ""
echo "测试 3: 上下文提取"
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"context_extract","arguments":{}}}' | ../target/debug/cis-mcp 2>/dev/null | head -1 | jq '.result.content[0].text' | head -c 100

echo ""
echo ""
echo "========================================"
echo "测试完成"
echo "========================================"
