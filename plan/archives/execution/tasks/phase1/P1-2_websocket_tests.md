# P1-2: WebSocket 测试修复

**优先级**: P0 (阻塞)  
**阶段**: Phase 1 - 稳定性加固  
**负责人**: Agent-B (Network/Rust-Core)  
**预估**: 2 天  
**依赖**: 无  

---

## 问题描述

WebSocket 服务器测试失败，影响 Matrix 联邦功能。

---

## 原子任务

### [ ] P1-2.1: 修复 `test_sync_response_handling`

**问题**: 异步响应超时

**解决方案**:
1. 增加 `tokio::time::timeout` 配置
2. 调整超时时间为合理的值 (5-10秒)
3. 确保 Mock 服务器正确响应

**文件**: `cis-core/src/matrix/websocket/server.rs`

**验收标准**:
```bash
cargo test -p cis-core --lib matrix::websocket::server::tests::test_sync_response_handling
```

---

### [ ] P1-2.2: 修复 `test_sync_request_handling`

**问题**: Mock 服务器端口冲突

**解决方案**:
1. 使用 `portpicker` crate 动态分配端口
2. 确保测试间端口不冲突
3. 测试完成后正确关闭端口

**代码参考**:
```rust
use portpicker::pick_unused_port;

#[test]
fn test_sync_request_handling() {
    let port = pick_unused_port().expect("No ports free");
    // 使用动态端口启动 Mock 服务器
}
```

**验收标准**:
```bash
cargo test -p cis-core --lib matrix::websocket::server::tests::test_sync_request_handling
```

---

### [ ] P1-2.3: 添加 WebSocket 重连测试

**场景**: 网络断开后自动重连

**实现**:
1. 模拟网络断开 (关闭连接)
2. 验证自动重连机制
3. 确保重连后状态恢复

**验收标准**:
```bash
# 3 次断连后仍能恢复同步
cargo test -p cis-core --lib matrix::websocket::tests::test_reconnect
```

---

## 输出物

- [ ] `fix/websocket-tests.patch`
- [ ] WebSocket 测试覆盖率报告
- [ ] `reports/P1-2-completion.md`

---

## 并行提示

**可并行**: 与 P1-1 (内存安全)、P1-3 (项目注册表) 同时执行
**依赖下游**: P1-4 (E2E测试) 依赖本任务完成
