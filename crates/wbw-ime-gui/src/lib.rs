//! wbwIME GUI crate
//!
//! 提供基于 Slint 渲染的候选词窗口。核心引擎（`engine`）与主题配置
//! （`config`）为纯 Rust 实现；UI 入口位于 `main.rs`（Slint 原生 Rust，无 Qt 依赖）。

pub mod config;
pub mod engine;
pub mod hook;
pub mod ipc;

pub use config::GuiConfig;
pub use engine::{GuiState, WbwIme};
