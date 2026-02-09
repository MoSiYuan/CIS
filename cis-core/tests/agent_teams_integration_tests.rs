//! # Agent Teams 集成测试套件
//!
//! 本测试套件提供 Agent Teams 系统的全面集成测试覆盖。
//!
//! ## 测试模块
//!
//! - `pool_tests`: Agent Pool 功能测试
//! - `executor_tests`: DAG 执行器测试
//! - `federation_tests`: 联邦/跨节点功能测试（模拟）
//!
//! ## 运行测试
//!
//! ```bash
//! # 运行所有 Agent Teams 测试
//! cargo test --test agent_teams_integration_tests
//!
//! # 运行特定模块
//! cargo test --test agent_teams_integration_tests pool_tests
//! cargo test --test agent_teams_integration_tests executor_tests
//! cargo test --test agent_teams_integration_tests federation_tests
//!
//! # 显示输出
//! cargo test --test agent_teams_integration_tests -- --nocapture
//! ```

mod agent_teams;

// 测试模块通过 mod.rs 自动包含
// - agent_teams::pool_tests
// - agent_teams::executor_tests
// - agent_teams::federation_tests
