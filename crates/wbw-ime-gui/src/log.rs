//! GUI 进程的固定路径诊断日志。
//!
//! `windows_subsystem = "windows"` 下没有控制台，`eprintln!` 无处输出。
//! 这里把日志追加写入 `%TEMP%\wbwime_gui.log`（带 `[pid]` 前缀），
//! 供排查候选窗口显示 / 按键截获 / 上屏等运行时问题。写失败静默忽略，
//! 绝不影响正常功能（日志仅用于诊断）。

use std::io::Write;
use std::sync::Mutex;

/// 每个进程一次的日志文件句柄（懒初始化，持锁写入）。
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// 写一行日志；失败静默忽略。
pub fn log(msg: &str) {
    let mut guard = match FILE.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if guard.is_none() {
        let path = std::env::temp_dir().join("wbwime_gui.log");
        *guard = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .ok();
    }
    if let Some(f) = guard.as_mut() {
        let _ = writeln!(f, "[pid={}] {}", std::process::id(), msg);
        let _ = f.flush();
    }
}

/// 用 `format_args!` 结构写一行日志（隐式捕获调用处的变量）。
#[macro_export]
macro_rules! logf {
    ($($arg:tt)*) => {
        $crate::log::log(&format!($($arg)*))
    };
}
