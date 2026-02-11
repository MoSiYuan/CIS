//! # WASM Configuration
//!
//! WebAssembly runtime configuration including memory limits, execution timeouts, and syscall restrictions.

use serde::{Deserialize, Serialize};

use super::{validation_error, ValidateConfig};
use crate::error::Result;

/// Default maximum memory per WASM instance (512 MB)
pub const DEFAULT_MAX_MEMORY: usize = 512 * 1024 * 1024;

/// Default maximum execution time (30 seconds)
pub const DEFAULT_MAX_EXECUTION_TIME_SECS: u64 = 30;

/// Default stack size (1 MB)
pub const DEFAULT_STACK_SIZE: usize = 1024 * 1024;

/// Default table size limit (10,000 entries)
pub const DEFAULT_TABLE_SIZE: u32 = 10000;

/// Default fuel limit (for fuel-metering execution)
pub const DEFAULT_FUEL_LIMIT: u64 = 10_000_000_000;

/// Default compilation timeout (60 seconds)
pub const DEFAULT_COMPILATION_TIMEOUT_SECS: u64 = 60;

/// Default memory pages limit (8192 pages = 512 MB)
pub const DEFAULT_MEMORY_PAGES_LIMIT: u32 = 8192;

/// Default instance pool size (number of precompiled instances to keep)
pub const DEFAULT_INSTANCE_POOL_SIZE: usize = 10;

/// WASM runtime configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    /// Maximum memory per WASM instance in bytes
    #[serde(default = "default_max_memory")]
    pub max_memory: usize,

    /// Maximum execution time per call
    #[serde(default = "default_max_execution_time")]
    pub max_execution_time: std::time::Duration,

    /// Stack size in bytes
    #[serde(default = "default_stack_size")]
    pub stack_size: usize,

    /// Maximum table size (number of entries)
    #[serde(default = "default_table_size")]
    pub table_size: u32,

    /// Fuel limit for fuel-metering execution
    #[serde(default = "default_fuel_limit")]
    pub fuel_limit: u64,

    /// Compilation timeout
    #[serde(default = "default_compilation_timeout")]
    pub compilation_timeout: std::time::Duration,

    /// Memory pages limit
    #[serde(default = "default_memory_pages_limit")]
    pub memory_pages_limit: u32,

    /// Instance pool size for caching precompiled modules
    #[serde(default = "default_instance_pool_size")]
    pub instance_pool_size: usize,

    /// Allowed system calls
    #[serde(default = "default_allowed_syscalls")]
    pub allowed_syscalls: Vec<String>,

    /// Allowed host functions
    #[serde(default = "default_allowed_host_functions")]
    pub allowed_host_functions: Vec<String>,

    /// Enable SIMD support
    #[serde(default = "default_simd_enabled")]
    pub simd_enabled: bool,

    /// Enable multi-threading support
    #[serde(default = "default_threads_enabled")]
    pub threads_enabled: bool,

    /// Enable bulk memory operations
    #[serde(default = "default_bulk_memory_enabled")]
    pub bulk_memory_enabled: bool,

    /// Enable reference types
    #[serde(default = "default_reference_types_enabled")]
    pub reference_types_enabled: bool,

    /// Enable multi-value returns
    #[serde(default = "default_multi_value_enabled")]
    pub multi_value_enabled: bool,

    /// Enable module caching
    #[serde(default = "default_module_caching_enabled")]
    pub module_caching_enabled: bool,

    /// Cache directory for compiled modules
    #[serde(default)]
    pub cache_dir: Option<std::path::PathBuf>,

    /// Enable strict validation (reject modules that don't follow strict validation)
    #[serde(default = "default_strict_validation")]
    pub strict_validation: bool,

    /// Maximum module size in bytes (100 MB)
    #[serde(default = "default_max_module_size")]
    pub max_module_size: usize,

    /// Maximum number of globals
    #[serde(default = "default_max_globals")]
    pub max_globals: u32,

    /// Maximum number of functions
    #[serde(default = "default_max_functions")]
    pub max_functions: u32,

    /// Maximum number of data segments
    #[serde(default = "default_max_data_segments")]
    pub max_data_segments: u32,

    /// Gas cost configuration
    #[serde(default)]
    pub gas_costs: GasCosts,
}

