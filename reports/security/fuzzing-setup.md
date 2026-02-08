# CIS Core Fuzzing Setup Report

## 概述

本文档描述了为 CIS 核心组件添加的模糊测试 (fuzzing) 基础设施。

## 安装 cargo-fuzz

```bash
cargo install cargo-fuzz
```

cargo-fuzz 使用 libFuzzer 作为后端，用于自动生成测试输入以发现崩溃和安全漏洞。

## 模糊测试目标

### 1. DAG 配置解析器 (`dag_parser`)

**位置**: `fuzz/fuzz_targets/dag_parser.rs`

**测试目标**:
- `DagDefinition` TOML 解析
- `DagDefinition` JSON 解析
- DAG 执行事件解析 (`parse_dag_event`)

**安全关注点**:
- 无效 TOML/JSON 结构处理
- 循环依赖检测
- 任务 ID 冲突
- 内存分配限制

**运行方式**:
```bash
cd fuzz
cargo fuzz run dag_parser
```

### 2. Skill Manifest 解析器 (`skill_manifest`)

**位置**: `fuzz/fuzz_targets/skill_manifest.rs`

**测试目标**:
- `SkillManifest` TOML 解析
- `SkillInfo` 结构解析
- `TaskLevelDefinition` 解析
- `DagTaskDefinition` 解析
- Manifest 验证 (`ManifestValidator::validate`)
- DAG 技能转换 (`dag.to_dag()`)

**安全关注点**:
- 无效语义版本号
- 权限声明验证
- WASM 导出函数检查
- DAG 结构有效性

**运行方式**:
```bash
cd fuzz
cargo fuzz run skill_manifest
```

### 3. 网络消息解析器 (`message_parser`)

**位置**: `fuzz/fuzz_targets/message_parser.rs`

**测试目标**:
- WebSocket 消息解析 (`WsMessage`)
- 各类消息子类型:
  - `EventMessage`
  - `HandshakeMessage`
  - `AuthMessage`
  - `PingMessage` / `PongMessage`
  - `ErrorMessage`
  - `AckMessage`
  - `SyncRequest` / `SyncResponse`
- Sync 过滤器解析 (`SyncFilter`)
- Skill 事件解析 (`SkillEvent::parse_event`)
- DAG Todo 提案事件解析

**安全关注点**:
- 恶意构造的 JSON 消息
- 超大消息处理
- 无效事件类型
- 认证消息伪造

**运行方式**:
```bash
cd fuzz
cargo fuzz run message_parser
```

## 目录结构

```
fuzz/
├── Cargo.toml              # 模糊测试 crate 配置
└── fuzz_targets/
    ├── dag_parser.rs       # DAG 解析模糊测试
    ├── skill_manifest.rs   # Skill manifest 模糊测试
    └── message_parser.rs   # 网络消息解析模糊测试

reports/security/
└── fuzzing-setup.md        # 本文档
```

## 使用方法

### 运行所有模糊测试

```bash
cd fuzz

# 运行 DAG 解析器 (默认 60 秒)
cargo fuzz run dag_parser

# 运行指定时间
cargo fuzz run dag_parser -- -max_total_time=300

# 使用多核并行运行
cargo fuzz run dag_parser -- -jobs=4
```

### 运行特定语料库

```bash
# 创建语料库目录
mkdir -p fuzz/corpus/dag_parser

# 添加种子文件
echo '[dag]' > fuzz/corpus/dag_parser/seed1.toml

# 使用语料库运行
cargo fuzz run dag_parser fuzz/corpus/dag_parser
```

### 最小化崩溃输入

```bash
# 当发现崩溃时，最小化测试用例
cargo fuzz tmin dag_parser <crash-file>
```

### 生成覆盖率报告

```bash
# 运行并收集覆盖率
cargo fuzz coverage dag_parser

# 查看覆盖率报告
llvm-cov show -format=html -instr-profile=fuzz/coverage/dag_parser/coverage.profdata \
  target/x86_64-unknown-linux-gnu/coverage/x86_64-unknown-linux-gnu/release/dag_parser \
  > coverage.html
```

## 发现的问题分类

模糊测试可能发现以下类型的问题：

### 1. 解析错误 (Parse Errors)
- 无效 TOML/JSON 语法导致崩溃
- 类型不匹配导致 panic

### 2. 内存安全问题
- 栈溢出 (深嵌套结构)
- 堆分配失败 (超大输入)
- 使用 `unsafe` 块的未定义行为

### 3. 逻辑错误
- 无效 DAG 结构未正确检测
- 权限验证绕过
- 状态不一致

### 4. DoS 攻击向量
- 解析复杂度攻击 ( billion laughs )
- 内存耗尽攻击
- CPU 耗尽攻击

## 安全建议

### 输入验证

所有模糊测试目标都验证了 UTF-8 转换，确保输入首先转换为有效字符串：

```rust
if let Ok(s) = std::str::from_utf8(data) {
    // 执行解析测试
}
```

### 解析限制

建议在生产环境中添加以下限制：

1. **输入大小限制**: 最大 1MB
2. **解析深度限制**: 最大 100 层嵌套
3. **超时保护**: 解析超时 5 秒
4. **内存限制**: 解析过程最大使用 100MB

### 监控与警报

```rust
// 示例：添加解析监控
pub fn parse_with_limits(input: &str) -> Result<T> {
    let start = Instant::now();
    let result = parse(input)?;
    
    if start.elapsed() > Duration::from_secs(5) {
        warn!("Slow parse detected: {:?}", start.elapsed());
    }
    
    Ok(result)
}
```

## CI/CD 集成

建议在 CI 中添加模糊测试步骤：

```yaml
# .github/workflows/fuzz.yml
name: Fuzzing

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  fuzz:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install cargo-fuzz
        run: cargo install cargo-fuzz
      - name: Run DAG parser fuzzing (5 minutes)
        run: cd fuzz && cargo fuzz run dag_parser -- -max_total_time=300
      - name: Run Skill manifest fuzzing (5 minutes)
        run: cd fuzz && cargo fuzz run skill_manifest -- -max_total_time=300
      - name: Run Message parser fuzzing (5 minutes)
        run: cd fuzz && cargo fuzz run message_parser -- -max_total_time=300
```

## 扩展计划

### 短期 (1-2 周)
- [ ] 添加更多语料库种子文件
- [ ] 实现结构化模糊测试 (arbitrary traits)
- [ ] 添加 DID 解析模糊测试

### 中期 (1 个月)
- [ ] WASM 模块解析模糊测试
- [ ] 存储层序列化模糊测试
- [ ] 网络协议状态机模糊测试

### 长期 (3 个月)
- [ ]  differential fuzzing (对比参考实现)
- [ ]  覆盖率引导的模糊测试优化
- [ ]  自动化漏洞报告流程

## 参考资源

- [cargo-fuzz documentation](https://rust-fuzz.github.io/book/cargo-fuzz.html)
- [libFuzzer documentation](https://llvm.org/docs/LibFuzzer.html)
- [Rust Fuzz Book](https://rust-fuzz.github.io/book/)

## 更新日志

### 2026-02-08
- 初始模糊测试基础设施设置
- 添加 3 个模糊测试目标:
  - `dag_parser`: DAG 配置解析
  - `skill_manifest`: Skill manifest 解析
  - `message_parser`: 网络消息解析
