#!/bin/bash
# 真实 CIS 组网测试脚本

set -e

echo "=== 真实 CIS 组网测试 ==="
echo "时间: $(date)"
echo

# 确保两个节点都有配置
for node in cis-node1 cis-node2; do
    docker exec $node mkdir -p /root/.cis 2>/dev/null || true
    docker exec $node test -f /root/.cis/config.toml 2>/dev/null || echo "  $node 需要配置"
done

echo "1. 在 node1 启动 pair generate（后台）..."
docker exec -d cis-node1 bash -c '
    export TERM=xterm
    cis-node pair generate --timeout 60 > /tmp/pair.log 2>&1 &
    sleep 2
    cat /tmp/pair.log
'
sleep 5

echo "2. 获取配对码..."
CODE=$(docker exec cis-node1 cat /tmp/pair.log 2>/dev/null | grep -oE '[0-9]{6}' | head -1)

if [ -z "$CODE" ]; then
    echo "   无法获取配对码，查看日志..."
    docker exec cis-node1 cat /tmp/pair.log 2>/dev/null | tail -10
    exit 1
fi

echo "   ✅ 配对码: $CODE"
echo

echo "3. 在 node2 执行 pair join..."
docker exec cis-node2 cis-node pair join "$CODE" --address 172.30.1.11:6768 &
JOIN_PID=$!

# 等待最多 15 秒
for i in {1..15}; do
    if ! kill -0 $JOIN_PID 2>/dev/null; then
        break
    fi
    sleep 1
    echo -n "."
done

# 如果还在运行，尝试获取结果
if kill -0 $JOIN_PID 2>/dev/null; then
    kill $JOIN_PID 2>/dev/null || true
fi
wait $JOIN_PID 2>/dev/null || true

echo
echo "4. 检查组网结果..."
docker exec cis-node2 cis-node neighbor list 2>/dev/null || echo "   邻居列表为空"

echo
echo "=== 测试完成 ==="
