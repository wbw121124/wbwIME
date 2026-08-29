#![allow(clippy::missing_const_for_thread_local)]
use std::ffi::c_void;

use crate::guid::*;

pub type HRESULT = i32;
pub type ULONG = u32;

pub const S_OK: HRESULT = 0;

// ========== Composition state ==========

struct CompositionState {
    context: *mut c_void,
    composition: *mut c_void,
    range: *mut c_void,
}

thread_local! {
    static COMPOSITION: std::cell::RefCell<Option<CompositionState>> = const { std::cell::RefCell::new(None) };
}

// ========== ITfCompositionSink ==========
unsafe extern "system" fn comp_sink_qi(
    _this: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return -2147024809;
    }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN {
        unsafe {
            *ppv = _this;
            comp_sink_add_ref(_this);
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
    -2147467263
}

unsafe extern "system" fn comp_sink_add_ref(_this: *mut c_void) -> ULONG {
    1
}

unsafe extern "system" fn comp_sink_release(_this: *mut c_void) -> ULONG {
    0
}

// ITfCompositionSink::OnCompositionTerminated
unsafe extern "system" fn comp_sink_on_terminated(
    _this: *mut c_void,
    _edit_cookie: u64,
    _composition: *mut c_void,
) -> HRESULT {
    COMPOSITION.with(|c| {
        *c.borrow_mut() = None;
    });
    S_OK
}

#[repr(C)]
pub struct CompositionSinkVtable {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> u32, // 假设是这个
    pub release: unsafe extern "system" fn(*mut c_void) -> u32, // 假设是这个
    pub on_terminated: unsafe extern "system" fn(*mut c_void, u64, *mut c_void) -> i32, // ✅ 修正
}

// ✅ 每个函数都有正确的签名
static COMPOSITION_SINK_VTABLE: CompositionSinkVtable = CompositionSinkVtable {
    query_interface: comp_sink_qi,
    add_ref: comp_sink_add_ref,
    release: comp_sink_release,
    on_terminated: comp_sink_on_terminated,
};

// ========== Helper: get context from thread_mgr ==========

unsafe fn get_context(thread_mgr: *mut c_void) -> Option<*mut c_void> {
    let tm_vtable = *(thread_mgr as *const *const usize);
    let get_active_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
        std::mem::transmute(*tm_vtable.add(8));
    let mut doc_mgr: *mut c_void = std::ptr::null_mut();
    if get_active_fn(thread_mgr, &mut doc_mgr) != S_OK || doc_mgr.is_null() {
        return None;
    }

    let dm_vtable = *(doc_mgr as *const *const usize);
    let get_active_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
        std::mem::transmute(*dm_vtable.add(4));
    let mut context: *mut c_void = std::ptr::null_mut();
    let hr = get_active_fn(doc_mgr, &mut context);
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

// ========== Public API ==========

pub fn clipboard_paste(text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;

        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, KEYBDINPUT, KEYEVENTF_KEYUP,
        };

        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return;
        }
        EmptyClipboard();
        let h_mem = GlobalAlloc(0x0002, size);
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                GlobalUnlock(h_mem);
                SetClipboardData(1, h_mem);
            }
        }
        CloseClipboard();

        std::thread::sleep(std::time::Duration::from_millis(50));

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

    // End any active composition first
    tsf_end_composition(thread_mgr);

    let wide: Vec<u16> = text.encode_utf16().collect();

    unsafe {
        let context = match get_context(thread_mgr) {
            Some(c) => c,
            None => {
                clipboard_paste(text);
                return;
            }
        };

        let insert_sel = match qi(context, &IID_ITF_INSERT_AT_SELECTION) {
            Some(p) => p,
            None => {
                release_obj(context);
                clipboard_paste(text);
                return;
            }
        };

        let insert_vtable = *(insert_sel as *const *const usize);
        let insert_fn: unsafe extern "system" fn(
            *mut c_void,
            u32,
            *const u16,
            u32,
            *mut u32,
        ) -> HRESULT = std::mem::transmute(*insert_vtable.add(3));
        let mut written: u32 = 0;
        let hr = insert_fn(
            insert_sel,
            0,
            wide.as_ptr(),
            wide.len() as u32,
            &mut written,
        );

        release_obj(insert_sel);
        release_obj(context);

        if hr != S_OK {
            clipboard_paste(text);
        }
    }
}