impl Default for WasmConfig {
    fn default() -> Self {
        Self {
            max_memory: default_max_memory(),
            max_execution_time: default_max_execution_time(),
            stack_size: default_stack_size(),
            table_size: default_table_size(),
            fuel_limit: default_fuel_limit(),
            compilation_timeout: default_compilation_timeout(),
            memory_pages_limit: default_memory_pages_limit(),
            instance_pool_size: default_instance_pool_size(),
            allowed_syscalls: default_allowed_syscalls(),
            allowed_host_functions: default_allowed_host_functions(),
            simd_enabled: default_simd_enabled(),
            threads_enabled: default_threads_enabled(),
            bulk_memory_enabled: default_bulk_memory_enabled(),
            reference_types_enabled: default_reference_types_enabled(),
            multi_value_enabled: default_multi_value_enabled(),
            module_caching_enabled: default_module_caching_enabled(),
            cache_dir: None,
            strict_validation: default_strict_validation(),
            max_module_size: default_max_module_size(),
            max_globals: default_max_globals(),
            max_functions: default_max_functions(),
            max_data_segments: default_max_data_segments(),
            gas_costs: GasCosts::default(),
        }
    }
}

impl ValidateConfig for WasmConfig {
    fn validate(&self) -> Result<()> {
        // Validate memory limits
        if self.max_memory == 0 {
            return Err(validation_error("max_memory cannot be zero"));
        }
        if self.max_memory < 1024 * 1024 {
            return Err(validation_error(
                "max_memory must be at least 1 MB",
            ));
        }
        if self.max_memory > 4 * 1024 * 1024 * 1024 {
            // Max 4 GB
            return Err(validation_error(
                "max_memory cannot exceed 4 GB",
            ));
        }

        // Validate stack size
        if self.stack_size == 0 {
            return Err(validation_error("stack_size cannot be zero"));
        }
        if self.stack_size > self.max_memory {
            return Err(validation_error(
                "stack_size cannot exceed max_memory",
            ));
        }

        // Validate table size
        if self.table_size == 0 {
            return Err(validation_error("table_size cannot be zero"));
        }
        if self.table_size > 100000 {
            return Err(validation_error(
                "table_size cannot exceed 100000",
            ));
        }

        // Validate timeouts are not zero
        if self.max_execution_time.is_zero() {
            return Err(validation_error("max_execution_time cannot be zero"));
        }
        if self.max_execution_time > std::time::Duration::from_secs(300) {
            // Max 5 minutes
            return Err(validation_error(
                "max_execution_time cannot exceed 300 seconds",
            ));
        }

        if self.compilation_timeout.is_zero() {
            return Err(validation_error("compilation_timeout cannot be zero"));
        }

        // Validate fuel limit
        if self.fuel_limit == 0 {
            return Err(validation_error("fuel_limit cannot be zero"));
        }

        // Validate memory pages limit
        if self.memory_pages_limit == 0 {
            return Err(validation_error("memory_pages_limit cannot be zero"));
        }
        // 1 page = 64 KB, 65536 pages = 4 GB
        if self.memory_pages_limit > 65536 {
            return Err(validation_error(
                "memory_pages_limit cannot exceed 65536 (4 GB)",
            ));
        }

        // Validate instance pool size
        if self.instance_pool_size == 0 {
            return Err(validation_error("instance_pool_size cannot be zero"));
        }
        if self.instance_pool_size > 100 {
            return Err(validation_error(
                "instance_pool_size cannot exceed 100",
            ));
        }

        // Validate max module size
        if self.max_module_size == 0 {
            return Err(validation_error("max_module_size cannot be zero"));
        }
        if self.max_module_size > 500 * 1024 * 1024 {
            // Max 500 MB
            return Err(validation_error(
                "max_module_size cannot exceed 500 MB",
            ));
        }

        // Validate max globals
        if self.max_globals == 0 {
            return Err(validation_error("max_globals cannot be zero"));
        }
        if self.max_globals > 100000 {
            return Err(validation_error(
                "max_globals cannot exceed 100000",
            ));
        }

        // Validate max functions
        if self.max_functions == 0 {
            return Err(validation_error("max_functions cannot be zero"));
        }
        if self.max_functions > 100000 {
            return Err(validation_error(
                "max_functions cannot exceed 100000",
            ));
        }

        // Validate max data segments
        if self.max_data_segments == 0 {
            return Err(validation_error("max_data_segments cannot be zero"));
        }
        if self.max_data_segments > 100000 {
            return Err(validation_error(
                "max_data_segments cannot exceed 100000",
            ));
        }

        // Validate gas costs
        self.gas_costs.validate()?;

        Ok(())
    }
}

