# CIS Engine Scanner - 使用指南

> **版本**: v1.1.6
> **功能**: 游戏引擎代码扫描和注入点检测
> **支持引擎**: Unreal Engine 5.7, Unity 2022, Godot 4.x

---

## 概述

CIS Engine Scanner 是一个强大的代码分析工具，用于检测游戏引擎项目中的可注入点。它能自动识别引擎类型，扫描源代码，并标记潜在的代码注入位置。

### 主要功能

- **自动引擎检测**: 智能识别项目使用的游戏引擎
- **代码模式匹配**: 使用正则表达式引擎检测 API 调用模式
- **多引擎支持**: 支持 Unreal, Unity, Godot 三大主流引擎
- **注入点分类**: 按类型分类（函数调用、变量赋值、资源加载等）
- **置信度评分**: 为每个检测点提供置信度评分

---

## 安装

CIS Engine Scanner 已集成到 cis-core v1.1.6+ 中，无需单独安装。

```bash
# 确保使用最新版本
cargo install cis-node --version 1.1.6
```

---

## CLI 命令

### 1. 扫描项目目录

扫描指定目录，检测引擎类型和注入点。

```bash
# 基本扫描
cis engine scan /path/to/project

# 指定引擎类型
cis engine scan /path/to/project --engine unreal5.7

# 输出到 JSON 文件
cis engine scan /path/to/project --output scan_result.json

# 详细输出
cis engine scan /path/to/project --verbose
```

**参数说明**:
- `directory`: 要扫描的项目目录（必需）
- `--engine`: 指定引擎类型（可选：unreal5.7, unity2022, godot4）
- `--output`: 输出文件路径（可选）
- `--verbose`: 显示详细扫描过程

**输出示例**:

```
══════════════════════════════════════════════════════════
══════════════════════════════════════════════════════════
Engine Scan Results

Engine: Unreal Engine 5.7
Files Scanned: 152
Injection Locations Found: 23

Injection Locations

1. ▸ (90% confidence)
File: /path/to/Source/MyActor.cpp
Line: 42
Type: ActorClass
Description: AActor subclass - can inject BeginPlay() logic

2. ▸ (85% confidence)
File: /path/to/Source/MyFunction.cpp
Line: 18
Type: UFunction
Description: Blueprint callable function - can inject AI logic
```

### 2. 生成扫描报告

基于之前的扫描结果生成格式化报告。

```bash
# Markdown 格式
cis engine report scan_result.json --format markdown

# JSON 格式
cis engine report scan_result.json --format json

# CSV 格式
cis engine report scan_result.json --format csv
```

### 3. 查看支持的引擎

列出所有支持的引擎类型和检测模式。

```bash
cis engine list-engines
```

**输出示例**:

```
Supported Engine Types

• Unreal5.7 - Unreal Engine 5.7
  Detection: *.uproject, Engine/Source/*.cpp

• Unity2022 - Unity 2022 LTS
  Detection: Assets/, ProjectSettings/

• Godot4 - Godot Engine 4.x
  Detection: project.godot, *.gd
```

---

## 编程 API

### 基本使用

```rust
use cis_core::engine::{EngineScanner, EngineType};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建扫描器
    let scanner = EngineScanner::new();

    // 扫描目录
    let result = scanner.scan_directory(PathBuf::from("/path/to/project")).await?;

    // 查看结果
    if let Some(engine) = result.engine {
        println!("检测到引擎: {}", engine.engine_type);
    }

    println!("扫描了 {} 个文件", result.files_scanned);
    println!("发现 {} 个注入点", result.locations.len());

    for location in result.locations {
        println!(
            "文件: {:?}, 行号: {}, 类型: {}, 置信度: {}",
            location.file_path,
            location.line_number,
            location.injection_type,
            location.confidence
        );
    }

    Ok(())
}
```

### 自定义配置

```rust
use cis_core::engine::{EngineScanner, PatternLibrary};
use std::path::PathBuf;

// 使用自定义模式库
let library = PatternLibrary::new();
let scanner = EngineScanner::with_library(library)
    .with_max_file_size(20 * 1024 * 1024) // 20 MB
    .with_follow_symlinks(true);

let result = scanner.scan_directory(PathBuf::from(".")).await?;
```

### 单文件扫描

```rust
// 同步扫描单个文件
let locations = scanner.scan_file_sync(Path::new("main.cpp"))?;

for location in locations {
    println!("发现注入点: {} at {}", location.injection_type, location.line_number);
}
```

---

## 支持的注入类型

### Unreal Engine

