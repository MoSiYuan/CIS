# CIS 应急响应手册

**版本**: 1.0  
**日期**: 2026-02-08  
**状态**: 已审查

---

## 概述

本手册提供 CIS (Cluster of Independent Systems) 安全事件的响应流程、分类标准和处理指南。所有 CIS 用户和维护人员应熟悉本手册内容。

---

## 安全事件分类

### 事件严重等级

```
┌─────────────────────────────────────────────────────────────┐
│                    安全事件严重等级                          │
├──────────┬──────────────────────────────────────────────────┤
│  等级    │  定义                                            │
├──────────┼──────────────────────────────────────────────────┤
│ P0 紧急  │ • 私钥/助记词泄露                                 │
│ (红色)   │ • 节点被完全控制                                  │
│          │ • 大规模网络入侵                                  │
│          │ • 数据泄露确认                                    │
├──────────┼──────────────────────────────────────────────────┤
│ P1 高    │ • 未授权节点连接                                  │
│ (橙色)   │ • ACL 配置被篡改                                  │
│          │ • DDoS 攻击影响服务                               │
│          │ • 疑似中间人攻击                                  │
├──────────┼──────────────────────────────────────────────────┤
│ P2 中    │ • 多次失败的认证尝试                              │
│ (黄色)   │ • 异常的网络流量                                  │
│          │ • Skill 执行异常                                  │
│          │ • 配置漂移                                        │
├──────────┼──────────────────────────────────────────────────┤
│ P3 低    │ • 单个失败的认证                                  │
│ (蓝色)   │ • 正常的网络扫描                                  │
│          │ • 信息收集类事件                                  │
│          │ • 性能问题（非攻击）                              │
└──────────┴──────────────────────────────────────────────────┘
```

### 事件类型定义

| 类型 | 描述 | 示例 |
|------|------|------|
| **身份安全** | DID 相关事件 | 私钥泄露、助记词丢失、身份伪造 |
| **网络安全** | P2P 网络事件 | 未授权连接、MITM、DDoS、扫描 |
| **数据安全** | 存储相关事件 | 数据库泄露、加密破解、备份丢失 |
| **访问控制** | 权限相关事件 | ACL 篡改、权限提升、越权访问 |
| **应用安全** | Skill/服务事件 | 沙箱逃逸、代码执行、API 滥用 |
| **物理安全** | 硬件相关事件 | 设备丢失、硬件故障、物理入侵 |

---

## 响应流程

### 总体流程图

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   发现      │────►│   评估      │────►│   遏制      │
│  (Detect)   │     │  (Assess)   │     │  (Contain)  │
└─────────────┘     └──────┬──────┘     └──────┬──────┘
                           │                   │
                           ▼                   ▼
                    ┌─────────────┐     ┌─────────────┐
                    │  分类定级   │     │  取证分析   │
                    │  P0/P1/P2   │     │ (Preserve)  │
                    └─────────────┘     └──────┬──────┘
                                               │
                           ┌───────────────────┘
                           ▼
                    ┌─────────────┐     ┌─────────────┐
                    │   根除      │────►│   恢复      │
                    │ (Eradicate) │     │ (Recover)   │
                    └─────────────┘     └──────┬──────┘
                                               │
                                               ▼
                                        ┌─────────────┐
                                        │  事后总结   │
                                        │ (Post-      │
                                        │  Incident)  │
                                        └─────────────┘
```

### 详细响应步骤

#### 阶段 1: 发现 (Detect)

**触发源：**
- 自动化监控告警
- 用户报告
- 日志分析发现
- 外部通报

**立即行动：**
```bash
# 1. 记录发现时间
INCIDENT_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
echo "Incident detected at: $INCIDENT_TIME" >> ~/incident_$(date +%Y%m%d).log

# 2. 创建事件目录
mkdir -p ~/incident_$(date +%Y%m%d)/{logs,evidence,reports}