/// Gas cost configuration for fuel-metering
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GasCosts {
    /// Base cost per operation
    #[serde(default = "default_gas_base_cost")]
    pub base_cost: u64,

    /// Cost per memory access
    #[serde(default = "default_gas_memory_cost")]
    pub memory_cost: u64,

    /// Cost per instruction
    #[serde(default = "default_gas_instruction_cost")]
    pub instruction_cost: u64,

    /// Cost per call
    #[serde(default = "default_gas_call_cost")]
    pub call_cost: u64,

    /// Cost per host function call
    #[serde(default = "default_gas_host_call_cost")]
    pub host_call_cost: u64,

    /// Cost per memory page
    #[serde(default = "default_gas_memory_page_cost")]
    pub memory_page_cost: u64,

    /// Cost per table access
    #[serde(default = "default_gas_table_cost")]
    pub table_cost: u64,
}

impl Default for GasCosts {
    fn default() -> Self {
        Self {
            base_cost: default_gas_base_cost(),
            memory_cost: default_gas_memory_cost(),
            instruction_cost: default_gas_instruction_cost(),
            call_cost: default_gas_call_cost(),
            host_call_cost: default_gas_host_call_cost(),
            memory_page_cost: default_gas_memory_page_cost(),
            table_cost: default_gas_table_cost(),
        }
    }
}

impl ValidateConfig for GasCosts {
    fn validate(&self) -> Result<()> {
        // All gas costs must be non-zero
        if self.base_cost == 0 {
            return Err(validation_error("gas base_cost cannot be zero"));
        }
        if self.memory_cost == 0 {
            return Err(validation_error("gas memory_cost cannot be zero"));
        }
        if self.instruction_cost == 0 {
            return Err(validation_error("gas instruction_cost cannot be zero"));
        }
        if self.call_cost == 0 {
            return Err(validation_error("gas call_cost cannot be zero"));
        }
        if self.host_call_cost == 0 {
            return Err(validation_error("gas host_call_cost cannot be zero"));
        }
        if self.memory_page_cost == 0 {
            return Err(validation_error("gas memory_page_cost cannot be zero"));
        }
        if self.table_cost == 0 {
            return Err(validation_error("gas table_cost cannot be zero"));
        }

        Ok(())
    }
}

// Default value functions
fn default_max_memory() -> usize {
    DEFAULT_MAX_MEMORY
}

fn default_max_execution_time() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_MAX_EXECUTION_TIME_SECS)
}

fn default_stack_size() -> usize {
    DEFAULT_STACK_SIZE
}

fn default_table_size() -> u32 {
    DEFAULT_TABLE_SIZE
}

fn default_fuel_limit() -> u64 {
    DEFAULT_FUEL_LIMIT
}

fn default_compilation_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_COMPILATION_TIMEOUT_SECS)
}

fn default_memory_pages_limit() -> u32 {
    DEFAULT_MEMORY_PAGES_LIMIT
}

fn default_instance_pool_size() -> usize {
    DEFAULT_INSTANCE_POOL_SIZE
}

fn default_allowed_syscalls() -> Vec<String> {
    vec![
        "fd_read".to_string(),
        "fd_write".to_string(),
        "fd_close".to_string(),
        "fd_seek".to_string(),
        "path_open".to_string(),
        "path_create_directory".to_string(),
        "path_remove_directory".to_string(),
        "path_unlink_file".to_string(),
        "path_rename".to_string(),
        "clock_time_get".to_string(),
        "clock_res_get".to_string(),
        "poll_oneoff".to_string(),
        "random_get".to_string(),
        "proc_exit".to_string(),
        "proc_raise".to_string(),
        "sched_yield".to_string(),
    ]
}

fn default_allowed_host_functions() -> Vec<String> {
    vec![
        "env.log".to_string(),
        "env.http_request".to_string(),
        "env.db_query".to_string(),
        "env.crypto_hash".to_string(),
        "env.get_time".to_string(),
        "env.random_bytes".to_string(),
    ]
}

fn default_simd_enabled() -> bool {
    true
}

