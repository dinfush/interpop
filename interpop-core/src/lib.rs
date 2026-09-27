//! # interpop-core
//!
//! `interpop-core` 是 interpop 微内核图形伺服器的核心抽象与调度基础库。
//!
//! ## 核心职责
//! - 定义 L0 顶层 Wire 协议与强类型 IPC 契约（`contract` 模块）
//! - 提供画布生命周期状态机与纯函数单漏斗调度机制（PR-02+）
//! - 统合 Wayland 连接与基于 Calloop 的单事件循环（PR-03+）
//!
//! ## 纯安全与架构红线
//! - 全库强制启用 `#![forbid(unsafe_code)]`，应用层状态机 100% 封闭在纯 Rust 内存安全域内。
//! - 遵循参数透传同构铁律：本 crate 契约类型为 wire 协议形态的权威载体（类型即协议）；
//!   若与上位规范（白皮书 / IF-BluePrint）发生冲突，严格按三级分流法则修订代码，上位文档永不去改。

#![forbid(unsafe_code)]
#![deny(missing_docs)]

/// L0 契约层：顶层动词、下行事件与共享词汇类型的唯一落点（IF-BluePrint 第 3 章）。
///
/// 类型即协议——serde 派生直接产出 wire 格式，Phase 1/2 scaffold 与 Phase 3 真实
/// Socket 共用同一组类型（参数透传同构铁律，白皮书 §2）。
pub mod contract;