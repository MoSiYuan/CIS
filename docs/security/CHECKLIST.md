# CIS 安全配置检查清单

**版本**: 1.0  
**日期**: 2026-02-08  
**适用**: CIS v1.0.0+

---

## 使用说明

- [ ] 未检查/不适用
- [x] 已检查/已完成
- [~] 部分完成/需关注

每个检查项包含：
- 检查内容
- 检查方法（命令或步骤）
- 期望结果
- 修复建议（如失败）

---

## 一、部署前检查项

### 1.1 系统环境检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 1.1.1 | 操作系统已更新 | `uname -a` + 包管理器 | 最新安全补丁 | 执行系统更新 |
| 1.1.2 | 防火墙已启用 | `sudo ufw status` / `sudo firewall-cmd --state` | active/running | 启用并配置防火墙 |
| 1.1.3 | 时区/时间正确 | `timedatectl status` | NTP 同步，时区正确 | 配置 NTP 和时区 |
| 1.1.4 | 磁盘加密（可选）| `cryptsetup status` | 根分区已加密 | 启用全盘加密 |

### 1.2 CIS 安装检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 1.2.1 | 从官方源安装 | `cis --version` + 验证签名 | 官方二进制 | 从官方渠道重新安装 |
| 1.2.2 | 二进制完整性 | `sha256sum $(which cis)` | 与发布哈希匹配 | 重新下载并验证 |
| 1.2.3 | 版本最新 | `cis --version` | >= v1.0.0 | 升级到最新版本 |

### 1.3 初始化配置检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 1.3.1 | 目录权限正确 | `ls -la ~/.cis` | drwx------ (700) | `chmod 700 ~/.cis` |
| 1.3.2 | 私钥文件权限 | `ls -la ~/.cis/node.key` | -rw------- (600) | `chmod 600 ~/.cis/node.key` |
| 1.3.3 | 配置文件权限 | `ls -la ~/.cis/config.toml` | -rw------- (600) | `chmod 600 ~/.cis/config.toml` |
| 1.3.4 | 助记词已备份 | 物理检查 | 纸质备份在安全位置 | 立即创建离线备份 |
| 1.3.5 | API 密钥使用环境变量 | `grep api_key ~/.cis/config.toml` | 无硬编码密钥 | 改为 `${ENV_VAR}` 格式 |

### 1.4 网络配置检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 1.4.1 | 非开放模式 | `cis network status` | Whitelist/Solitary | `cis network mode whitelist` |
| 1.4.2 | 防火墙限制端口 | `sudo ufw status` / `sudo iptables -L` | 仅开放必要端口 | 配置防火墙规则 |
| 1.4.3 | 白名单非空（如使用）| `cis network list whitelist` | 至少一个可信节点 | 添加可信节点 |
| 1.4.4 | 无默认/测试凭据 | `grep -i "password\|secret" ~/.cis/config.toml` | 无硬编码密码 | 移除或加密存储 |

### 1.5 加密配置检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 1.5.1 | 数据库加密启用 | `cis config get storage.encryption.algorithm` | ChaCha20-Poly1305 | 启用加密 |
| 1.5.2 | KDF 参数强度 | `grep -A5 "kdf" ~/.cis/config.toml` | Argon2id, memory>=65536 | 调整 KDF 参数 |
| 1.5.3 | 传输加密强制 | `grep "require_encryption" ~/.cis/config.toml` | true | 设置为 true |
| 1.5.4 | TLS 版本 | `grep "min_version" ~/.cis/config.toml` | "1.3" | 强制 TLS 1.3 |

---

## 二、定期审计项

### 2.1 每周检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 2.1.1 | 节点运行正常 | `cis node status` | healthy, running | 检查日志并重启 |
| 2.1.2 | P2P 连接正常 | `cis p2p status` | 预期节点在线 | 检查网络连接 |
| 2.1.3 | 无异常断开 | `cis log audit --since "7 days" --event network.disconnect` | 无异常模式 | 调查异常原因 |
| 2.1.4 | 磁盘空间充足 | `df -h ~/.cis` | >20% 可用 | 清理日志或扩容 |
| 2.1.5 | 日志轮转正常 | `ls -la ~/.cis/logs/` | 无过度增长的日志 | 配置日志轮转 |