# 3. 保存当前状态快照
cis node status --json > ~/incident_$(date +%Y%m%d)/status.json
cis network status > ~/incident_$(date +%Y%m%d)/network.txt
```

#### 阶段 2: 评估 (Assess)

**评估清单：**
- [ ] 事件类型（身份/网络/数据/访问/应用/物理）
- [ ] 影响范围（单节点/多节点/全网）
- [ ] 数据是否泄露
- [ ] 攻击是否进行中
- [ ] 是否需要立即遏制

**定级命令：**
```bash
# 评估连接安全
cis p2p peers --suspicious

# 检查 ACL 变更历史
cis network acl-log --since "1 hour ago"

# 查看异常登录
cis log audit --event network.connect --level warning

# 检查节点完整性
cis node verify
```

#### 阶段 3: 遏制 (Contain)

##### P0 紧急事件遏制

```bash
# 1. 立即进入隔离模式
cis network mode solitary --reason "P0 Incident: $(date)"

# 2. 断开所有 P2P 连接
cis p2p disconnect --all --force

# 3. 停止所有 Skill
cis skill stop --all

# 4. 禁用外部访问（防火墙）
# Linux
sudo iptables -A INPUT -p tcp --dport 6767 -j DROP
sudo iptables -A INPUT -p tcp --dport 7676 -j DROP
sudo iptables -A INPUT -p udp --dport 7677 -j DROP

# 5. 备份当前状态
cis backup create --name "incident-$(date +%Y%m%d-%H%M%S)" --encrypt
```

##### P1 高优先级事件遏制

```bash
# 1. 隔离可疑节点
SUSPICIOUS_DID="did:cis:suspicious:node1"
cis network deny "$SUSPICIOUS_DID" \
    --reason "P1 Incident: $(date)" \
    --expires 7d

# 2. 断开特定连接
cis p2p disconnect "$SUSPICIOUS_DID"

# 3. 增加监控级别
cis log level set --module network --level debug

# 4. 启用详细审计
cis config set logging.audit.critical_only false
```

##### P2/P3 事件遏制

```bash
# 增加监控和记录
cis log level set --module network --level debug

# 设置告警
cis alert set --metric failed_auth --threshold 5
```

#### 阶段 4: 取证 (Preserve)

**证据收集清单：**

```bash
INCIDENT_DIR=~/incident_$(date +%Y%m%d)

# 1. 收集日志
cp -r ~/.cis/logs "$INCIDENT_DIR/logs/"
cis log export --format json --output "$INCIDENT_DIR/audit.json"

# 2. 收集网络状态
cis p2p status --verbose > "$INCIDENT_DIR/p2p_status.txt"
cis network status > "$INCIDENT_DIR/network_status.txt"
netstat -an > "$INCIDENT_DIR/netstat.txt" 2>/dev/null || ss -an > "$INCIDENT_DIR/ss.txt"

# 3. 收集配置
cp ~/.cis/config.toml "$INCIDENT_DIR/config.toml"
cp ~/.cis/network_acl.toml "$INCIDENT_DIR/network_acl.toml"

# 4. 收集节点信息
cis node info > "$INCIDENT_DIR/node_info.txt"
cis did show > "$INCIDENT_DIR/did.txt"

# 5. 计算哈希（完整性）
find "$INCIDENT_DIR" -type f -exec sha256sum {} \; > "$INCIDENT_DIR/integrity.sha256"

# 6. 压缩并加密
tar -czf "$INCIDENT_DIR.tar.gz" "$INCIDENT_DIR"
# 可选: 加密存储
#gpg --symmetric --cipher-algo AES256 "$INCIDENT_DIR.tar.gz"
```

**证据链维护：**
- 记录所有操作的时间戳
- 记录执行人身份
- 保持证据只读状态
- 安全存储证据文件

#### 阶段 5: 根除 (Eradicate)

##### 私钥泄露场景

```bash
# 1. 生成新密钥对
cis node rotate-key --force

# 2. 通知所有白名单节点移除旧 DID
# 手动联系或通过安全渠道

# 3. 更新所有配置中的 DID 引用
# 检查: ~/.cis/config.toml, 其他节点白名单

# 4. 验证新密钥
cis node info
cis did show

# 5. 重新建立信任关系
# 与其他节点重新交换 DID
```

##### ACL 污染场景

```bash
# 1. 停止 ACL 同步
cis config set network.acl.enable_sync false

