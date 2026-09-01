//! 简易调试日志：写入临时目录，用于定位 TSF 宿主崩溃点。
//!
//! TSF 调试难点：DLL 会被多个进程加载，且崩溃时无法用断点。
//! 这里把日志写到 `%TEMP%\wbwime_tsf.log`。
//!
//! 注意：`DllMain` 在 Windows 加载器锁下执行，绝不能在此做文件 IO 或
//! 环境变量读取（`std::env::var` 可能分配/加锁导致死锁）。因此 `DllMain`
//! 里不调用 [`log`]。为避免风险，DllMain 阶段不做日志。

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

static ENABLED: AtomicBool = AtomicBool::new(true);

/// 缓存日志路径（只计算一次，避免每次按键都分配 PathBuf）。
fn log_path() -> &'static std::path::Path {
    static PATH: OnceLock<std::path::PathBuf> = OnceLock::new();
    PATH.get_or_init(|| std::env::temp_dir().join("wbwime_tsf.log"))
}

/// 缓存文件句柄（保持打开，避免每次按键都重新 open + flush）。
/// 初始化失败时返回 None，禁用日志而非 panic。
fn log_file() -> Option<&'static Mutex<std::fs::File>> {
    static FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    FILE.get_or_init(|| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path())
            .ok()
            .map(Mutex::new)
    })
    .as_ref()
}

pub fn log(msg: &str) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let pid = std::process::id();
    if let Some(f) = log_file() {
        if let Ok(mut guard) = f.lock() {
            let _ = writeln!(guard, "[pid={pid}] {msg}");
            let _ = guard.flush();
        }
    }
}

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}
