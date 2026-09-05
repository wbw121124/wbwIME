#![allow(clippy::missing_const_for_thread_local)]
use std::ffi::c_void;

use crate::guid::*;

pub type HRESULT = i32;
pub type ULONG = u32;

pub const S_OK: HRESULT = 0;

/// 屏幕坐标矩形（与 Win32 `RECT` 布局一致）。
#[repr(C)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

// ========== Helper: get context from thread_mgr ==========

unsafe fn get_context(thread_mgr: *mut c_void) -> Option<*mut c_void> {
    // 权威链路（msctf.idl）：ITfThreadMgr::GetFocus(=7) -> ITfDocumentMgr，
    // ITfDocumentMgr::GetTop(=6) -> ITfContext。
    let tm_vtable = *(thread_mgr as *const *const usize);
    let get_focus_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
        std::mem::transmute(*tm_vtable.add(7));
    let mut doc_mgr: *mut c_void = std::ptr::null_mut();
    if get_focus_fn(thread_mgr, &mut doc_mgr) != S_OK || doc_mgr.is_null() {
        return None;
    }

    let dm_vtable = *(doc_mgr as *const *const usize);
    let get_top_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
        std::mem::transmute(*dm_vtable.add(6));
    let mut context: *mut c_void = std::ptr::null_mut();
    let hr = get_top_fn(doc_mgr, &mut context);
    {
        let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(*dm_vtable.add(2));
        release_fn(doc_mgr);
    }
    if hr != S_OK || context.is_null() {
        return None;
    }
    Some(context)
}

unsafe fn release_obj(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }
    let vtable = *(obj as *const *const usize);
    let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
        std::mem::transmute(*vtable.add(2));
    release_fn(obj);
}

unsafe fn qi(obj: *mut c_void, iid: &Guid) -> Option<*mut c_void> {
    let vtable = *(obj as *const *const usize);
    let qi_fn: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT =
        std::mem::transmute(*vtable.add(0));
    let mut result: *mut c_void = std::ptr::null_mut();
    let hr = qi_fn(obj, iid, &mut result);
    if hr == S_OK && !result.is_null() {
        Some(result)
    } else {
        None
    }
}

/// `TF_SELECTION` —— `ITfContext::GetSelection` 的输出结构。
/// 这里只关心 range 指针（首个字段），其余由调用方保留。
#[repr(C)]
struct TSF_SELECTION {
    range: *mut c_void,
    style: u64,
}

const TF_DEFAULT_SELECTION: u32 = u32::MAX;
const TF_ES_SYNC: u32 = 0x0000_0002;
const TF_ES_READ: u32 = 0x0000_0004;
const TF_ES_WRITE: u32 = 0x0000_0008;

// 同步会话里备用的活动上下文指针（由 `get_caret_screen_coords` 在发请求前写入）。
thread_local! {
    static SESSION_CTX: std::cell::Cell<*mut c_void> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    static CARET_OUT: std::cell::Cell<Option<(i32, i32)>> = const { std::cell::Cell::new(None) };
    static SESSION_JOB: std::cell::RefCell<Option<SessionJob>> = const { std::cell::RefCell::new(None) };
    static INSERT_OUT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

enum SessionJob {
    Caret,
    Insert { wide: Vec<u16> },
}

// ===== ITfEditSession sink（接收同步会话回调，读取光标屏幕坐标） =====
unsafe extern "system" fn es_qi(
    _this: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if ppv.is_null() {
            return -2147024809;
        }
        let iid = unsafe { &*riid };
        if *iid == IID_IUNKNOWN {
            unsafe {
                *ppv = _this;
            }
            return S_OK;
        }
        unsafe {
            *ppv = std::ptr::null_mut();
        }
        -2147467263
    }))
    .unwrap_or(-2147467259)
}

unsafe extern "system" fn es_add_ref(_this: *mut c_void) -> ULONG {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| 1)).unwrap_or(0)
}