fn default_threads_enabled() -> bool {
    false // Disabled by default for security
}

fn default_bulk_memory_enabled() -> bool {
    true
}

fn default_reference_types_enabled() -> bool {
    true
}

fn default_multi_value_enabled() -> bool {
    true
}

fn default_module_caching_enabled() -> bool {
    true
}

fn default_strict_validation() -> bool {
    true
}

fn default_max_module_size() -> usize {
    100 * 1024 * 1024 // 100 MB
}

fn default_max_globals() -> u32 {
    10000
}

fn default_max_functions() -> u32 {
    100000
}

fn default_max_data_segments() -> u32 {
    100000
}

fn default_gas_base_cost() -> u64 {
    1
}

fn default_gas_memory_cost() -> u64 {
    2
}

fn default_gas_instruction_cost() -> u64 {
    1
}

fn default_gas_call_cost() -> u64 {
    10
}

fn default_gas_host_call_cost() -> u64 {
    100
}

fn default_gas_memory_page_cost() -> u64 {
    1000
}

fn default_gas_table_cost() -> u64 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_config_default() {
        let config = WasmConfig::default();
        assert_eq!(config.max_memory, 512 * 1024 * 1024);
        assert_eq!(config.stack_size, 1024 * 1024);
        assert_eq!(config.table_size, 10000);
        assert_eq!(config.fuel_limit, 10_000_000_000);
        assert_eq!(config.memory_pages_limit, 8192);
        assert_eq!(config.instance_pool_size, 10);
        assert!(config.simd_enabled);
        assert!(!config.threads_enabled);
        assert!(config.bulk_memory_enabled);
        assert!(config.module_caching_enabled);
        assert!(config.strict_validation);
    }

    #[test]
    fn test_wasm_config_validate_success() {
        let config = WasmConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_wasm_config_validate_zero_memory() {
        let mut config = WasmConfig::default();
        config.max_memory = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_memory"));
    }

    #[test]
    fn test_wasm_config_validate_memory_too_small() {
        let mut config = WasmConfig::default();
        config.max_memory = 1024; // Less than 1 MB
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_memory"));
    }

    #[test]
    fn test_wasm_config_validate_stack_exceeds_memory() {
        let mut config = WasmConfig::default();
        config.max_memory = 1024 * 1024; // 1 MB
        config.stack_size = 2 * 1024 * 1024; // 2 MB
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("stack_size"));
    }

    #[test]
    fn test_wasm_config_validate_execution_time_too_long() {
        let mut config = WasmConfig::default();
        config.max_execution_time = std::time::Duration::from_secs(301);
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_execution_time"));
    }

    #[test]
    fn test_wasm_config_validate_zero_table_size() {
        let mut config = WasmConfig::default();
        config.table_size = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("table_size"));
    }

    #[test]
    fn test_wasm_config_validate_table_size_too_large() {
        let mut config = WasmConfig::default();
        config.table_size = 100001;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("table_size"));
    }

    #[test]
    fn test_gas_costs_default() {
        let costs = GasCosts::default();
        assert_eq!(costs.base_cost, 1);
        assert_eq!(costs.memory_cost, 2);
        assert_eq!(costs.instruction_cost, 1);
        assert_eq!(costs.call_cost, 10);
        assert_eq!(costs.host_call_cost, 100);
        assert_eq!(costs.memory_page_cost, 1000);
        assert_eq!(costs.table_cost, 5);
    }

    #[test]
    fn test_gas_costs_validate_success() {
        let costs = GasCosts::default();
        assert!(costs.validate().is_ok());
    }

    #[test]
    fn test_gas_costs_validate_zero() {
        let mut costs = GasCosts::default();
        costs.base_cost = 0;
        
        let result = costs.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("base_cost"));
    }

    #[test]
    fn test_wasm_config_serialize() {
        let config = WasmConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("max_memory"));
        assert!(toml.contains("allowed_syscalls"));
    }

    #[test]
    fn test_wasm_config_deserialize() {
        let toml = r#"
            max_memory = 1073741824
            stack_size = 2097152
            simd_enabled = false
            threads_enabled = true
        "#;
        let config: WasmConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_memory, 1073741824);
        assert_eq!(config.stack_size, 2097152);
        assert!(!config.simd_enabled);
        assert!(config.threads_enabled);
    }
}
