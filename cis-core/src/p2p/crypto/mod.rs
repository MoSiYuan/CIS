//! P2P 加密模块
//!
//! 提供 Noise Protocol 加密和 Ed25519/X25519 密钥管理。

pub mod keys;
pub mod noise;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod noise_tests;