unsafe extern "system" fn es_release(_this: *mut c_void) -> ULONG {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| 0)).unwrap_or(0)
}

/// `ITfEditSession::DoEditSession(ec)` —— 在同步会话里用合法 cookie 执行请求的任务。
unsafe extern "system" fn es_do_edit_session(_this: *mut c_void, ec: u32) -> HRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let context = SESSION_CTX.with(|c| c.get());
        if context.is_null() {
            return S_OK;
        }

        let job = SESSION_JOB.with(|j| j.borrow_mut().take());
        match job {
            Some(SessionJob::Caret) => {
                es_do_caret(context, ec);
            }
            Some(SessionJob::Insert { wide }) => {
                es_do_insert(context, ec, wide);
            }
            None => {}
        }

        S_OK
    }))
    .unwrap_or(-2147467259)
}

/// 在只读会话里用合法 cookie 取插入光标屏幕坐标。
unsafe fn es_do_caret(context: *mut c_void, ec: u32) {
    unsafe {
        let ctx_vtable = *(context as *const *const usize);
        // ITfContext::GetSelection (index 5): (ec, ulIndex, pSelection, pcFetched)
        let get_sel_fn: unsafe extern "system" fn(
            *mut c_void,
            u32,
            u32,
            *mut TSF_SELECTION,
            *mut u32,
        ) -> HRESULT = std::mem::transmute(*ctx_vtable.add(5));

        let mut selection: TSF_SELECTION =
            TSF_SELECTION { range: std::ptr::null_mut(), style: 0 };
        let mut fetched: u32 = 0;
        let hr = get_sel_fn(
            context,
            TF_DEFAULT_SELECTION,
            1,
            &mut selection,
            &mut fetched,
        );
        if hr != S_OK || fetched == 0 || selection.range.is_null() {
            return;
        }

        let range = selection.range;

        // ITfContext::GetActiveView (index 9): (pView)
        let get_view_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(*ctx_vtable.add(9));
        let mut view: *mut c_void = std::ptr::null_mut();
        if get_view_fn(context, &mut view) == S_OK && !view.is_null() {
            let view_vtable = *(view as *const *const usize);
            // ITfContextView::GetTextExt (index 4): (ec, pRange, prc, pfClipped)
            let get_ext_fn: unsafe extern "system" fn(
                *mut c_void,
                u32,
                *mut c_void,
                *mut RECT,
                *mut i32,
            ) -> HRESULT = std::mem::transmute(*view_vtable.add(4));

            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let mut clipped: i32 = 0;
            let hr2 = get_ext_fn(view, ec, range, &mut rect, &mut clipped);
            if hr2 == S_OK {
                // 选区的起始点（插入光标）作为候选窗锚点。
                let (mut x, mut y) = (rect.left, rect.top);
                if x == 0 && y == 0 {
                    // 零宽选区有时返回 0 坐标，此时用视口右下近似。
                    x = rect.right;
                    y = rect.bottom;
                }
                CARET_OUT.with(|c| c.set(Some((x, y))));
            }
            release_obj(view);
        }

        release_obj(range);
    }
}

