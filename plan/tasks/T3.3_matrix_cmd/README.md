# T3.3: matrix start/stop/status 命令

**任务编号**: T3.3  
**任务名称**: Matrix CLI Commands  
**优先级**: P1  
**预估时间**: 4h  
**依赖**: T2.2 (Matrix Server Manager)  
**分配状态**: 待分配

---

## 任务概述

替换 Matrix 命令的模拟实现，使用真实的生命周期管理。

---

## 输入

### 依赖任务输出
- **T2.2**: `MatrixServerManager`

### 待修改文件
- `cis-node/src/commands/matrix.rs`

---

## 输出要求

替换以下函数：
- `start_matrix_server()` - 使用真实启动
- `stop_matrix_server()` - 使用真实停止
- `show_matrix_status()` - 显示真实状态

---

## 验收标准

- [ ] start 启动真实进程
- [ ] stop 终止进程
- [ ] status 显示 PID、端口、运行时间
- [ ] 端口冲突时明确报错

---

## 阻塞关系

**依赖**:
- T2.2: MatrixServerManager