| 类型 | 描述 | 置信度 |
|------|--------|--------|
| `FunctionCall` | APROJECT, CALLPROCESS 等函数调用 | 95% |
| `VariableAssignment` | 全局变量赋值 | 70% |
| `ResourceLoad` | StaticLoadObject/LoadClass 调用 | 95% |
| `EventHook` | Blueprint Function Library 调用 | 90% |
| `Constructor` | AActor/APlayerController 构造 | 85% |

### Unity

| 类型 | 描述 | 置信度 |
|------|--------|--------|
| `ResourceLoad` | Resources.Load / AssetDatabase.LoadAssetAtPath | 95% |
| `EventHook` | Start/Update/OnCollision 等生命周期方法 | 95% |
| `FunctionCall` | GetComponent / AddComponent 调用 | 90% |
| `Constructor` | GameObject.Instantiate 调用 | 95% |

### Godot

| 类型 | 描述 | 置信度 |
|------|--------|--------|
| `ResourceLoad` | load / preload / ResourceLoader.load | 90% |
| `EventHook` | _ready / _process / _input 等虚函数 | 95% |
| `FunctionCall` | get_node / find_node 调用 | 90% |
| `EventHook` | connect 信号连接 | 85% |

---

## 检测模式

### Unreal Engine 模式

```cpp
// 函数调用 - 可注入
APROJECT(SomeActor)
CALLPROCESS()
StaticLoadObject<UMyClass>()

// 宏定义 - 可注入
UFUNCTION(BlueprintCallable)
UCLASS()

// 变量赋值 - 可能注入
GLOBAL_VARIABLE = value;
```

### Unity 模式

```csharp
// 生命周期方法 - 可注入
void Start() { }
void Update() { }

// 组件访问 - 可注入
GetComponent<MyComponent>()
AddComponent<MyComponent>()

// 资源加载 - 可注入
Resources.Load<GameObject>("Prefabs/MyPrefab")
AssetDatabase.LoadAssetAtPath("Assets/MyAsset.prefab")
```

### Godot 模式

```gdscript
# 节点扩展 - 可注入
extends Node

# 生命周期函数 - 可注入
func _ready():
    pass

# 资源加载 - 可注入
load("res://textures/icon.png")
preload("res://scenes/level1.tscn")

# 节点访问 - 可注入
get_node("Player")
find_node("Enemy")
```

---

## 高级用法

### 自定义注入模式

```rust
use cis_core::engine::{InjectionPattern, InjectionType, PatternLibrary};

// 创建自定义模式
let custom_pattern = InjectionPattern::new(
    "Custom Network Call".to_string(),
    r"\b(SendHTTPRequest|Fetch)\s*\(".to_string(),
    InjectionType::FunctionCall,
)
.with_confidence(0.85)
.with_language("C++".to_string());

// 添加到库中
let mut library = PatternLibrary::new();
library.add_pattern(custom_pattern);

// 使用自定义库
let scanner = EngineScanner::with_library(library);
```

### 过滤结果

```rust
use cis_core::engine::{InjectionType};

// 只获取高置信度结果
let high_confidence: Vec<_> = result
    .locations
    .into_iter()
    .filter(|l| l.confidence > 0.8)
    .collect();

// 按类型过滤
let function_calls: Vec<_> = result
    .locations
    .into_iter()
    .filter(|l| l.injection_type == InjectionType::FunctionCall)
    .collect();
```

### 集成到 DAG 工作流

```toml
# .cis/dags/code-injection-check.toml

[skill]
name = "engine-injection-scan"
type = "dag"
description = "扫描引擎代码注入点"

[[dag.tasks]]
id = "scan"
name = "扫描项目代码"
skill = "engine"
level = { type = "mechanical", retry = 3 }

[[dag.tasks]]
id = "report"
name = "生成报告"
skill = "engine"
deps = ["scan"]
level = { type = "confirmed" }
```

---

## 配置选项

### EngineScanner 配置

| 选项 | 类型 | 默认值 | 说明 |
|------|------|---------|------|
| `max_file_size` | `usize` | 10 MB | 最大扫描文件大小 |
| `follow_symlinks` | `bool` | false | 是否跟随符号链接 |

### 排除目录

以下目录默认排除，不会被扫描：

- `node_modules`
- `target`, `build`
- `.git`, `.svn`
- `Binaries`, `Intermediate` (Unreal)
- `DerivedDataCache` (Unreal)
- `Library` (Unity)
- `Temp`, `obj`

---

## 输出格式

### JSON 输出

