//! TSF 侧的候选窗口 IPC 客户端。
//!
//! DNSB（作为客户端）连接独立候选窗口进程 `wbw-ime-gui`（`--ipc` 模式，
//! localhost TCP 服务端），向其发送 [`ToGui::Show` / `ToGui::Hide`]，
//! 并读取用户点击回传的 [`ToDll`]。
//!
//! 点击选词/翻页在后台线程里直接操作全局 `IME_STATE` 并走剪贴板上屏
//!（`clipboard_paste` 是线程安全的，不依赖 TSF 编辑会话）。

use std::io::BufReader;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use wbw_ime_ipc::{frame, ToDll, ToGui, PORT};

/// 当前到 GUI 的连接（满双工）。
static STREAM: Mutex<Option<TcpStream>> = Mutex::new(None);
/// 是否已尝试过启动 GUI 进程（每会话仅一次）。
static LAUNCHED: AtomicBool = AtomicBool::new(false);

/// 后台读取线程的退出信号（复用 ToDll 的 None）。
static READER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 解析出 GUI 可执行文件路径（与 DNSB 同目录的 `wbw-ime-gui.exe`）。
///
/// 先按 DNSB 自身模块路径解析；失败时回退到当前工作目录。
fn gui_exe_path() -> Option<std::path::PathBuf> {
    unsafe {
        let handle =
            windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null());
        if handle.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
            handle,
            buf.as_mut_ptr(),
            buf.len() as u32,
        );
        if len == 0 {
            return None;
        }
        let exe_path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
        let dir = exe_path.parent()?;
        let gui = dir.join("wbw-ime-gui.exe");
        if gui.exists() {
            Some(gui)
        } else {
            None
        }
    }
}

/// 确保已连接 GUI。若未连接：先尝试启动 GUI 进程（每会话一次），再连接。
fn ensure_connected() -> bool {
    if STREAM.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
        return true;
    }
    if !LAUNCHED.swap(true, Ordering::SeqCst) {
        if let Some(exe) = gui_exe_path() {
            let mut cmd = std::process::Command::new(&exe);
            cmd.arg("--ipc");
            // 同目录有 gui-config.yaml 则传入作为主题；否则用默认主题
            if let Some(dir) = exe.parent() {
                let cfg = dir.join("gui-config.yaml");
                if cfg.exists() {
                    cmd.arg(cfg.to_string_lossy().to_string());
                }
            }
            let _ = cmd.spawn();
        }
    }

    let addr = match ("127.0.0.1", PORT).to_socket_addrs() {
        Ok(mut addrs) => match addrs.next() {
            Some(a) => a,
            None => return false,
        },
        Err(_) => return false,
    };
    // GUI 启动需要一点时间，简单重试若干次
    for attempt in 0..20 {
        match TcpStream::connect(addr) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(
                    wbw_ime_ipc::TIMEOUT_MS * 4,
                )));
                spawn_reader(match stream.try_clone() {
                    Ok(s) => s,
                    Err(e) => {
                        crate::log::log(&format!("ensure_connected: try_clone failed: {}", e));
                        return false;
                    }
                });
                *STREAM.lock().unwrap_or_else(|e| e.into_inner()) = Some(stream);
                return true;
            }
            Err(_) => {
                if attempt >= 19 {
                    return false;
                }
                std::thread::sleep(std::time::Duration::from_millis(50 + attempt * 25));
            }
        }
    }
    false
}

/// 是否已尝试启动钩子兜底 GUI（每进程一次）。
static HOOK_LAUNCHED: AtomicBool = AtomicBool::new(false);