### 2.2 每月检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 2.2.1 | 认证失败分析 | `cis log audit --since "30 days" --event auth.failure` | 失败率 <1% | 调查异常来源 |
| 2.2.2 | 密钥轮换检查 | `cis node info` | 密钥年龄 <90 天 | 执行密钥轮换 |
| 2.2.3 | ACL 审查 | `cis network list whitelist` + `cis network list blacklist` | 无过期/可疑条目 | 清理和更新 ACL |
| 2.2.4 | 软件版本 | `cis --version` + 检查发布页面 | 最新稳定版 | 计划升级 |
| 2.2.5 | 备份验证 | `cis backup list` + `cis backup verify --latest` | 备份完整可恢复 | 修复备份流程 |
| 2.2.6 | 审计日志审查 | `cis log audit --since "30 days" --level warning` | 无未处理告警 | 处理所有告警 |
| 2.2.7 | 访问规则审查 | `cis network rules list` | 规则仍然适用 | 更新或删除过时规则 |

### 2.3 每季度检查

| # | 检查项 | 方法 | 期望 | 修复 |
|---|--------|------|------|------|
| 2.3.1 | 完整安全审计 | 渗透测试/代码审查 | 无高危漏洞 | 修复发现的问题 |
| 2.3.2 | 助记词备份验证 | 物理检查备份 | 可读且完整 | 重新备份如需要 |
| 2.3.3 | 灾难恢复演练 | 模拟节点故障 | 成功恢复 | 更新恢复流程 |
| 2.3.4 | 信任关系审查 | 审查所有白名单节点 | 所有节点仍可信 | 移除不再可信的节点 |
| 2.3.5 | 配置安全审查 | `cis config audit` | 无配置漂移 | 标准化配置 |
| 2.3.6 | 依赖安全扫描 | `cargo audit` (如从源码构建) | 无已知漏洞 | 更新依赖 |

---

## 三、合规检查项

### 3.1 数据保护合规

| # | 检查项 | 标准 | 方法 | 期望 |
|---|--------|------|------|------|
| 3.1.1 | 数据加密存储 | GDPR/等保 | `cis config get storage.encryption.enabled` | true |
| 3.1.2 | 传输加密 | GDPR/等保 | `cis config get network.tls.min_version` | "1.3" |
| 3.1.3 | 访问日志保留 | 等保 2.0 | `cis config get logging.audit.retention_days` | >= 180 |
| 3.1.4 | 数据最小化 | GDPR | 审查存储的记忆内容 | 仅存储必要数据 |
| 3.1.5 | 用户同意记录 | GDPR | 审查 Skill 权限 | 有明确的权限授予 |

### 3.2 访问控制合规

| # | 检查项 | 标准 | 方法 | 期望 |
|---|--------|------|------|------|
| 3.2.1 | 最小权限原则 | 等保 2.0 | `cis network list whitelist` | 仅必要的节点 |
| 3.2.2 | 定期权限审查 | SOX/等保 | 季度审查记录 | 有审查记录 |
| 3.2.3 | 特权访问监控 | 等保 2.0 | `cis log audit --event config.change` | 完整记录 |
| 3.2.4 | 账户生命周期 | 等保 2.0 | 检查过期 ACL 条目 | 及时清理 |

### 3.3 审计合规

| # | 检查项 | 标准 | 方法 | 期望 |
|---|--------|------|------|------|
| 3.3.1 | 审计日志完整性 | 等保 2.0 | `cis verify --audit-logs` | 通过验证 |
| 3.3.2 | 时间同步 | 等保 2.0 | `timedatectl status` | NTP 同步 |
| 3.3.3 | 安全事件响应 | ISO 27001 | 检查事件响应计划 | 文档化且测试过 |
| 3.3.4 | 备份加密 | 等保 2.0 | `cis backup verify --check-encryption` | 备份已加密 |

### 3.4 合规报告生成

