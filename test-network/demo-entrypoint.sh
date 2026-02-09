#!/bin/sh
mkdir -p /var/lib/cis/data /etc/cis /var/log/cis
echo "[$(date '+%Y-%m-%d %H:%M:%S')] CIS Node $CIS_NODE_NAME starting..."
exec cis-node daemon