# 2. 从备份恢复 ACL
cis network acl-restore --from-backup

# 3. 验证 ACL 完整性
cis network acl-verify

# 4. 手动清理恶意条目
cis network list blacklist --verify

# 5. 重新启用同步（谨慎）
cis config set network.acl.enable_sync true
```

##### 节点入侵场景

```bash
# 1. 完全隔离
cis network mode solitary
cis node stop

# 2. 备份数据
cis backup create --name "pre-wipe-$(date +%Y%m%d)"

# 3. 检查数据完整性
cis verify --deep

# 4. 如需要，完全重置
# WARNING: 这将删除所有本地数据！
# cis node reset --factory

# 5. 重新初始化
cis init
# 从助记词恢复身份
# 从备份恢复数据
```

#### 阶段 6: 恢复 (Recover)

**恢复检查清单：**

```bash
# 1. 系统完整性检查
cis node verify --full

# 2. 网络连接测试
cis p2p status
cis network status

# 3. 逐步恢复连接
# 先连接最信任的节点
cis p2p connect did:cis:trusted:node1 --test-only

# 4. 恢复 Skill
cis skill start --all

# 5. 恢复正常网络模式
cis network mode whitelist

# 6. 验证同步功能
cis p2p sync --test

# 7. 监控 24 小时
cis monitor --duration 24h
```

---

## 联系人列表

### 内部响应团队

| 角色 | 职责 | 联系方式 |
|------|------|----------|
| 事件指挥官 | 总体协调、决策 | incident-commander@cis.local |
| 技术专家 | 技术分析、根除 | tech-lead@cis.local |
| 通讯联络 | 内外沟通、报告 | comms@cis.local |
| 法务顾问 | 法律合规、披露 | legal@cis.local |

### 外部联系人

| 组织 | 场景 | 联系方式 |
|------|------|----------|
| CIS 安全团队 | 产品漏洞 | security@cis.dev |
| 上游依赖 | 依赖漏洞 | 见各项目 SECURITY.md |
| 执法机构 | 犯罪事件 | 当地网警/网络犯罪部门 |
| CERT | 大规模事件 | cert@cert.org.cn |

### 升级路径

```
本地处理 (1小时内)
    │
    ▼ 无法解决
团队响应 (4小时内)
    │
    ▼ 涉及产品漏洞
CIS 安全团队 (24小时内)
    │
    ▼ 大规模安全事件
