# CIS v1.1.6 集成测试和性能测试计划

> **创建日期**: 2026-02-13
> **目标**: 验证所有新模块的集成和性能
> **状态**: 执行中

---

## 测试范围

### 1. 核心模块测试

| 模块 | 测试类型 | 覆盖率目标 | 优先级 |
|------|----------|-------------|--------|
| Task Repository | 单元测试 | >90% | P0 |
| Session Repository | 单元测试 | >90% | P0 |
| DAG Builder | 单元测试 | >90% | P0 |
| Task Manager | 集成测试 | >85% | P0 |
| Task Migrator | 单元测试 | >85% | P1 |
| Weekly Memory | 单元测试 | >85% | P1 |

### 2. 集成测试场景

#### 2.1 端到端任务流程

```test
1. 创建 TOML 任务定义
2. 运行迁移工具
3. 验证任务已正确导入数据库
4. 使用 TaskManager 查询任务
5. 构建执行计划
6. 分配任务到 Teams
7. 执行任务编排
```

**预期结果**:
- ✅ 所有任务正确迁移
- ✅ DAG依赖关系正确
- ✅ Team分配符合预期

#### 2.2 Session 复用测试

```test
1. 创建 Agent Session
2. 分配任务到 Session
3. 完成任务
4. 释放 Session
5. 重新获取同一 Session
6. 验证状态正确
```

**预期结果**:
- ✅ Session 正确创建和复用
- ✅ 状态正确转换（active → idle → active）
- ✅ Session 统计准确

#### 2.3 记忆系统集成测试

```test
1. 创建新记忆
2. 验证向量索引
3. 执行语义搜索
4. 触发周归档
5. 验证旧记忆清理
```

**预期结果**:
- ✅ 记忆正确存储和检索
- ✅ 向量索引正常工作
- ✅ 周归档自动执行

### 3. 性能测试

#### 3.1 数据库性能

```test
// 批量插入性能
let start = Instant::now();
for i in 0..1000 {
    repository.create(&task).await?;
}
let duration = start.elapsed();

// 目标: <100ms per task
```

**基准目标**:
| 操作 | 目标 | 测量方法 |
|------|------|----------|
| 任务创建 | <50ms | 单次插入 |
| 批量创建 | <10ms/task | 批量插入 |
| 任务查询 | <100ms | 简单查询 |
| 复杂查询 | <500ms | 带过滤 |
| DAG构建 | <200ms | 100节点 |

#### 3.2 并发性能

```test
// 并发 Session 管理
let handles: Vec<_> = (0..100).map(|_| {
    tokio::spawn(async {
        session_repository.create_session(...).await
    })
}).collect();

for h in handles {
    h.await?;
}
```

**基准目标**:
- ✅ 无死锁
- ✅ 无数据竞争
- ✅ 内存占用稳定

#### 3.3 内存测试

```test
// 长时间运行稳定性
for hour in 0..24 {
    // 执行各种操作
    // 记录内存使用
    let usage = get_memory_usage();
    assert!(usage < MAX_MEMORY);
}
```

**基准目标**:
| 场景 | 内存上限 |
|------|---------|
| 空闲 | <50MB |
| 1000 任务 | <100MB |
| 复杂 DAG (1000节点) | <150MB |
| 并发50 Sessions | <200MB |

### 4. 压力测试

#### 4.1 数据库并发

```test
// 模拟多进程/线程并发写入
for thread in 0..10 {
    thread::spawn(|| {
        let repository = TaskRepository::new(...);
        // 执行写入操作
    });
}
```

**验证**:
- ✅ 无数据损坏
- ✅ 无死锁
- ✅ 事务正确回滚

#### 4.2 大量数据

```test
// 插入 10000 个任务
for i in 0..10000 {
    repository.create(&large_task).await?;
}

// 执行复杂查询
let results = repository.query(TaskFilter {
    dependencies: true,
    recursive: true,
}).await?;
```

**验证**:
- ✅ 查询时间线性增长
- ✅ 内存可控
- ✅ 无文件描述符泄漏

---

## 测试实现计划

### Phase 1: 单元测试补充 (1-2天)

**优先级 P0**:
- [ ] TaskRepository 完整测试覆盖
- [ ] SessionRepository 完整测试覆盖
- [ ] DagBuilder 边界测试
- [ ] TaskMigrator 测试补充

### Phase 2: 集成测试编写 (1-2天)

**优先级 P0**:
- [ ] 端到端迁移流程测试
- [ ] TaskManager 集成测试
- [ ] Session 复用流程测试

### Phase 3: 性能基准建立 (1天)

**优先级 P1**:
- [ ] 数据库操作基准
- [ ] DAG构建基准
- [ ] 并发性能基准

### Phase 4: 压力测试 (1天)

**优先级 P2**:
- [ ] 并发写入测试
- [ ] 大数据量测试
- [ ] 长时间稳定性测试

---

## 验收标准

### 代码质量
- [ ] 所有新模块测试覆盖率 >85%
- [ ] 核心模块测试覆盖率 >90%
- [ ] 无 clippy 警告
- [ ] 无 unsafe 代码（除非必要且文档化）

### 功能完整性
- [ ] 所有测试用例通过
- [ ] 边界条件覆盖
- [ ] 错误路径测试完整

### 性能
- [ ] 所有基准达标
- [ ] 压力测试无崩溃
- [ ] 内存泄漏检测通过

### 文档
- [ ] 测试文档完整
- [ ] 运行测试指南清晰
- [ ] CI配置就绪

---

## 输出

测试完成后将生成：

1. **测试报告**: `docs/plan/v1.1.6/TEST_REPORT.md`
2. **性能基准**: `docs/plan/v1.1.6/BENCHMARKS.md`
3. **覆盖率报告**: `coverage/` 目录

---

**文档版本**: 1.0
**创建日期**: 2026-02-13
**作者**: CIS Team
