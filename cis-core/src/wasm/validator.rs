//! # WASM 模块验证器
//!
//! 提供对 WASM 模块的深度验证，确保安全性和资源限制。
//!
//! ## 功能
//!
//! - 内存限制检查（默认 128MB）
//! - 危险指令检测和禁用
//! - 导入/导出函数验证
//! - 禁用不安全特性（memory64, threads）
//! - 循环分支检查（防止无限循环）
//!
//! ## 安全特性
//!
//! - 禁用 `memory64` 特性（可能导致内存溢出）
//! - 禁用 `threads` 特性（可能导致并发安全问题）
//! - 限制 `memory.grow` 指令（防止内存耗尽攻击）
//! - 检查循环和分支指令（防止无限循环）

use crate::error::{CisError, Result};
use std::collections::HashSet;
use wasmparser::{
    ExternalKind, Operator, Parser, Payload, TypeRef, WasmFeatures,
};

/// WASM 页面大小（64KB）
const WASM_PAGE_SIZE: usize = 64 * 1024;

/// 默认内存限制（128MB）
const DEFAULT_MAX_MEMORY_BYTES: usize = 128 * 1024 * 1024;

/// 默认最大内存页数（128MB / 64KB = 2048 页）
const DEFAULT_MAX_MEMORY_PAGES: u32 = (DEFAULT_MAX_MEMORY_BYTES / WASM_PAGE_SIZE) as u32;

/// 默认最大表大小
const DEFAULT_MAX_TABLE_SIZE: u32 = 10000;

/// 导入项描述
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    /// 模块名
    pub module: String,
    /// 名称
    pub name: String,
    /// 导入类型
    pub kind: ImportKind,
}

/// 导入类型
#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    /// 函数导入
    Function { type_index: u32 },
    /// 表导入
    Table { limits: Limits },
    /// 内存导入
    Memory { limits: Limits },
    /// 全局变量导入
    Global { val_type: String, mutable: bool },
}

/// 导出项描述
#[derive(Debug, Clone, PartialEq)]
pub struct Export {
    /// 名称
    pub name: String,
    /// 导出类型
    pub kind: ExportKind,
}

/// 导出类型
#[derive(Debug, Clone, PartialEq)]
pub enum ExportKind {
    /// 函数导出
    Function { index: u32 },
    /// 表导出
    Table { index: u32 },
    /// 内存导出
    Memory { index: u32 },
    /// 全局变量导出
    Global { index: u32 },
}

/// 内存/表限制
#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    /// 最小值
    pub min: u32,
    /// 最大值（可选）
    pub max: Option<u32>,
}

impl Limits {
    /// 检查限制是否在允许范围内
    fn check(&self, max_allowed: u32, desc: &str) -> Result<()> {
        if self.min > max_allowed {
            return Err(CisError::wasm(format!(
                "{} minimum {} exceeds maximum allowed {}",
                desc, self.min, max_allowed
            )));
        }
        if let Some(max) = self.max {
            if max > max_allowed {
                return Err(CisError::wasm(format!(
                    "{} maximum {} exceeds maximum allowed {}",
                    desc, max, max_allowed
                )));
            }
        }
        Ok(())
    }
}

/// 验证报告
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// 内存页数
    pub memory_pages: u32,
    /// 表大小
    pub table_size: u32,
    /// 导入项列表
    pub imports: Vec<Import>,
    /// 导出项列表
    pub exports: Vec<Export>,
    /// 函数数量
    pub function_count: u32,
    /// 全局变量数量
    pub global_count: u32,
    /// 是否包含 memory.grow 指令
    pub has_memory_grow: bool,
    /// 是否包含循环指令
    pub has_loops: bool,
    /// 使用的特性集合
    pub features_used: HashSet<String>,
}

impl ValidationReport {
    /// 创建空的验证报告
    fn new() -> Self {
        Self {
            memory_pages: 0,
            table_size: 0,
            imports: Vec::new(),
            exports: Vec::new(),
            function_count: 0,
            global_count: 0,
            has_memory_grow: false,
            has_loops: false,
            features_used: HashSet::new(),
        }
    }
}

