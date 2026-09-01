//! 键盘钩子兜底模式（`--hook`）。
//!
//! 适用于 TSF `ITfKeystrokeMgr` 拿不到的宿主：本进程安装一个全局低级键盘钩子
//! (`WH_KEYBOARD_LL`)，由独立线程泵消息、回调把按键翻译成引擎事件；
//! 引擎确认的文本走剪贴板 + 模拟 `Ctrl+V` 上屏；候选窗口跟随光标显示。
//!
//! 键位门控：仅当前台线程键盘布局为中文（`LANG_CHINESE`）时接管字母/数字/功能键，
//! 避免在英文布局下吞掉正常输入。

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HWND, LRESULT, LPARAM, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    KBDLLHOOKSTRUCT, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, HC_ACTION,
    LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT,
};

use crate::engine::GuiState;

const WH_KEYBOARD_LL_CODE: i32 = WH_KEYBOARD_LL as i32;
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
        let hook = SetWindowsHookExW(
            WH_KEYBOARD_LL_CODE,
            Some(ll_keyboard_proc),
            hmod,
            0,
        );
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
/// 低级键盘钩子回调。
unsafe extern "system" fn ll_keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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

        // 同步本钩子线程 ID（仅占位，保留后续诊断能力）
        {
            let mut slot = HOOK_THREAD_ID.lock().unwrap();
            if slot.is_none() {
                let tid = windows_sys::Win32::System::Threading::GetCurrentThreadId();
                *slot = Some(tid);
            }
        }

        match wparam as u32 {
            WM_KEYDOWN => {
                if is_chinese_foreground() || !EATEN_DOWN.lock().unwrap().is_empty() {
                    let rotated = translate(&info.vkCode);
                    match rotated {
                        Some((code, ch)) => {
                            let eaten = process_key(code, ch);
                            if eaten {
                                EATEN_DOWN.lock().unwrap().push(info.vkCode);
                                return 1;
                            }
                        }
                        None => {}
                    }
                }
                CallNextHookEx(HHOOK::default(), code, wparam, lparam)
            }
            WM_KEYUP => {
                let mut eaten = EATEN_DOWN.lock().unwrap();
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

/// 前台线程键盘布局是否为中文。
fn is_chinese_foreground() -> bool {
    unsafe {
        let fg: HWND = GetForegroundWindow();
        let mut pid: u32 = 0;
        let tid = GetWindowThreadProcessId(fg, &mut pid);
        if tid == 0 {
            // 保底：使用本钩子线程布局——若正在组合则继续接管
            return false;
        }
        let layout = GetKeyboardLayout(tid) as usize as u32;
        layout & 0x3FF == 0x04 // PRIMARYLANGID == LANG_CHINESE
    }
}

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

/// 返回单一实例守卫（进程存活期间持有一个已创建的命名 Mutex 手柄）。
/// 若已有同名 Mutex 存在（另一次运行中），返回 None，调用方应直接退出。
pub fn acquire_single_instance() -> Option<()> {
    unsafe {
        let name = encode_wide("Local\\wbwIME_gui_hook");
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

fn encode_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 引擎状态 → 交回调用方（UI 线程通过 SameThread channel 应用）。
pub type StateSender = Sender<GuiState>;