# T-P1.1: Matrix CORS 配置

**优先级**: 🟡 P1  
**预估时间**: 2h  
**依赖**: -  
**分配**: Agent-C

---

## 问题描述

Matrix 服务器使用 `Any` 开放所有 origin，生产环境不安全。

**问题文件**:
- `cis-core/src/matrix/server.rs:70`
- `cis-core/src/matrix/federation/server.rs:195`

**当前代码**:
```rust
.allow_origin(Any)  // TODO: Configure specific origins for production
```

---

## 修复方案

### 1. 添加 CORS 配置到 MatrixConfig

```rust
#[derive(Debug, Clone)]
pub struct MatrixConfig {
    // ... existing fields
    pub allowed_origins: Vec<String>,
}
```

### 2. 修改 server.rs

```rust
let allowed_origins = config.allowed_origins.clone();
let cors = if allowed_origins.is_empty() {
    CorsLayer::new().allow_origin(Any)
} else {
    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .map(|o| o.parse().unwrap())
        .collect();
    CorsLayer::new().allow_origin(origins)
};
```

---

## 验收标准

- [ ] 支持配置允许的 origin 列表
- [ ] 生产环境文档说明配置方法
- [ ] 默认配置安全
