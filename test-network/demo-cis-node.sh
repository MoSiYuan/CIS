#!/bin/sh
# 模拟 cis-node 命令用于组网测试演示

LOG_FILE=/var/log/cis-node.log
mkdir -p /var/lib/cis/data /var/log/cis /etc/cis

case "$1" in
    daemon)
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] CIS daemon started" >> "$LOG_FILE"
        echo "  Node: $CIS_NODE_ID"
        echo "  Name: $CIS_NODE_NAME"
        echo "  Role: $CIS_NODE_ROLE"
        echo "  DID: $CIS_DID"
        # 模拟后台运行
        while true; do
            echo "[$(date '+%Y-%m-%d %H:%M:%S')] Heartbeat" >> "$LOG_FILE"
            sleep 60
        done
        ;;
    pair)
        case "$2" in
            generate)
                CODE=$(shuf -i 100000-999999 -n 1)
                echo "╔════════════════════════════════════╗"
                echo "║         🔢 组网配对码               ║"
                echo "╠════════════════════════════════════╣"
                echo "║                                    ║"
                echo "║         $CODE                       ║"
                echo "║                                    ║"
                echo "╠════════════════════════════════════╣"
                echo "║  ⏱️  有效期: 5分钟                  ║"
                echo "║  📌 节点: $CIS_NODE_NAME            ║"
                echo "╚════════════════════════════════════╝"
                echo "$CODE" > /tmp/pairing-code.txt
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] Pairing code generated: $CODE" >> "$LOG_FILE"
                ;;
            join)
                CODE="$3"
                echo "🔍 正在使用组网码 $CODE 查找节点..."
                sleep 1
                echo "✅ 发现目标节点!"
                echo "✅ 组网成功!"
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] Joined network with code: $CODE" >> "$LOG_FILE"
                ;;
            *)
                echo "Usage: cis-node pair [generate|join]"
                ;;
        esac
        ;;
    neighbor)
        case "$2" in
            discover)
                echo "🔍 发现网络中的节点..."
                sleep 1
                echo "  cis-node1 (172.30.1.11) - coordinator"
                echo "  cis-node2 (172.30.1.12) - worker"
                echo "  cis-node3 (172.30.1.13) - worker"
                echo "[$(date '+%Y-%m-%d %H:%M:%S')] Neighbor discovery completed" >> "$LOG_FILE"
                ;;
            list)
                echo "邻居节点列表:"
                echo "  - node1 (172.30.1.11) - 在线"
                echo "  - node2 (172.30.1.12) - 在线"
                ;;
            *)
                echo "Usage: cis-node neighbor [discover|list]"
                ;;
        esac
        ;;
    status)
        echo "节点状态:"
        echo "  ID: $CIS_NODE_ID"
        echo "  Name: $CIS_NODE_NAME"
        echo "  Role: $CIS_NODE_ROLE"
        echo "  Status: 运行中"
        ;;
    --version|-v)
        echo "cis-node version 1.1.0 (demo)"
        ;;
    --help|-h)
        echo "CIS Node CLI (Demo Version)"
        echo ""
        echo "Commands:"
        echo "  daemon              启动守护进程"
        echo "  pair generate       生成组网码"
        echo "  pair join <code>    使用组网码加入"
        echo "  neighbor discover   发现邻居"
        echo "  neighbor list       列出邻居"
        echo "  status              显示状态"
        ;;
    *)
        echo "Unknown command: $1"
        echo "Use --help for usage"
        exit 1
        ;;
esac
