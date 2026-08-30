//! 简易调试日志：写入固定目录，用于定位 TSF 宿主（VSCode/explorer）崩溃点。
//!
//! TSF 调试难点：DLL 会被多个进程加载，且崩溃时无法用断点。
//! 这里把日志写到 `C:\Users\wbw\AppData\Local\Temp\wbwime_tsf.log`。
//!
//! 注意：`DllMain` 在 Windows 加载器锁下执行，绝不能在此做文件 IO 或
//! 环境变量读取（`std::env::var` 可能分配/加锁导致死锁）。因此 `DllMain`
//! 里不调用 [`log`]，只调用 `log_raw`（写固定路径，但仍可能死锁）。
//! 为避免风险，DllMain 阶段不做日志，依靠 DllGetClassObject 等入口点记录。

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(true);

fn log_path() -> std::path::PathBuf {
    std::path::PathBuf::from(r"C:\Users\wbw\AppData\Local\Temp\wbwime_tsf.log")
}

pub fn log(msg: &str) {
    if !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let pid = std::process::id();
    let path = log_path();
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(f, "[pid={pid}] {msg}");
        let _ = f.flush();
    }
}

pub fn set_enabled(v: bool) {
    ENABLED.store(v, Ordering::Relaxed);
}