外部 CERT/执法
```

---

## 取证指南

### 日志分析

#### 关键日志位置

| 日志文件 | 路径 | 内容 |
|----------|------|------|
| 主日志 | `~/.cis/logs/cis.log` | 应用日志 |
| 审计日志 | `~/.cis/logs/audit.log` | 安全事件 |
| P2P 日志 | `~/.cis/logs/p2p.log` | 网络通信 |
| Skill 日志 | `~/.cis/logs/skills/*.log` | Skill 执行 |

#### 分析命令

```bash
# 查找认证失败
grep -i "auth.*fail\|login.*fail\|challenge.*fail" ~/.cis/logs/audit.log

# 查找异常连接
grep -i "suspicious\|unauthorized\|blocked" ~/.cis/logs/p2p.log

# 时间线分析
cat ~/.cis/logs/audit.log | jq -r '.timestamp + " " + .event + " " + .level' | sort

# IP 分析
grep -oE '\b([0-9]{1,3}\.){3}[0-9]{1,3}\b' ~/.cis/logs/p2p.log | sort | uniq -c | sort -rn | head -20
```

#### 关键指标

| 指标 | 正常值 | 告警值 | 调查值 |
|------|--------|--------|--------|
| 认证失败率 | <1% | >5% | >20% |
| 新连接数/分 | <10 | >50 | >100 |
| ACL 变更/天 | <5 | >20 | >50 |
| 异常断开 | <5% | >10% | >30% |

### 网络取证

```bash
# 捕获网络流量（如正在进行攻击）
sudo tcpdump -i any -w incident_$(date +%Y%m%d).pcap port 6767 or port 7676 or port 7677

# 分析连接
ss -tan | grep -E '6767|7676|7677'
lsof -i :6767

# 检查防火墙日志
sudo tail -f /var/log/ufw.log  # Ubuntu/Debian
sudo tail -f /var/log/firewalld  # RHEL/CentOS
```

### 内存取证（高级）

```bash
# 创建内存转储（需要 root）
# Linux
sudo dd if=/proc/$(pgrep cis)/mem of=~/incident_$(date +%Y%m%d)/memory.dump bs=1M

# macOS
sudo vmmap $(pgrep cis) > ~/incident_$(date +%Y%m%d)/vmmap.txt
```

**注意**: 内存取证可能涉及隐私和法律问题，确保获得适当授权。

---

## 恢复步骤

### 从备份恢复

```bash
# 1. 列出可用备份
cis backup list

# 2. 验证备份完整性
cis backup verify --name "backup-name"

# 3. 恢复数据（选择性）
cis backup restore --name "backup-name" --selective \
    --include config \
    --include memory \
    --exclude p2p-state

# 4. 完全恢复
cis backup restore --name "backup-name" --full
```

### 从助记词恢复

```bash
# 1. 停止当前节点
cis node stop

# 2. 备份当前数据（如需要）
cp -r ~/.cis ~/.cis.backup.$(date +%Y%m%d)

# 3. 重新初始化
cis init --recover

# 4. 输入助记词
# 注意：这将创建新的 DID，旧节点需要重新信任

# 5. 从备份恢复数据（可选）
cis backup restore --name "pre-incident-backup"
```

### 验证恢复

```bash
# 1. 基本功能测试
cis node status
cis did show
cis network status

# 2. 连接测试
cis p2p ping did:cis:trusted:node1

# 3. 数据完整性检查
cis verify --deep

# 4. 功能测试
cis skill test --all
cis memory test
```

---

## 事后总结

### 总结报告模板

```markdown
# 安全事件总结报告

## 基本信息
- 事件编号: INC-2026-XXXX
- 发现时间: YYYY-MM-DD HH:MM:SS
- 解决时间: YYYY-MM-DD HH:MM:SS
- 持续时间: X 小时 Y 分钟
- 严重等级: P0/P1/P2/P3

## 事件概述
[简要描述发生了什么]

## 影响分析
- 影响节点: [列表]
- 数据泄露: 是/否，范围
- 服务中断: 是/否，时长
- 财务影响: [如有]

## 根本原因
[技术分析导致事件的根本原因]

## 响应行动
- 遏制措施: [列表]
- 根除措施: [列表]
- 恢复措施: [列表]

## 经验教训
- 做得好的: [列表]
- 需要改进的: [列表]

## 改进措施
- [ ] 措施 1 (负责人, 截止日期)
- [ ] 措施 2 (负责人, 截止日期)

## 证据附件
- [证据文件列表]
```

### 跟进行动

| 时间 | 行动 | 负责人 |
|------|------|--------|
| 事件后 24h | 初步总结报告 | 事件指挥官 |
| 事件后 72h | 详细技术分析 | 技术专家 |
| 事件后 1周 | 改进计划制定 | 团队 |
| 事件后 1月 | 改进措施实施 | 各负责人 |
| 事件后 3月 | 效果评估 | 安全团队 |

---

## 附录

### 快速参考卡片

```
┌────────────────────────────────────────────────────────────┐
│              CIS 应急响应快速参考                           │
├────────────────────────────────────────────────────────────┤
│                                                            │
│  立即隔离:                                                  │
│  cis network mode solitary                                 │
│                                                            │
│  断开所有连接:                                              │
│  cis p2p disconnect --all --force                          │
│                                                            │
│  创建备份:                                                  │
│  cis backup create --name "incident-$(date +%Y%m%d)"       │
│                                                            │
│  查看可疑节点:                                              │
│  cis p2p peers --suspicious                                │
│                                                            │
│  查看审计日志:                                              │
│  cis log audit --since "1 hour ago"                        │
│                                                            │
│  联系安全团队:                                              │
│  security@cis.dev                                          │
│                                                            │
└────────────────────────────────────────────────────────────┘
```

---

**文档维护**: CIS 安全团队  
**最后更新**: 2026-02-08