```json
{
  "engine": {
    "engine_type": "Unreal5_7",
    "version": null,
    "root_path": "/path/to/project",
    "config_files": ["/path/to/project/MyProject.uproject"]
  },
  "locations": [
    {
      "file_path": "/path/to/file.cpp",
      "line_number": 42,
      "column_number": 8,
      "injection_type": "FunctionCall",
      "code_snippet": "APROJECT(MyActor)",
      "pattern_name": "Unreal Process Call",
      "confidence": 0.95,
      "context": null
    }
  ],
  "files_scanned": 152,
  "lines_scanned": 34200,
  "duration_ms": 1250,
  "errors": []
}
```

### Markdown 报告

```markdown
# Engine Injection Report

- **Engine**: Unreal5_7
- **Files Scanned**: 152
- **Injection Locations**: 23

## Injection Locations

### 1. FunctionCall

- **File**: `/path/to/MyActor.cpp`
- **Line**: 42
- **Type**: FunctionCall
- **Confidence**: 95%
- **Description**: Unreal process execution calls
```

### CSV 报告

```csv
File,Line,Type,Confidence,Description
/path/to/file.cpp,42,FunctionCall,0.95,"Unreal process call"
/path/to/file2.cpp,18,ResourceLoad,0.90,"Static resource load"
```

---

## 错误处理

```rust
use cis_core::error::{CisError, Result};

match scanner.scan_directory(path).await {
    Ok(result) => {
        // 处理结果
    }
    Err(CisError::NotFound { message }) => {
        eprintln!("目录不存在: {}", message);
    }
    Err(CisError::Io { message, .. }) => {
        eprintln!("IO 错误: {}", message);
    }
    Err(e) => {
        eprintln!("扫描失败: {}", e);
    }
}
```

---

## 性能优化

### 大型项目扫描

对于大型项目（>10,000 文件），建议：

```rust
// 1. 限制文件大小
let scanner = EngineScanner::new()
    .with_max_file_size(1024 * 1024); // 1 MB

// 2. 分批扫描
let subdirs = vec!["Source", "Plugins", "Content"];
for subdir in subdirs {
    let result = scanner.scan_directory(PathBuf::from(subdir)).await?;
    // 处理每个子目录的结果
}

// 3. 并行扫描（需要自定义实现）
use futures::future::join_all;

let scan_tasks = subdirs.into_iter()
    .map(|dir| scanner.scan_directory(PathBuf::from(dir)))
    .collect::<Vec<_>>();

let results = join_all(scan_tasks).await;
```

---

## 最佳实践

1. **定期扫描**: 在 CI/CD 流程中集成扫描
2. **记录结果**: 保存扫描历史以追踪变化
3. **人工审查**: 高置信度结果仍需人工验证
4. **更新模式**: 根据项目特点更新检测模式
5. **排除无关代码**: 使用 .gitignore 风格的排除规则

---

## 故障排查

### 扫描速度慢

```bash
# 限制文件大小
export CIS_MAX_FILE_SIZE=1048576  # 1 MB

# 限制扫描深度
find . -maxdepth 5 -name "*.cpp" | xargs cis engine scan
```

### 误报率高

```rust
// 提高置信度阈值
let filtered: Vec<_> = result
    .locations
    .into_iter()
    .filter(|l| l.confidence >= 0.9)
    .collect();
```

### 漏报

```rust
// 添加自定义模式
let mut library = PatternLibrary::new();
library.add_pattern(custom_pattern);
let scanner = EngineScanner::with_library(library);
```

---

## 示例项目

### 1. Unreal 项目扫描

```bash
# 检测 UE5 项目
cd /path/to/ue5-project
cis engine scan . --output ue5-scan.json

# 查看结果
cis engine report ue5-scan.json --format markdown
```

### 2. Unity 项目扫描

```bash
# Unity 项目
cd /path/to/unity-project
cis engine scan . --engine unity2022

# 生成 CSV 报告
cis engine report scan_result.json --format csv > report.csv
```

### 3. Godot 项目扫描

```bash
# Godot 项目
cd /path/to/godot-project
cis engine scan . --engine godot4 --verbose
```

---

## 相关文档

- [CIS 使用指南](../CLAUDE.md)
- [DAG 编排](../docs/DISTRIBUTED_DAG_COORDINATOR.md)
- [CLI 参考](../cis-node/src/cli/README.md)

---

## 更新日志

### v1.1.6 (2026-02-13)

- ✅ 初始实现
  - 支持 Unreal Engine 5.7, Unity 2022, Godot 4.x
  - 30+ 内置注入模式
  - 多格式报告输出
  - 完整单元测试

### 未来计划

- [ ] 支持 Unreal Engine 5.x 早期版本
- [ ] 添加自定义模式配置文件
- [ ] AI 辅助的误报过滤
- [ ] 可视化报告生成（HTML）

---

**最后更新**: 2026-02-13
**维护者**: CIS Team