```bash
# 生成合规报告
cis security compliance-report \
    --standard gdpr \
    --output compliance_gdpr_$(date +%Y%m%d).pdf

cis security compliance-report \
    --standard dengbao \
    --output compliance_db_$(date +%Y%m%d).pdf
```

---

## 四、快速检查脚本

### 自动化检查脚本

```bash
#!/bin/bash
# CIS Security Check Script
# 保存为: cis-security-check.sh
# 运行: bash cis-security-check.sh

echo "=== CIS Security Check ==="
echo "Date: $(date)"
echo ""

PASS=0
FAIL=0
WARN=0

check() {
    local name="$1"
    local cmd="$2"
    local expected="$3"
    
    echo -n "[CHECK] $name... "
    if eval "$cmd" > /dev/null 2>&1; then
        echo "✅ PASS"
        ((PASS++))
    else
        echo "❌ FAIL (expected: $expected)"
        ((FAIL++))
    fi
}

# 1. 目录权限
check "Data directory permissions" \
    "stat -c '%a' ~/.cis | grep -q '700'" \
    "700"

# 2. 私钥文件权限
check "Private key permissions" \
    "stat -c '%a' ~/.cis/node.key 2>/dev/null | grep -q '600'" \
    "600"

# 3. 网络模式
check "Network mode not open" \
    "cis network status 2>/dev/null | grep -qv 'Open'" \
    "Whitelist or Solitary"

# 4. 加密启用
check "Database encryption" \
    "grep -q 'encryption' ~/.cis/config.toml 2>/dev/null" \
    "encryption configured"

# 5. 审计日志
check "Audit logging enabled" \
    "grep -q 'audit.*enabled.*true' ~/.cis/config.toml 2>/dev/null" \
    "audit enabled"

# 6. 版本检查
VERSION=$(cis --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [[ "$VERSION" =~ ^1\.[0-9]+\.[0-9]+$ ]]; then
    echo "[CHECK] CIS version... ✅ PASS ($VERSION)"
    ((PASS++))
else
    echo "[CHECK] CIS version... ❌ FAIL (expected: >= 1.0.0)"
    ((FAIL++))
fi

echo ""
echo "=== Summary ==="
echo "✅ Passed: $PASS"
echo "❌ Failed: $FAIL"
echo "⚠️  Warnings: $WARN"
echo ""

if [ $FAIL -eq 0 ]; then
    echo "🎉 All checks passed!"
    exit 0
else
    echo "⚠️  Some checks failed. Please review and fix."
    exit 1
fi
```

---

## 五、检查记录模板

### 部署前检查记录

```
检查日期: ___________
执行人员: ___________
节点 DID: ___________
CIS 版本: ___________

检查项                          状态    备注
────────────────────────────────────────────────
□ 1.1 系统环境检查              [ ]     
□ 1.2 CIS 安装检查              [ ]     
□ 1.3 初始化配置检查            [ ]     
□ 1.4 网络配置检查              [ ]     
□ 1.5 加密配置检查              [ ]     

整体评估: [ ] 通过  [ ] 有条件通过  [ ] 未通过

签名: ___________    日期: ___________
```

### 定期审计记录

```
审计周期: □ 周度  □ 月度  □ 季度
审计日期: ___________
审计人员: ___________

检查结果:
────────────────────────────────────────────────
检查项                          状态    行动项
────────────────────────────────────────────────
□ 节点运行状态                  [ ]     
□ P2P 连接状态                  [ ]     
□ 认证失败分析                  [ ]     
□ 密钥轮换状态                  [ ]     
□ ACL 审查                      [ ]     
□ 软件版本                      [ ]     
□ 备份验证                      [ ]     
□ 审计日志审查                  [ ]     

发现的问题:
1. 
2. 
3. 

计划行动:
1. 
2. 

下次审计日期: ___________

签名: ___________
```

---

## 六、参考文档

- [威胁模型分析](./THREAT_MODEL.md)
- [安全指南](./SECURITY_GUIDE.md)
- [应急响应手册](./INCIDENT_RESPONSE.md)
- [项目安全策略](../../SECURITY.md)

---

**文档维护**: CIS 安全团队  
**最后更新**: 2026-02-08