/// 在写会话里用合法 cookie 把文本插入到当前选区。
unsafe fn es_do_insert(context: *mut c_void, ec: u32, wide: Vec<u16>) {
    unsafe {
        let insert_sel = match qi(context, &IID_ITF_INSERT_AT_SELECTION) {
            Some(p) => p,
            None => {
                INSERT_OUT.with(|o| o.set(false));
                return;
            }
        };

        let insert_vtable = *(insert_sel as *const *const usize);
        // ITfInsertAtSelection::InsertTextAtSelection (index 3):
        //   (ec, dwFlags, pchText, cch[LONG], ppRange[ITfRange **])
        let insert_fn: unsafe extern "system" fn(
            *mut c_void,
            u32,
            u32,
            *const u16,
            i32,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(*insert_vtable.add(3));

        let mut range: *mut c_void = std::ptr::null_mut();
        let hr = insert_fn(
            insert_sel,
            ec,
            0,
            wide.as_ptr(),
            wide.len() as i32,
            &mut range,
        );
        if !range.is_null() {
            release_obj(range);
        }

        release_obj(insert_sel);
        INSERT_OUT.with(|o| o.set(hr == S_OK));
    }
}

#[repr(C)]
struct EditSessionVtable {
    query_interface: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    do_edit_session: unsafe extern "system" fn(*mut c_void, u32) -> i32,
}

static EDIT_SESSION_VTABLE: EditSessionVtable = EditSessionVtable {
    query_interface: es_qi,
    add_ref: es_add_ref,
    release: es_release,
    do_edit_session: es_do_edit_session,
};

/// 通过活动上下文取得插入光标（选区）的屏幕坐标。
///
/// 读取链路（权威 msctf.idl）：
/// `ITfThreadMgr::GetFocus(7)` → `ITfDocumentMgr::GetTop(6)` → `ITfContext`，
/// 再发起一个同步只读 edit session，在 `ITfEditSession::DoEditSession(ec)` 里用
/// 合法 cookie 调用 `ITfContext::GetSelection(5)` + `ITfContext::GetActiveView(9)`
/// + `ITfContextView::GetTextExt(4)` 得到选区矩形。
///
/// 所有步骤都检查返回值/空指针，任何一步失败都返回 `None`，由调用方回退，
/// 绝不让宿主进程崩溃。若同步会话被拒绝（返回非 S_OK），也走回退。
///
/// # Safety
///
/// `thread_mgr` 必须是指向有效 `ITfThreadMgr` 的指针。
pub unsafe fn get_caret_screen_coords(thread_mgr: *mut c_void) -> Option<(i32, i32)> {
    unsafe {
        let (_, client_id) = crate::text_service::current_tsf_ctx();
        if client_id == 0 {
            return None;
        }

        let context = get_context(thread_mgr)?;

        SESSION_CTX.with(|c| c.set(context));
        CARET_OUT.with(|c| c.set(None));
        SESSION_JOB.with(|j| *j.borrow_mut() = Some(SessionJob::Caret));

        let ctx_vtable = *(context as *const *const usize);
        // ITfContext::RequestEditSession (index 3): (tid, pES, dwFlags, pec: TfEditCookie*)
        // TfEditCookie 在 x64 上是 LONG_PTR（8 字节），不能用 i32。
        let req_fn: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut c_void,
            u32,
            *mut i64,
        ) -> HRESULT = std::mem::transmute(*ctx_vtable.add(3));

        let sink = &EDIT_SESSION_VTABLE as *const _ as *mut c_void;
        let mut _edit_cookie: i64 = 0;
        let hr = req_fn(
            context,
            client_id,
            sink,
            TF_ES_SYNC | TF_ES_READ,
            &mut _edit_cookie,
        );

        SESSION_CTX.with(|c| c.set(std::ptr::null_mut()));
        SESSION_JOB.with(|j| *j.borrow_mut() = None);
        release_obj(context);

        if hr != S_OK {
            return None;
        }

        CARET_OUT.with(|c| c.get())
    }
}

