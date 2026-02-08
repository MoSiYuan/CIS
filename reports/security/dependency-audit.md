# CIS 项目依赖安全审计报告

**生成日期**: 2026-02-08  
**审计工具**: cargo-audit  
**项目**: CIS (Cluster of Independent Systems)  

---

## 执行摘要

本次审计扫描了 **916 个 crate 依赖项**，发现：

| 类别 | 数量 | 状态 |
|------|------|------|
| **关键漏洞** | 0 | ✅ 已修复 |
| **维护警告** | 12 | ⚠️ 非直接安全风险 |

### 修复成果

通过修改 `cis-core/Cargo.toml` 中的 sqlx 配置，成功移除了 **RUSTSEC-2023-0071** (Marvin Attack) 关键漏洞：

- **移除的依赖**: `rsa`, `sqlx-mysql`, `sqlx-postgres`, `hkdf`, `hmac`, `num-bigint-dig`, `num-iter`, `pkcs1`, `stringprep`, `unicode-*`
- **依赖减少**: 916 → 903 个 crate (-13)

---

## 漏洞修复详情

### RUSTSEC-2023-0071 - 已修复 ✅

| 属性 | 详情 |
|------|------|
| **ID** | RUSTSEC-2023-0071 |
| **包名** | rsa |
| **版本** | 0.9.10 |
| **标题** | Marvin Attack: potential key recovery through timing sidechannels |
| **严重程度** | 5.9 (中等) |
| **发现日期** | 2023-11-22 |
| **CVE** | CVE-2023-49092 |

#### 漏洞描述

`rsa` crate 的实现不是恒定时间的，导致私钥信息通过网络可观测的定时信息泄露。攻击者可能利用这些信息恢复密钥。

#### 修复方案

**修改文件**: `cis-core/Cargo.toml`

```toml
# 修改前
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"], optional = true }

# 修改后
sqlx = { version = "0.8", default-features = false, features = ["sqlite", "runtime-tokio", "macros"], optional = true }
```

**原理**: 
- `sqlx` 默认启用了 `any` 特性，该特性会引入 `sqlx-mysql` 和 `sqlx-postgres`
- `sqlx-mysql` 依赖有漏洞的 `rsa` crate 用于 MySQL 认证
- 由于 CIS 项目仅使用 SQLite 数据库，禁用默认特性并只启用 SQLite 相关特性即可避免引入有漏洞的依赖

---

## 维护警告 (非直接安全风险)

以下 crate 已被标记为不再维护，但不构成直接安全漏洞：

### 1. atomic-polyfill (RUSTSEC-2023-0089)
- **版本**: 1.0.3
- **影响**: 通过 `heapless` → `serde-json-core` → `cis-skill-sdk`
- **建议**: 监控上游更新，考虑迁移到 `portable-atomic`

### 2. atty (RUSTSEC-2024-0375, RUSTSEC-2021-0145)
- **版本**: 0.2.14
- **影响**: 直接使用于 `cis-core`
- **建议**: 使用标准库 `std::io::IsTerminal` 替代 (Rust 1.70+)

### 3. bincode (RUSTSEC-2025-0141)
- **版本**: 1.3.3
- **影响**: 直接使用于 `cis-core`
- **建议**: 升级到 bincode 2.0 (API 不兼容，需代码修改)

### 4. derivative (RUSTSEC-2024-0388)
- **版本**: 2.2.0
- **影响**: 通过 `wasmer` 引入
- **建议**: 监控 wasmer 更新

### 5. fxhash (RUSTSEC-2025-0057)
- **版本**: 0.2.1
- **影响**: 通过 `cranelift-codegen` → `wasmer` 引入
- **建议**: 监控 wasmer/cranelift 更新

### 6. number_prefix (RUSTSEC-2025-0119)
- **版本**: 0.4.0
- **影响**: 通过 `indicatif` → `tokenizers` 引入
- **建议**: 监控上游更新

### 7. paste (RUSTSEC-2024-0436)
- **版本**: 1.0.15
- **影响**: 通过 `tokenizers`, `rmcp`, `metal`, `accesskit_windows` 引入
- **建议**: 监控上游更新

### 8. proc-macro-error (RUSTSEC-2024-0370)
- **版本**: 1.0.4
- **影响**: 通过 `wasmer-derive` → `wasmer` 引入
- **建议**: 监控 wasmer 更新

### 9. rustls-pemfile (RUSTSEC-2025-0134)
- **版本**: 1.0.4
- **影响**: 通过 `rustls-native-certs` 和 `reqwest` 引入
- **建议**: 升级到 rustls-pemfile 2.0

### 10. serial (RUSTSEC-2017-0008)
- **版本**: 0.4.0
- **影响**: 通过 `portable-pty` 引入
- **建议**: 监控 portable-pty 更新

### 11. git2 (RUSTSEC-2026-0008)
- **版本**: 0.18.3
- **类型**: unsound
- **影响**: 直接使用于 `cis-core` 和 `cis-capability`
- **建议**: 升级到 0.19.x

---

## 建议后续行动

### 高优先级
1. **✅ 已完成**: 移除 rsa 漏洞依赖

### 中优先级
2. **更新 git2**: `0.18.3` → `0.19.x` (修复 RUSTSEC-2026-0008)
3. **替换 atty**: 使用标准库替代 (Rust 1.70+)

### 低优先级
4. **监控维护警告**: 定期检查上游 crate 更新
5. **评估 bincode 升级**: bincode 2.0 有重大 API 变更
6. **考虑 rustls-pemfile 升级**: 1.x → 2.x

---

## 审计命令参考

```bash
# 运行依赖审计
cargo audit

# 以 JSON 格式输出
cargo audit --json

# 更新依赖
cargo update

# 查看依赖树
cargo tree -i <crate-name>
```

---

## 附录

### 修改记录

| 时间 | 文件 | 修改内容 |
|------|------|----------|
| 2026-02-08 | cis-core/Cargo.toml | 禁用 sqlx 默认特性，移除 MySQL/Postgres 支持 |

### 依赖变化

```
移除的依赖:
- rsa 0.9.10 (有漏洞)
- sqlx-mysql 0.8.6
- sqlx-postgres 0.8.6
- hkdf 0.12.4
- hmac 0.12.1
- num-bigint-dig 0.8.6
- num-iter 0.1.45
- pkcs1 0.7.5
- stringprep 0.1.5
- unicode-bidi 0.3.18
- unicode-normalization 0.1.25
- unicode-properties 0.1.4
- etcetera 0.8.0

总依赖数: 916 → 903 (-13)
```

---

**报告生成**: Kimi Code CLI  
**审计标准**: RustSec Advisory Database
