//! wbwIME GUI crate
//!
//! 提供基于 Qt/QML 渲染的候选词窗口。核心引擎（`engine`）与主题配置
//! （`config`）为纯 Rust 实现，可脱离 Qt 编译与测试；Qt 渲染入口位于
//! 由 `qt` feature 门控的 `main.rs`。

pub mod config;
pub mod engine;

pub use config::GuiConfig;
pub use engine::{GuiState, WbwIme};
