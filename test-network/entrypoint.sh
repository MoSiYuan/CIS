#!/bin/sh
# CIS 容器入口脚本

cat > /etc/cis/config.toml << CFG
[node]
id = "${CIS_NODE_ID}"
name = "${CIS_NODE_NAME}"
did = "${CIS_DID}"
role = "${CIS_NODE_ROLE:-worker}"

[ai.glm]
model = "${GLM_MODEL:-code-plan-glm4.7}"
base_url = "https://api.glm.ai/v1"

[network]
discovery_port = 6767
pairing_port = 6768
CFG

exec cis-node daemon --config /etc/cis/config.toml