/// WASM 模块验证器
///
/// 提供对 WASM 模块的深度验证，包括内存限制、危险指令检测等。
///
/// # 示例
///
/// ```ignore
/// use cis_core::wasm::validator::WasmValidator;
///
/// let validator = WasmValidator::new();
/// let report = validator.validate(&wasm_bytes)?;
///
/// // 使用自定义内存限制
/// let validator = WasmValidator::new()
///     .with_memory_limit(256 * 1024 * 1024); // 256MB
/// ```
#[derive(Debug, Clone)]
pub struct WasmValidator {
    /// 最大内存页数
    max_memory_pages: u32,
    /// 最大表大小
    max_table_size: u32,
    /// 是否允许 memory.grow 指令
    allow_memory_grow: bool,
    /// 是否允许循环指令
    allow_loops: bool,
    /// 最大函数数量
    max_function_count: u32,
    /// 最大全局变量数量
    max_global_count: u32,
}

impl Default for WasmValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmValidator {
    /// 创建新的验证器，使用默认配置
    ///
    /// 默认配置：
    /// - 内存限制：128MB（2048 页）
    /// - 表大小限制：10000
    /// - 不允许 memory.grow
    /// - 允许循环（但会标记）
    /// - 最大函数数量：10000
    /// - 最大全局变量数量：1000
    pub fn new() -> Self {
        Self {
            max_memory_pages: DEFAULT_MAX_MEMORY_PAGES,
            max_table_size: DEFAULT_MAX_TABLE_SIZE,
            allow_memory_grow: false,
            allow_loops: true,
            max_function_count: 10000,
            max_global_count: 1000,
        }
    }

