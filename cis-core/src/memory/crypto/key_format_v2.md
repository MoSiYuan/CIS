# 加密密钥存储格式 v2.0

## 概述

本文档定义了 CIS v2 加密密钥的存储格式。v2 格式修复了 v1 格式中固定盐值的安全问题，为每个密钥派生生成唯一的盐值。

## 设计目标

1. **安全性**: 每个密钥使用唯一的盐值
2. **兼容性**: 保持与 v1 格式的清晰区分
3. **可迁移性**: 支持从 v1 格式迁移到 v2 格式
4. **可扩展性**: 预留未来扩展空间

## v2 格式定义

### 二进制格式

```
+-------------------+-------------------+-------------------+
| Magic (4 bytes)   | Version (1 byte)  | Salt Length (2B)  |
+-------------------+-------------------+-------------------+
| Salt (variable)    | Key Length (2B)    | Key (variable)    |
+-------------------+-------------------+-------------------+
| Reserved (8 bytes) | HMAC (32 bytes)    |
+-------------------+-------------------+
```

### 字段说明

- **Magic**: `0x43495332` ("CIS2" in ASCII) - 用于识别 v2 格式
- **Version**: 格式版本号，当前为 `0x02`
- **Salt Length**: 盐值长度（通常 32 字节）
- **Salt**: 随机生成的盐值，用于 Argon2id 密钥派生
- **Key Length**: 派生密钥长度（通常 32 字节）
- **Key**: 使用 Argon2id 派生的加密密钥
- **Reserved**: 保留字段，用于未来扩展
- **HMAC**: 使用主密钥计算的整个记录的 HMAC-SHA256，用于完整性验证

### 存储格式

密钥文件存储为 JSON 格式，包含编码后的二进制数据：

```json
{
  "format": "cis-key-v2",
  "version": 2,
  "created_at": "2026-02-12T10:30:00Z",
  "algorithm": "argon2id",
  "algorithm_params": {
    "iterations": 4096,
    "parallelism": 3,
    "memory": 65536,
    "output_length": 32
  },
  "encoding": "base64",
  "data": "<base64-encoded binary structure>"
}
```

## 密钥派生流程

### 1. 生成盐值

```rust
let mut salt = [0u8; 32];
rng.fill_bytes(&mut salt);
```

### 2. 派生密钥

使用 Argon2id 算法：

```rust
let params = Params::new(4096, 3, 1);  // 高安全参数
let argon = Argon2::new(
    Algorithm::Argon2id,
    Version::Version13,
    params,
);

let mut context = node_key.to_vec();
context.extend_from_slice(unique_id);
context.extend_from_slice(b"cis-memory-v2");

argon.hash_password_into(
    &context,
    &salt,
    &mut key,
)?;
```

### 3. 序列化

将所有字段序列化为二进制格式并计算 HMAC。

## v1 vs v2 对比

| 特性 | v1 格式 | v2 格式 |
|-----|---------|---------|
| 盐值 | 固定盐值 `"cis-memory-encryption"` | 每个密钥唯一盐值 |
| 算法 | SHA256 | Argon2id |
| 密钥长度 | 32 字节 | 32 字节（可配置） |
| 格式识别 | Magic `0x43495331` | Magic `0x43495332` |
| 完整性保护 | 无 | HMAC-SHA256 |
| 参数存储 | 硬编码 | JSON 元数据 |

## 迁移策略

### v1 到 v2 迁移

1. **检测**: 读取密钥文件，检查 magic 字节
2. **解密**: 使用 v1 密钥解密数据
3. **派生**: 使用 Argon2id 和新盐值派生 v2 密钥
4. **重新加密**: 使用 v2 密钥重新加密数据
5. **备份**: 保留 v1 密钥文件作为备份

### 迁移工具

```bash
# 自动迁移
cis migrate-keys --from v1 --to v2

# 批量迁移
cis migrate-keys --batch --backup ~/.cis/keys/v1_backup/
```

## 安全考虑

### 盐值管理

- **唯一性**: 每个密钥必须使用不同的盐值
- **长度**: 建议使用 32 字节（256 位）
- **随机性**: 使用加密安全的随机数生成器

### Argon2id 参数

- **时间成本 (t)**: 4096 次迭代
- **并行度 (p)**: 3 个通道
- **内存成本 (m)**: 65536 KB (64 MB)
- **输出长度**: 32 字节

这些参数在 2026 年提供约 100ms 的派生时间，平衡了安全性和性能。

### HMAC 密钥

HMAC 使用主密钥（从节点密钥派生）计算：

```rust
let hmac_key = derive_hmac_key(node_key, b"cis-key-hmac");
let hmac = HmacSha256::sign(&hmac_key, &data_without_hmac);
```

## 实现文件

- `cis-core/src/memory/crypto/key_format_v2.md` - 本文档
- `cis-core/src/memory/encryption_v2.rs` - v2 加密实现
- `tools/migrate_keys.rs` - 迁移工具

## 示例代码

### 生成 v2 密钥

```rust
use cis_core::memory::encryption_v2::EncryptionKeyV2;

let key = EncryptionKeyV2::from_node_key_v2(
    node_key,
    unique_id,
)?;

// 存储到文件
key.save("~/.cis/keys/encryption_key_v2.json")?;
```

### 加载 v2 密钥

```rust
let key = EncryptionKeyV2::load("~/.cis/keys/encryption_key_v2.json")?;

// 创建加密器
let cipher = MemoryEncryptionV2::from_key(&key);
```

## 参考资料

- [Argon2 RFC 9106](https://datatracker.ietf.org/doc/html/rfc9106)
- [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)
- [NIST SP 800-63B](https://pages.nist.gov/800-63-3/sp800-63b.html)