/// 启动钩子兜底模式 GUI（`--hook`，自行捕获键盘+上屏，无需 TSF 按键通道）。
/// 仅当 `ITfKeystrokeMgr` 不可用时由调用方触发；单实例由 GUI 侧命名 Mutex 保证。
pub fn launch_hook_gui() {
    if HOOK_LAUNCHED.swap(true, Ordering::SeqCst) {
        return;
    }
    let Some(exe) = gui_exe_path() else {
        crate::log::log("launch_hook_gui: no gui exe");
        return;
    };
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--hook");
    if let Some(dir) = exe.parent() {
        let cfg = dir.join("gui-config.yaml");
        if cfg.exists() {
            cmd.arg(cfg.to_string_lossy().to_string());
        }
    }
    crate::log::log(&format!("launch_hook_gui: spawn {}", exe.display()));
    let _ = cmd.spawn();
}

/// 启动后台线程读取 GUI 点击回传。
fn spawn_reader(stream: TcpStream) {
    if READER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        while let Ok(Some(msg)) = frame::read::<ToDll>(&mut reader) {
            handle_to_dll(msg);
        }
        {
            let mut stream = STREAM.lock().unwrap_or_else(|e| e.into_inner());
            *stream = None;
        }
        READER_RUNNING.store(false, Ordering::SeqCst);
    });
}

/// 处理 GUI 回传的一条点击消息。
fn handle_to_dll(msg: ToDll) {
    match msg {
        ToDll::Select(idx) => {
            let text = {
                let mut guard = match crate::text_service::IME_STATE.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                let state = match guard.as_mut() {
                    Some(s) => s,
                    None => return,
                };
                state.select_commit(idx);
                state.commit_text.take()
            };
            if let Some(text) = text {
                crate::output::clipboard_paste(&text);
            }
        }
        ToDll::ToggleMode => {
            if let Ok(mut guard) = crate::text_service::IME_STATE.lock() {
                if let Some(state) = guard.as_mut() {
                    state.toggle_chinese();
                }
            }
            refresh_gui();
        }
        ToDll::PageUp | ToDll::PageDown => {
            let changed = {
                let mut guard = match crate::text_service::IME_STATE.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                let state = match guard.as_mut() {
                    Some(s) => s,
                    None => return,
                };
                match msg {
                    ToDll::PageUp => {
                        state.prev_page();
                        state.page > 0
                    }
                    ToDll::PageDown => {
                        let before = state.page;
                        state.next_page();
                        before != state.page
                    }
                    _ => false,
                }
            };
            // 翻页后刷新 GUI 的候选显示
            if changed {
                refresh_gui();
            }
        }
    }
}

/// 构造并发送一条候选窗口显示消息给 GUI（尽力而为，不阻塞按键主路径太久）。
pub fn refresh_gui() {
    if !ensure_connected() {
        return;
    }
    let msg = {
        let guard = match crate::text_service::IME_STATE.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let state = match guard.as_ref() {
            Some(s) => s,
            None => return,
        };
        if !state.composing || state.buffer.is_empty() {
            ToGui::Hide
        } else {
            let candidates: Vec<String> =
                state.candidates.iter().map(|c| c.text.clone()).collect();
            let (x, y) = {
                let tm = current_thread_mgr();
                if tm.is_null() {
                    fallback_coords()
                } else {
                    unsafe { crate::output::get_caret_screen_coords(tm) }
                        .unwrap_or_else(fallback_coords)
                }
            };
            ToGui::Show {
                buffer: state.buffer.clone(),
                candidates,
                selected: state.selected_index,
                page: state.page,
                total_pages: state.total_pages(),
                x,
                y,
                mode: if state.chinese_mode { "中".into() } else { "英".into() },
            }
        }
    };

    let mut guard = match STREAM.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(stream) = guard.as_mut() {
        if frame::write(stream, &msg).is_err() {
            *guard = None;
        }
    }
}

/// 当前会话激活的 TextService 持有的 thread_mgr 指针。
fn current_thread_mgr() -> *mut std::ffi::c_void {
    crate::text_service::current_tsf_ctx().0
}

/// 拿不到光标坐标时的兜底位置（主显示器右下角上方）。
fn fallback_coords() -> (i32, i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, *};
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vcx = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let vcy = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let x = vx + vcx - 320;
        let y = vy + vcy - 120;
        (x.max(vx), y.max(vy))
    }
}