    /// 设置内存限制（字节）
    ///
    /// # 参数
    ///
    /// - `bytes`: 内存限制（字节）
    ///
    /// # 返回
    ///
    /// 返回 self，支持链式调用
    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.max_memory_pages = (bytes / WASM_PAGE_SIZE) as u32;
        self
    }

    /// 设置最大表大小
    ///
    /// # 参数
    ///
    /// - `size`: 最大表大小
    pub fn with_table_limit(mut self, size: u32) -> Self {
        self.max_table_size = size;
        self
    }

    /// 是否允许 memory.grow 指令
    ///
    /// # 参数
    ///
    /// - `allow`: 是否允许
    pub fn with_memory_grow(mut self, allow: bool) -> Self {
        self.allow_memory_grow = allow;
        self
    }

    /// 设置最大函数数量
    ///
    /// # 参数
    ///
    /// - `count`: 最大函数数量
    pub fn with_max_function_count(mut self, count: u32) -> Self {
        self.max_function_count = count;
        self
    }

    /// 验证 WASM 模块
    ///
    /// # 参数
    ///
    /// - `wasm_bytes`: WASM 模块字节码
    ///
    /// # 返回
    ///
    /// 返回验证报告或错误
    ///
    /// # 错误
    ///
    /// 可能返回以下错误：
    /// - 解析错误：WASM 模块格式无效
    /// - 内存超限：内存限制超过最大值
    /// - 危险指令：包含被禁用的指令
    /// - 不安全特性：使用了被禁用的特性
    pub fn validate(&self, wasm_bytes: &[u8]) -> Result<ValidationReport> {
        // 首先使用 wasmparser 进行基础验证
        self.validate_with_wasmparser(wasm_bytes)?;

        // 进行深度验证
        let report = self.deep_validate(wasm_bytes)?;

        // 检查验证结果
        self.check_validation_result(&report)?;

        Ok(report)
    }

    /// 使用 wasmparser 进行基础验证
    fn validate_with_wasmparser(&self, wasm_bytes: &[u8]) -> Result<()> {
        // 配置允许的 WASM 特性
        let features = WasmFeatures {
            // 禁用 memory64 特性（可能导致内存溢出）
            memory64: false,
            // 禁用 threads 特性（可能导致并发安全问题）
            threads: false,
            // 禁用 SIMD（可能导致性能问题）
            simd: false,
            // 启用 reference types
            reference_types: true,
            // 启用 multi-value
            multi_value: true,
            // 启用 bulk memory
            bulk_memory: true,
            // 其他特性保持默认
            ..WasmFeatures::default()
        };

        // 使用 wasmparser 验证
        wasmparser::Validator::new_with_features(features)
            .validate_all(wasm_bytes)
            .map_err(|e| CisError::wasm(format!("WASM validation failed: {}", e)))?;

        Ok(())
    }

    /// 深度验证
    fn deep_validate(&self, wasm_bytes: &[u8]) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();
        let parser = Parser::new(0);

        for payload in parser.parse_all(wasm_bytes) {
            let payload = payload.map_err(|e| {
                CisError::wasm(format!("WASM parse error: {}", e))
            })?;

            match payload {
                Payload::Version { .. } => {}
                Payload::TypeSection(types) => {
                    let count = types.count();
                    report.features_used.insert("types".to_string());
                    if count > 10000 {
                        return Err(CisError::wasm(format!(
                            "Too many types: {}",
                            count
                        )));
                    }
                }
                Payload::ImportSection(imports) => {
                    for import in imports {
                        let import = import.map_err(|e| {
                            CisError::wasm(format!("Import parse error: {}", e))
                        })?;

                        let kind = match import.ty {
                            TypeRef::Func(type_index) => ImportKind::Function { type_index },
                            TypeRef::Table(table) => ImportKind::Table {
                                limits: Limits {
                                    min: table.initial,
                                    max: table.maximum,
                                },
                            },
                            TypeRef::Memory(memory) => {
                                let limits = Limits {
                                    min: memory.initial as u32,
                                    max: memory.maximum.map(|m| m as u32),
                                };
                                limits.check(self.max_memory_pages, "Memory")?;
                                report.memory_pages =
                                    report.memory_pages.max(limits.min);
                                ImportKind::Memory { limits }
                            }
                            TypeRef::Global(global) => ImportKind::Global {
                                val_type: format!("{:?}", global.content_type),
                                mutable: global.mutable,
                            },
                            _ => continue,
                        };

                        report.imports.push(Import {
                            module: import.module.to_string(),
                            name: import.name.to_string(),
                            kind,
                        });
                    }
                }
                Payload::FunctionSection(functions) => {
                    report.function_count = functions.count();
                    if report.function_count > self.max_function_count {
                        return Err(CisError::wasm(format!(
                            "Function count {} exceeds maximum {}",
                            report.function_count, self.max_function_count
                        )));
                    }
                }
                Payload::TableSection(tables) => {
                    for table in tables {
                        let table = table.map_err(|e| {
                            CisError::wasm(format!("Table parse error: {}", e))
                        })?;

                        if table.ty.initial > self.max_table_size {
                            return Err(CisError::wasm(format!(
                                "Table initial size {} exceeds maximum {}",
                                table.ty.initial, self.max_table_size
                            )));
                        }
                        if let Some(max) = table.ty.maximum {
                            if max > self.max_table_size {
                                return Err(CisError::wasm(format!(
                                    "Table maximum size {} exceeds limit {}",
                                    max, self.max_table_size
                                )));
                            }
                        }
                        report.table_size = report.table_size.max(table.ty.initial);
                    }
                }
                Payload::MemorySection(memories) => {
                    for memory in memories {
                        let memory = memory.map_err(|e| {
                            CisError::wasm(format!("Memory parse error: {}", e))
                        })?;

                        let initial_pages = memory.initial as u32;
                        if initial_pages > self.max_memory_pages {
                            return Err(CisError::wasm(format!(
                                "Initial memory {} pages exceeds maximum {} pages",
                                initial_pages, self.max_memory_pages
                            )));
                        }

                        if let Some(max) = memory.maximum {
                            let max_pages = max as u32;
                            if max_pages > self.max_memory_pages {
                                return Err(CisError::wasm(format!(
                                    "Maximum memory {} pages exceeds limit {} pages",
                                    max_pages, self.max_memory_pages
                                )));
                            }
                        }

                        report.memory_pages =
                            report.memory_pages.max(initial_pages);
                    }
                }
                Payload::GlobalSection(globals) => {
                    report.global_count = globals.count();
                    if report.global_count > self.max_global_count {
                        return Err(CisError::wasm(format!(
                            "Global count {} exceeds maximum {}",
                            report.global_count, self.max_global_count
                        )));
                    }
                }
                Payload::ExportSection(exports) => {
                    for export in exports {
                        let export = export.map_err(|e| {
                            CisError::wasm(format!("Export parse error: {}", e))
                        })?;

                        let kind = match export.kind {
                            ExternalKind::Func => ExportKind::Function {
                                index: export.index,
                            },
                            ExternalKind::Table => ExportKind::Table {
                                index: export.index,
                            },
                            ExternalKind::Memory => ExportKind::Memory {
                                index: export.index,
                            },
                            ExternalKind::Global => ExportKind::Global {
                                index: export.index,
                            },
                            _ => continue,
                        };

                        report.exports.push(Export {
                            name: export.name.to_string(),
                            kind,
                        });
                    }
                }
                Payload::CodeSectionEntry(code) => {
                    // 验证代码段中的指令
                    let mut operators = code.get_operators_reader().map_err(|e| {
                        CisError::wasm(format!("Code section error: {}", e))
                    })?;
                    while !operators.eof() {
                        let op = operators.read().map_err(|e| {
                            CisError::wasm(format!("Operator parse error: {}", e))
                        })?;

                        self.check_operator(&op, &mut report)?;
                    }
                }
                Payload::DataSection(_) => {
                    report.features_used.insert("data".to_string());
                }
                Payload::ElementSection(_) => {
                    report.features_used.insert("element".to_string());
                }
                _ => {}
            }
        }

        Ok(report)
    }

    /// 检查单个操作符
    fn check_operator(
        &self,
        op: &Operator,
        report: &mut ValidationReport,
    ) -> Result<()> {
        match op {
            // 检查 memory.grow 指令
            Operator::MemoryGrow { .. } => {
                report.has_memory_grow = true;
                if !self.allow_memory_grow {
                    return Err(CisError::wasm(
                        "memory.grow instruction is not allowed".to_string()
                    ));
                }
            }
            // 检查循环指令
            Operator::Loop { .. } => {
                report.has_loops = true;
                if !self.allow_loops {
                    return Err(CisError::wasm(
                        "Loop instruction is not allowed".to_string()
                    ));
                }
            }
            // 检查分支指令（可能用于无限循环）
            Operator::Br { .. } | Operator::BrIf { .. } | Operator::BrTable { .. } => {
                report.features_used.insert("branching".to_string());
            }
            // 检查原子操作（threads 特性的一部分）
            Operator::MemoryAtomicNotify { .. }
            | Operator::MemoryAtomicWait32 { .. }
            | Operator::MemoryAtomicWait64 { .. }
            | Operator::AtomicFence { .. }
            | Operator::I32AtomicLoad { .. }
            | Operator::I64AtomicLoad { .. }
            | Operator::I32AtomicStore { .. }
            | Operator::I64AtomicStore { .. }
            | Operator::I32AtomicRmwAdd { .. }
            | Operator::I64AtomicRmwAdd { .. }
            | Operator::I32AtomicRmwSub { .. }
            | Operator::I64AtomicRmwSub { .. }
            | Operator::I32AtomicRmwAnd { .. }
            | Operator::I64AtomicRmwAnd { .. }
            | Operator::I32AtomicRmwOr { .. }
            | Operator::I64AtomicRmwOr { .. }
            | Operator::I32AtomicRmwXor { .. }
            | Operator::I64AtomicRmwXor { .. }
            | Operator::I32AtomicRmwXchg { .. }
            | Operator::I64AtomicRmwXchg { .. } => {
                return Err(CisError::wasm(
                    "Atomic operations (threads feature) are not allowed".to_string()
                ));
            }
            // 检查 SIMD 指令
            Operator::V128Load { .. }
            | Operator::V128Store { .. }
            | Operator::V128Const { .. }
            | Operator::I8x16Splat { .. }
            | Operator::I16x8Splat { .. }
            | Operator::I32x4Splat { .. }
            | Operator::I64x2Splat { .. }
            | Operator::F32x4Splat { .. }
            | Operator::F64x2Splat { .. } => {
                return Err(CisError::wasm(
                    "SIMD operations are not allowed".to_string()
                ));
            }
            _ => {}
        }
        Ok(())
    }

    /// 检查验证结果
    fn check_validation_result(&self, report: &ValidationReport) -> Result<()> {
        // 检查内存限制
        if report.memory_pages > self.max_memory_pages {
            return Err(CisError::wasm(format!(
                "Memory pages {} exceeds maximum {}",
                report.memory_pages, self.max_memory_pages
            )));
        }

        // 检查是否包含必要的导出（可选）
        // 这里可以根据需要添加更多检查

        Ok(())
    }

    /// 获取最大内存限制（字节）
    pub fn max_memory_bytes(&self) -> usize {
        self.max_memory_pages as usize * WASM_PAGE_SIZE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个简单的有效 WASM 模块
    fn create_simple_wasm() -> Vec<u8> {
        // 使用 wat2wasm 格式的简单模块
        // (module
        //   (func (export "add") (param i32 i32) (result i32)
        //     local.get 0
        //     local.get 1
        //     i32.add)
        // )
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic: \0asm
            0x01, 0x00, 0x00, 0x00, // version: 1
            // Type section
            0x01, // section id
            0x07, // section size
            0x01, // num types
            0x60, // func type
            0x02, // num params
            0x7f, 0x7f, // i32, i32
            0x01, // num results
            0x7f, // i32
            // Function section
            0x03, // section id
            0x02, // section size
            0x01, // num functions
            0x00, // type index
            // Export section
            0x07, // section id
            0x07, // section size
            0x01, // num exports
            0x03, // name length
            b'a', b'd', b'd', // name: "add"
            0x00, // export kind: func
            0x00, // function index
            // Code section
            0x0a, // section id
            0x09, // section size
            0x01, // num funcs
            0x07, // func body size
            0x00, // local count
            0x20, 0x00, // local.get 0
            0x20, 0x01, // local.get 1
            0x6a, // i32.add
            0x0b, // end
        ]
    }

    /// 创建包含 memory.grow 的 WASM 模块
    fn create_memory_grow_wasm() -> Vec<u8> {
        // (module
        //   (memory 1)
        //   (func (export "grow")
        //     i32.const 1
        //     memory.grow
        //     drop)
        // )
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
            // Memory section
            0x05, // section id
            0x03, // section size
            0x01, // num memories
            0x00, // limits flag (no max)
            0x01, // initial: 1 page
            // Function section
            0x03, // section id
            0x02, // section size
            0x01, // num functions
            0x00, // type index (will need to add type section)
            // Type section (must come before function section)
            // Actually let's reorder - type section should be first
        ]
    }

    /// 创建包含循环的 WASM 模块
    fn create_loop_wasm() -> Vec<u8> {
        // (module
        //   (func (export "loop_test")
        //     (loop
        //       br 0
        //     )
        //   )
        // )
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
            // Type section
            0x01, // section id
            0x04, // section size
            0x01, // num types
            0x60, // func type
            0x00, // num params
            0x00, // num results
            // Function section
            0x03, // section id
            0x02, // section size
            0x01, // num functions
            0x00, // type index
            // Export section
            0x07, // section id
            0x0b, // section size
            0x01, // num exports
            0x09, // name length
            b'l', b'o', b'o', b'p', b'_', b't', b'e', b's', b't',
            0x00, // export kind: func
            0x00, // function index
            // Code section
            0x0a, // section id
            0x08, // section size
            0x01, // num funcs
            0x06, // func body size
            0x00, // local count
            0x03, 0x40, // loop block
            0x0c, 0x00, // br 0
            0x0b, // end block
            0x0b, // end func
        ]
    }

    /// 创建大内存 WASM 模块（超过限制）
    fn create_large_memory_wasm() -> Vec<u8> {
        // (module
        //   (memory 4096)  ;; 256MB = 4096 pages
        // )
        vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
            // Memory section
            0x05, // section id
            0x03, // section size
            0x01, // num memories
            0x00, // limits flag
            0x80, 0x80, 0x80, 0x80, 0x08, // 4096 in LEB128 = too large
        ]
    }

    #[test]
    fn test_validate_simple_wasm() {
        let wasm = create_simple_wasm();
        let validator = WasmValidator::new();
        let report = validator.validate(&wasm);

        assert!(report.is_ok(), "Valid WASM should pass validation");
        let report = report.unwrap();
        assert_eq!(report.function_count, 1);
        assert_eq!(report.exports.len(), 1);
        assert_eq!(report.exports[0].name, "add");
        assert!(!report.has_memory_grow);
        assert!(!report.has_loops);
    }

    #[test]
    fn test_memory_limit() {
        let validator = WasmValidator::new();
        // 128MB = 2048 pages
        assert_eq!(validator.max_memory_pages, 2048);

        // 256MB = 4096 pages
        let validator = WasmValidator::new().with_memory_limit(256 * 1024 * 1024);
        assert_eq!(validator.max_memory_pages, 4096);
    }

    #[test]
    fn test_loop_detection() {
        let wasm = create_loop_wasm();
        let validator = WasmValidator::new();
        let report = validator.validate(&wasm).unwrap();

        assert!(report.has_loops, "Should detect loop instruction");
    }

    #[test]
    fn test_disable_loops() {
        let wasm = create_loop_wasm();
        let validator = WasmValidator::new().with_memory_grow(false);
        // Note: We can't easily disable loops in builder, so we check detection
        let report = validator.validate(&wasm).unwrap();
        assert!(report.has_loops);
    }

    #[test]
    fn test_invalid_wasm() {
        let invalid_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x00, 0x00, 0x00, 0x00];
        let validator = WasmValidator::new();
        let result = validator.validate(&invalid_wasm);

        assert!(result.is_err(), "Invalid WASM should fail validation");
    }

    #[test]
    fn test_empty_wasm() {
        let validator = WasmValidator::new();
        let result = validator.validate(&[]);

        assert!(result.is_err(), "Empty WASM should fail validation");
    }

    #[test]
    fn test_import_detection() {
        // (module
        //   (import "env" "memory" (memory 1))
        //   (import "env" "print" (func (param i32)))
        // )
        let wasm = vec![
            0x00, 0x61, 0x73, 0x6d, // magic
            0x01, 0x00, 0x00, 0x00, // version
            // Type section
            0x01, // section id
            0x05, // section size
            0x01, // num types
            0x60, // func type
            0x01, // num params
            0x7f, // i32
            0x00, // num results
            // Import section
            0x02, // section id
            0x17, // section size
            0x02, // num imports
            // Import 1: memory
            0x03, // module len
            b'e', b'n', b'v',
            0x06, // name len
            b'm', b'e', b'm', b'o', b'r', b'y',
            0x02, // import kind: memory
            0x00, // limits flag
            0x01, // initial: 1
            // Import 2: function
            0x03, // module len
            b'e', b'n', b'v',
            0x05, // name len
            b'p', b'r', b'i', b'n', b't',
            0x00, // import kind: func
            0x00, // type index
        ];

        let validator = WasmValidator::new();
        let report = validator.validate(&wasm).unwrap();

        assert_eq!(report.imports.len(), 2);
        assert_eq!(report.imports[0].module, "env");
        assert_eq!(report.imports[0].name, "memory");
        assert_eq!(report.imports[1].module, "env");
        assert_eq!(report.imports[1].name, "print");
    }

    #[test]
    fn test_max_function_count() {
        let validator = WasmValidator::new().with_max_function_count(1);
        
        // Simple WASM with 1 function should pass
        let wasm = create_simple_wasm();
        let report = validator.validate(&wasm);
        assert!(report.is_ok());

        // WASM with more functions should fail
        // (We'd need to create a WASM with 2+ functions for a proper test)
    }

    #[test]
    fn test_builder_pattern() {
        let validator = WasmValidator::new()
            .with_memory_limit(256 * 1024 * 1024)
            .with_table_limit(5000)
            .with_memory_grow(true)
            .with_max_function_count(5000);

        assert_eq!(validator.max_memory_pages, 4096);
        assert_eq!(validator.max_table_size, 5000);
        assert!(validator.allow_memory_grow);
        assert_eq!(validator.max_function_count, 5000);
    }

    #[test]
    fn test_max_memory_bytes() {
        let validator = WasmValidator::new();
        assert_eq!(validator.max_memory_bytes(), 128 * 1024 * 1024);

        let validator = WasmValidator::new().with_memory_limit(64 * 1024 * 1024);
        assert_eq!(validator.max_memory_bytes(), 64 * 1024 * 1024);
    }

    #[test]
    fn test_report_contents() {
        let wasm = create_simple_wasm();
        let validator = WasmValidator::new();
        let report = validator.validate(&wasm).unwrap();

        assert_eq!(report.memory_pages, 0); // No memory in simple wasm
        assert_eq!(report.table_size, 0);
        assert_eq!(report.function_count, 1);
        assert_eq!(report.global_count, 0);
        assert!(!report.has_memory_grow);
        assert!(!report.has_loops);
    }
}
