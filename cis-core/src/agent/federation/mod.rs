//! # Agent Federation 模块
//!
//! 提供跨节点 Agent 通信能力，支持 CIS 节点间的 Agent 联邦。
//!
//! ## 功能特性
//!
//! - **FederatedAgent**: 包装本地 Agent 或作为远程 Agent 代理
//! - **FederatedRuntime**: 管理和创建联邦 Agent
//! - **Agent Federation Protocol**: 定义跨节点通信协议
//!
//! ## 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                  Agent Federation                           │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────────┐      ┌──────────────────────────────┐  │
//! │  │  FederatedAgent │      │  Agent Federation Protocol   │  │
//! │  │                 │      │                              │  │
//! │  │  ┌───────────┐  │      │  - AgentFederationEvent      │  │
//! │  │  │  Local    │  │      │  - AgentAddress              │  │
//! │  │  │  Agent    │  │      │  - TaskRequestPayload        │  │
//! │  │  └───────────┘  │      │  - TaskResultPayload         │  │
//! │  │       OR        │      │  - AgentRoutingTable         │  │
//! │  │  ┌───────────┐  │      │                              │  │
//! │  │  │  Remote   │  │      └──────────────────────────────┘  │
//! │  │  │  Proxy    │  │                                        │
//! │  │  └───────────┘  │      ┌──────────────────────────────┐  │
//! │  └─────────────────┘      │   Matrix Federation Client   │  │
//! │                           │                              │  │
//! │  ┌─────────────────┐      │  - CisMatrixEvent            │  │
//! │  │ FederatedRuntime│      │  - FederationClient          │  │
//! │  └─────────────────┘      └──────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 使用示例
//!
//! ### 创建联邦 Agent（包装本地 Agent）
//!
//! ```rust,ignore
//! use cis_core::agent::federation::{FederatedAgent, FederatedRuntime};
//! use cis_core::agent::persistent::{AgentConfig, AgentRuntime};
//! use cis_core::matrix::federation::FederationClient;
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 创建 Matrix 联邦客户端
//! let matrix_client = Arc::new(FederationClient::new()?);
//!
//! // 创建联邦 Runtime
//! let runtime = FederatedRuntime::new(matrix_client, "!agent-federation:local");
//!
//! // 创建联邦 Agent
//! let config = AgentConfig::new("my-agent", std::env::temp_dir());
//! let agent = runtime.create_agent(config).await?;
//!
//! // 执行任务
//! let result = agent.execute(TaskRequest::new("task-1", "Hello")).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### 创建远程 Agent 代理
//!
//! ```rust,ignore
//! use cis_core::agent::federation::FederatedAgent;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let proxy = FederatedAgent::remote_proxy(
//!     "remote-agent-id",
//!     "remote-node-id",
//!     matrix_client,
//!     room_id,
//! ).await?;
//!
//! // 通过代理执行任务
//! let result = proxy.execute(task).await?;
//! # Ok(())
//! # }
//! ```

pub mod agent;
pub mod manager;
pub mod protocol;

#[cfg(test)]
mod manager_tests;

#[cfg(test)]
mod task_tests;

// Re-export 主要类型
pub use agent::{FederatedAgent, FederatedRuntime};
pub use manager::{FederationManager, FederatedAddress, FederationTaskRequest, FederationTaskResult};
pub use protocol::{
    AgentAddress, AgentFederationEvent, AgentFederationRoom, AgentRoute, AgentRoutingTable,
    TaskRequestPayload, TaskResultPayload,
};
