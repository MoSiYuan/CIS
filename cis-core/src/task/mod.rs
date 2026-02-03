//! # Task 模块
//!
//! 提供 Task 管理和向量索引功能。
//!
//! ## 主要组件
//!
//! - `vector::TaskVectorIndex`: Task 向量索引，支持多字段语义搜索

pub mod vector;

pub use vector::{
    TaskVectorIndex, 
    TaskSearchResult, 
    TaskSimilarity, 
    TaskField,
};
