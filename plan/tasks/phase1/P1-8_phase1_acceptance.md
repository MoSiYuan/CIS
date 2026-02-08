# P1-8: Phase 1 验收

**优先级**: P0 (阻塞)  
**阶段**: Phase 1 - 稳定性加固  
**负责人**: Agent-Lead  
**预估**: 1 天  
**依赖**: P1-1 ~ P1-7 全部完成  

---

## 验收标准

### 稳定性基线

```yaml
测试:
  单元测试通过率: 100%
  集成测试通过率: 100%
  E2E 测试通过率: > 95%
  代码覆盖率: > 80%

代码质量:
  编译警告: 0
  Clippy 警告: < 10
  安全漏洞: 0 高危

性能:
  DAG 执行 (空): < 100ms
  向量检索 (1k): < 50ms
  Skill 调用: < 200ms
  内存使用 (空闲): < 100MB
```

---

## 验收检查清单

### [ ] 1. 运行所有测试

```bash
cd /Users/jiangxiaolong/work/project/CIS

# 单元测试
cargo test -p cis-core --lib 2>&1 | tee /tmp/test-lib.log

# 检查通过率
if grep -q "test result: ok" /tmp/test-lib.log; then
    echo "✅ 单元测试通过"
else
    echo "❌ 单元测试失败"
    exit 1
fi

# 覆盖率检查
cargo tarpaulin -p cis-core --out stdout 2>&1 | grep "Coverage"
```

### [ ] 2. 检查编译警告

```bash
cargo build -p cis-core 2>&1 | grep "warning:" | wc -l
# 预期: 0

cargo clippy -p cis-core 2>&1 | grep "warning:" | wc -l
# 预期: < 10
```

### [ ] 3. 安全扫描

```bash
# 依赖漏洞扫描
cargo audit

# 许可证检查
cargo deny check
```

### [ ] 4. 性能基准

```bash
# 运行基准测试
cargo bench -p cis-core

# 检查结果
cat target/criterion/report/index.html
```

### [ ] 5. E2E 测试

```bash
# 启动 CIS
cis init

# 测试基本流程
cis skill list
cis dag status

# 验证无崩溃
echo "✅ E2E 测试通过"
```

---

## 输出物

- [ ] `reports/phase1-test-report.md` - 测试报告
- [ ] `reports/phase1-coverage-report.html` - 覆盖率报告
- [ ] `reports/phase1-security-report.md` - 安全扫描报告
- [ ] `reports/phase1-performance-report.md` - 性能基准报告
- [ ] Phase 1 完成标记 (更新到项目看板)

---

## 通过标准

所有检查项必须 ✅ 才能进入 Phase 2:

```markdown
- [x] 单元测试 100% 通过
- [x] 代码覆盖率 > 80%
- [x] 编译警告 0
- [x] 安全扫描无高危漏洞
- [x] 性能基准达标
- [x] E2E 测试通过
```

---

## 失败处理

如有检查项失败:
1. 记录失败项到 `reports/phase1-failures.md`
2. 分配修复任务到相关 Agent
3. 修复后重新验收

---

## 下一步

Phase 1 验收通过后:
- 通知所有 Agent 开始 Phase 2 任务
- 发布 Phase 1 完成公告
- 合并所有 Phase 1 分支到 main