/// 在活动上下文中发起同步写会话，执行一个插入任务（用合法 cookie）。
///
/// 返回是否成功执行。任何失败都返回 `false`，由调用方回退剪贴板，不崩溃。
///
/// # Safety
///
/// `thread_mgr` 必须是指向有效 `ITfThreadMgr` 的指针，或为 null（返回 `false`）。
pub unsafe fn insert_text_at_caret(thread_mgr: *mut c_void, text: &str) -> bool {
    unsafe {
        let (_, client_id) = crate::text_service::current_tsf_ctx();
        if client_id == 0 || text.is_empty() {
            return false;
        }

        let context = match get_context(thread_mgr) {
            Some(c) => c,
            None => return false,
        };

        let wide: Vec<u16> = text.encode_utf16().collect();

        SESSION_CTX.with(|c| c.set(context));
        INSERT_OUT.with(|o| o.set(false));
        SESSION_JOB.with(|j| *j.borrow_mut() = Some(SessionJob::Insert { wide }));

        let ctx_vtable = *(context as *const *const usize);
        // ITfContext::RequestEditSession (index 3): (tid, pES, dwFlags, pec: TfEditCookie*)
        let req_fn: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *mut c_void,
            u32,
            *mut i64,
        ) -> HRESULT = std::mem::transmute(*ctx_vtable.add(3));

        let sink = &EDIT_SESSION_VTABLE as *const _ as *mut c_void;
        let mut _edit_cookie: i64 = 0;
        let hr = req_fn(
            context,
            client_id,
            sink,
            TF_ES_SYNC | TF_ES_READ | TF_ES_WRITE,
            &mut _edit_cookie,
        );

        SESSION_CTX.with(|c| c.set(std::ptr::null_mut()));
        SESSION_JOB.with(|j| *j.borrow_mut() = None);
        release_obj(context);

        if hr != S_OK {
            return false;
        }

        INSERT_OUT.with(|o| o.get())
    }
}

// ========== Public API ==========

static CLIPBOARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn clipboard_paste(text: &str) {
    // 使用锁保护剪贴板操作，SendInput 在锁外执行避免阻塞键盘热路径
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;

        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};

        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(0x0002, size);
        if h_mem.is_null() {
            CloseClipboard();
            return;
        }
        let ptr = GlobalLock(h_mem) as *mut u16;
        if ptr.is_null() {
            CloseClipboard();
            return;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        GlobalUnlock(h_mem);
        SetClipboardData(1, h_mem);
        CloseClipboard();
    }
    // 锁在此处释放，之后做 sleep + SendInput
    std::thread::sleep(std::time::Duration::from_millis(50));

    unsafe {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, KEYBDINPUT, KEYEVENTF_KEYUP,
        };
        let make_key = |vk: u16, scan: u16, flags: u32| INPUT {
            r#type: 1,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [
            make_key(0x11, 0x1D, 0),
            make_key(0x56, 0x2F, 0),
            make_key(0x56, 0x2F, KEYEVENTF_KEYUP),
            make_key(0x11, 0x1D, KEYEVENTF_KEYUP),
        ];
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

/// Inserts `text` into the focused TSF context, falling back to the clipboard.
///
/// # Safety
///
/// `thread_mgr` must be a valid pointer to an active `ITfThreadMgr`, or null to
/// force the clipboard fallback. The pointer must remain valid for the call.
pub unsafe fn tsf_insert_text(thread_mgr: *mut c_void, _client_id: u32, text: &str) {
    if thread_mgr.is_null() || text.is_empty() {
        clipboard_paste(text);
        return;
    }

    // 验证 thread_mgr 仍然有效
    {
        let ctx = crate::text_service::TSF_CTX.lock().unwrap_or_else(|e| e.into_inner());
        if ctx.thread_mgr.is_null() || ctx.thread_mgr != thread_mgr {
            // thread_mgr 已失效，回退剪贴板粘贴
            drop(ctx);
            clipboard_paste(text);
            return;
        }
    }

    // End any active composition first（当前组合路径未激活，防御性清理）
    tsf_end_composition(thread_mgr);

    // 在主写的 edit session（合法 cookie）里插入；失败则回退剪贴板
    if !insert_text_at_caret(thread_mgr, text) {
        clipboard_paste(text);
    }
}

/// 结束组合（当前组合路径未启用，保留为一个安全的 no-op）。
pub fn tsf_end_composition(_thread_mgr: *mut c_void) {
    // 组合（ITfComposition）路径当前未接入热路径；提交统一走
    // insert_text_at_caret / tsf_insert_text，因此这里无需真实结束操作。
}
