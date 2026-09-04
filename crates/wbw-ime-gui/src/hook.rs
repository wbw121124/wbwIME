//! 键盘钩子兜底模式（`--hook`）。
//!
//! 适用于 TSF `ITfKeystrokeMgr` 拿不到的宿主：本进程安装一个全局低级键盘钩子
//! (`WH_KEYBOARD_LL`)，由独立线程泵消息、回调把按键翻译成引擎事件；
//! 引擎确认的文本走剪贴板 + 模拟 `Ctrl+V` 上屏；候选窗口跟随光标显示。
//!
//! 键位门控：仅当前台线程键盘布局为中文（`LANG_CHINESE`）时接管字母/数字/功能键，
//! 避免在英文布局下吞掉正常输入。Shift 键可手动切换中文模式作为覆盖。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_ALREADY_EXISTS, HWND, LRESULT, LPARAM, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayout, HKL};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetForegroundWindow,
    GetWindowThreadProcessId, KBDLLHOOKSTRUCT, PostQuitMessage, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HHOOK, HC_ACTION, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN,
    WM_KEYUP, WM_QUIT,
};

use crate::engine::GuiState;

const WH_KEYBOARD_LL_CODE: i32 = WH_KEYBOARD_LL;
const HC_ACTION_CODE: i32 = HC_ACTION as i32;

const VK_BACK: u32 = 0x08;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_NUMPAD0: u32 = 0x60;
const VK_NUMPAD9: u32 = 0x69;

/// 中文模式手动覆盖（Shift 键切换）。false = 跟随系统布局，true = 强制中文模式。
pub static CHINESE_MODE: AtomicBool = AtomicBool::new(false);

/// 每个候选窗回调：处理一次按键，返回本次状态（供 UI 线程应用 + 决定是否吞键）。
type KeyHandler = Box<dyn Fn(u32, Option<char>) -> GuiState + Send + Sync>;

static HANDLER: OnceLock<Mutex<Option<KeyHandler>>> = OnceLock::new();
/// 已被本进程吞掉、仍处于按下状态的键（keyup 也吞掉，防止应用端配对错乱）。
static EATEN_DOWN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

/// 单一实例守卫（命名 Mutex 句柄，进程存活期间持有）。
static INSTANCE_HANDLE: OnceLock<usize> = OnceLock::new();

/// 安装键盘钩子并开始泵消息。`handler` 在钩子回调线程上执行。
pub fn start(handler: KeyHandler) {
    let _ = HANDLER.set(Mutex::new(Some(handler)));
    std::thread::spawn(run_hook_thread);
}

/// 钩子线程：安装 `WH_KEYBOARD_LL` 并持续 GetMessage 泵消息。
fn run_hook_thread() {
    unsafe {
        let hmod = GetModuleHandleW(std::ptr::null());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL_CODE, Some(ll_keyboard_proc), hmod, 0);
        if hook.is_null() {
            eprintln!("[hook] SetWindowsHookExW failed");
            return;
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, HWND::default(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if msg.message == WM_QUIT {
                break;
            }
        }
        UnhookWindowsHookEx(hook);
    }
}

/// 主持钩子的线程 ID（识别前台线程是否仍是本钩子线程，用于布局门控）。
static HOOK_THREAD_ID: Mutex<Option<u32>> = Mutex::new(None);
/// 每帧是否调用 force_exit_if_idle（低频检查，防止非中文布局下长期驻留）。
static EXIT_CHECK_TICK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// 低级键盘钩子回调。
unsafe extern "system" fn ll_keyboard_proc(
    code: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if code != HC_ACTION_CODE {
            return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
        }
        let kbd = lparam as *const KBDLLHOOKSTRUCT;
        if kbd.is_null() {
            return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
        }
        let info = &*kbd;
        // 忽略注入事件（防止我们模拟的 Ctrl+V 递归）
        if info.flags & LLKHF_INJECTED != 0 {
            return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
        }

        // 低频检查：若当前非中文模式且无吞键，允许钩子自行退出
        let tick = EXIT_CHECK_TICK.fetch_add(1, Ordering::Relaxed);
        if tick.is_multiple_of(30) {
            force_exit_if_idle();
        }

        // 同步本钩子线程 ID（仅占位，保留后续诊断能力）
        {
            let mut slot = HOOK_THREAD_ID.lock().unwrap_or_else(|e| e.into_inner());
            if slot.is_none() {
                let tid = windows_sys::Win32::System::Threading::GetCurrentThreadId();
                *slot = Some(tid);
            }
        }

        match wparam as u32 {
            WM_KEYDOWN => {
                let vkey = info.vkCode;
                // Ctrl / Alt 不拦截，透传给应用
                if vkey == VK_CONTROL || vkey == VK_MENU {
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }
                // Shift 键切换中文模式（composing 时不切换，但仍然透传给引擎处理）
                if vkey == VK_SHIFT {
                    let eating = !EATEN_DOWN.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
                    if !eating {
                        let old = CHINESE_MODE.load(Ordering::Acquire);
                        CHINESE_MODE.store(!old, Ordering::Release);
                        crate::logf!("hook Shift toggle chinese_mode={}", !old);
                    }
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }
                let chinese = is_chinese_foreground();
                let eating = !EATEN_DOWN.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
                if !chinese && !eating {
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }
                if chinese && eating {
                    crate::logf!("hook keydown vk={} chinese=true eating=true", info.vkCode);
                }
        // Ctrl / Alt 组合键始终透传（即使在中英混合状态）
                let ctrl_pressed = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL as i32) as i16) < 0;
                let alt_pressed = (windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_MENU as i32) as i16) < 0;
                if ctrl_pressed || alt_pressed {
                    return CallNextHookEx(HHOOK::default(), code, wparam, lparam);
                }
                if chinese || eating {
                    let rotated = translate(&info.vkCode);
                    if let Some((code, ch)) = rotated {
                    let eaten = process_key(code, ch);
                    if eaten {
                        let mut eaten = EATEN_DOWN.lock().unwrap_or_else(|e| e.into_inner());
                        if !eaten.contains(&info.vkCode) {
                            eaten.push(info.vkCode);
                        }
                        return 1;
                    }
                    }
                }
                CallNextHookEx(HHOOK::default(), code, wparam, lparam)
            }
            WM_KEYUP => {
                let mut eaten = EATEN_DOWN.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(pos) = eaten.iter().position(|v| *v == info.vkCode) {
                    eaten.remove(pos);
                    return 1;
                }
                CallNextHookEx(HHOOK::default(), code, wparam, lparam)
            }
            _ => CallNextHookEx(HHOOK::default(), code, wparam, lparam),
        }
    }
}