/// Starts a TSF composition tied to the active context.
///
/// # Safety
///
/// `thread_mgr` must be a valid pointer to an active `ITfThreadMgr`, or null.
/// The pointer must remain valid for the call.
pub unsafe fn tsf_start_composition(thread_mgr: *mut c_void) {
    if thread_mgr.is_null() {
        return;
    }

    // Already composing?
    let already = COMPOSITION.with(|c| c.borrow().is_some());
    if already {
        return;
    }

    unsafe {
        let context = match get_context(thread_mgr) {
            Some(c) => c,
            None => return,
        };

        let comp_ctx = match qi(context, &IID_ITF_CONTEXT_COMPOSITION) {
            Some(p) => p,
            None => {
                release_obj(context);
                return;
            }
        };

        // Create a static vtable pointer for our composition sink
        let sink = &COMPOSITION_SINK_VTABLE as *const _ as *mut c_void;

        // ITfContextComposition::StartComposition (index 3)
        let vtable = *(comp_ctx as *const *const usize);
        let start_fn: unsafe extern "system" fn(
            *mut c_void,
            u64,
            *mut c_void,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(*vtable.add(3));
        let mut composition: *mut c_void = std::ptr::null_mut();
        let hr = start_fn(comp_ctx, 0, sink, &mut composition);

        release_obj(comp_ctx);

        if hr == S_OK && !composition.is_null() {
            // Get the range from the composition
            let comp_vtable = *(composition as *const *const usize);
            let get_range_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
                std::mem::transmute(*comp_vtable.add(3));
            let mut range: *mut c_void = std::ptr::null_mut();
            let hr = get_range_fn(composition, &mut range);

            if hr == S_OK && !range.is_null() {
                COMPOSITION.with(|c| {
                    *c.borrow_mut() = Some(CompositionState {
                        context,
                        composition,
                        range,
                    });
                });
            } else {
                // End composition if we can't get range
                let end_fn: unsafe extern "system" fn(*mut c_void, u64) -> HRESULT =
                    std::mem::transmute(*comp_vtable.add(6));
                end_fn(composition, 0);
                release_obj(composition);
                release_obj(context);
            }
        } else {
            release_obj(context);
        }
    }
}

/// Updates the visible composition text.
///
/// # Safety
///
/// `thread_mgr` must be a valid pointer to an active `ITfThreadMgr`, or null.
/// The pointer must remain valid for the call.
pub unsafe fn tsf_update_composition(thread_mgr: *mut c_void, text: &str) {
    if thread_mgr.is_null() || text.is_empty() {
        return;
    }

    // Start composition if not already
    let has_comp = COMPOSITION.with(|c| c.borrow().is_some());
    if !has_comp {
        tsf_start_composition(thread_mgr);
    }

    COMPOSITION.with(|c| {
        let borrowed = c.borrow();
        let state = match borrowed.as_ref() {
            Some(s) => s,
            None => return,
        };
        let range = state.range;
        if range.is_null() {
            return;
        }

        let wide: Vec<u16> = text.encode_utf16().collect();

        unsafe {
            let vtable = *(range as *const *const usize);
            // ITfRange::SetText (index 4)
            let set_text_fn: unsafe extern "system" fn(
                *mut c_void,
                u64,
                *const u16,
                u32,
            ) -> HRESULT = std::mem::transmute(*vtable.add(4));
            set_text_fn(range, 0, wide.as_ptr(), wide.len() as u32);
        }
    });
}

pub fn tsf_end_composition(thread_mgr: *mut c_void) {
    let state = COMPOSITION.with(|c| c.borrow_mut().take());
    if let Some(state) = state {
        unsafe {
            if !state.composition.is_null() {
                let vtable = *(state.composition as *const *const usize);
                // ITfComposition::EndComposition (index 6)
                let end_fn: unsafe extern "system" fn(*mut c_void, u64) -> HRESULT =
                    std::mem::transmute(*vtable.add(6));
                end_fn(state.composition, 0);
                release_obj(state.composition);
            }
            release_obj(state.range);
            release_obj(state.context);
        }
    }
    let _ = thread_mgr;
}