/// 低频检查：钩子线程每帧（由 tick 驱动）调用此函数，当非中文模式且无吞键时触发退出。
/// 检查前台窗口的系统键盘布局是否为中文。
fn is_system_chinese_layout() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return false;
        }
        let mut pid = 0u32;
        let tid = GetWindowThreadProcessId(hwnd, &mut pid);
        if tid == 0 {
            return false;
        }
        let hkl: HKL = GetKeyboardLayout(tid);
        let langid = (hkl as usize as u32) & 0xffff;
        let primary = langid & 0x3ff;
        // LANG_CHINESE_SIMPLIFIED (0x04) 或 LANG_CHINESE_TRADITIONAL (0x1C)
        primary == 0x04 || primary == 0x1C
    }
}

/// 中文模式判断：系统键盘布局为中文，或手动 Shift 覆盖已开启。
fn is_chinese_foreground() -> bool {
    if CHINESE_MODE.load(Ordering::Acquire) {
        return true;
    }
    is_system_chinese_layout()
}

/// 主动退出钩子：若当前非强制中文模式且无吞键，发送 WM_QUIT 终止钩子线程。
pub fn force_exit_if_idle() {
    if !CHINESE_MODE.load(Ordering::Acquire) {
        let eaten = EATEN_DOWN.lock().unwrap_or_else(|e| e.into_inner());
        if eaten.is_empty() {
            unsafe { PostQuitMessage(0); }
        }
    }
}

/// 每帧低频检查是否应退出（在钩子回调中周期性调用）。
/// 将虚拟键码翻译为引擎按键。
fn translate(vk: &u32) -> Option<(u32, Option<char>)> {
    let vk = *vk;
    match vk {
        VK_BACK => Some((8, None)),
        VK_RETURN => Some((13, None)),
        VK_ESCAPE => Some((27, None)),
        VK_SPACE => Some((32, None)),
        VK_PRIOR => Some((33, None)),
        VK_NEXT => Some((34, None)),
        VK_LEFT => Some((37, None)),
        VK_UP => Some((38, None)),
        VK_RIGHT => Some((39, None)),
        VK_DOWN => Some((40, None)),
        0x41..=0x5A => {
            // 字母：小写
            let c = (vk + 0x20) as u8 as char;
            Some((vk, Some(c)))
        }
        0x30..=0x39 => {
            let c = vk as u8 as char;
            Some((vk, Some(c)))
        }
        VK_SHIFT => Some((VK_SHIFT, None)),
        VK_NUMPAD0..=VK_NUMPAD9 => {
            let c = (b'0' + (vk - VK_NUMPAD0) as u8) as char;
            Some((vk - VK_NUMPAD0 + 0x30, Some(c)))
        }
        _ => None,
    }
}

/// 处理一次按键：调用引擎，按其结果决定是否吞键。
fn process_key(code: u32, ch: Option<char>) -> bool {
    let mut guard = match HANDLER.get() {
        Some(h) => match h.lock() {
            Ok(g) => g,
            Err(_) => return false,
        },
        None => return false,
    };
    let Some(handler) = guard.as_mut() else {
        return false;
    };
    let state = handler(code, ch);
    // 吞键条件：仍在组合（窗口可见）或本次确认了文本
    state.visible || state.committed.is_some()
}

/// 通用单实例守卫：为给定 tag 创建命名 Mutex；若已存在同名 Mutex（另有实例在跑）返回 None。
/// 修改原因：提取公共逻辑，供 hook / ipc 两种模式复用。
fn acquire_single_instance_for(tag: &str) -> Option<()> {
    unsafe {
        let name = encode_wide(&format!("Local\\wbwIME_gui_{tag}"));
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return None;
        }
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return None;
        }
        // 首次创建：持有句柄直到进程退出（存放到 static 防被释放）
        INSTANCE_HANDLE.set(handle as usize).ok();
        Some(())
    }
}

/// 钩子模式单实例守卫（`--hook`）。
pub fn acquire_single_instance() -> Option<()> {
    acquire_single_instance_for("hook")
}

/// IPC 模式单实例守卫（`--ipc`）：全系统仅允许一个候选窗口 GUI 进程。
pub fn acquire_single_instance_ipc() -> Option<()> {
    acquire_single_instance_for("ipc")
}

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 引擎状态 → 交回调用方（UI 线程通过 SameThread channel 应用）。
pub type StateSender = Sender<GuiState>;
