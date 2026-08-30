use std::ffi::c_void;

use crate::guid::*;
use crate::output;
use crate::output::{HRESULT, S_OK, ULONG};

pub static IME_STATE: std::sync::Mutex<Option<crate::state::ImeState>> =
    std::sync::Mutex::new(None);

/// 当前会话激活的 `ITfThreadMgr` 指针（供 IPC 取光标坐标）。
pub static THREAD_MGR: std::sync::atomic::AtomicPtr<std::ffi::c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

/// 当前会话激活的 TfClientId（`ITfContext::RequestEditSession` 需要）。
pub static CLIENT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// 字典加载是否已尝试过（保证只初始化一次，且避免在 DllMain 的加载器锁下做重工作）。
static STATE_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 惰性初始化输入法状态。
///
/// 从 DllMain 中移除字典/引擎的加载（那会在加载器锁下做内存分配和文件 IO，
/// 容易导致 regsvr32 及真实应用的崩溃/死锁——典型表现为 0xC000013A），
/// 改为在首次按键处理时懒加载。
pub fn ensure_state_loaded() {
    if STATE_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    let dict_path = std::path::PathBuf::from(&home)
        .join("AppData")
        .join("Roaming")
        .join("wbwIME")
        .join("dict.fst");
    if dict_path.exists() {
        if let Some(state) = crate::state::ImeState::new(&dict_path.to_string_lossy()) {
            *IME_STATE.lock().unwrap() = Some(state);
        }
    }
}

// ========== ITfKeystrokeMgr vtable helpers ==========

/// # Safety
///
/// `mgr_ptr` must be a valid pointer to an `ITfKeystrokeMgr` interface obtained
/// from an active thread manager. `sink` must be a valid pointer to a COM object
/// implementing `ITfKeyEventSink`. Both must remain alive across the call.
pub unsafe fn advise_key_sink(
    mgr_ptr: *mut c_void,
    tid: u32,
    sink: *mut c_void,
    focus: i32,
) -> HRESULT {
    let vtable = unsafe { *(mgr_ptr as *const *const usize) };
    let advise_fn: unsafe extern "system" fn(*mut c_void, u32, *mut c_void, i32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(3)) };
    unsafe { advise_fn(mgr_ptr, tid, sink, focus) }
}

/// # Safety
///
/// `mgr_ptr` must be a valid pointer to an `ITfKeystrokeMgr` interface that was
/// previously advised via [`advise_key_sink`] with the given `tid`.
pub unsafe fn unadvise_key_sink(mgr_ptr: *mut c_void, tid: u32) -> HRESULT {
    let vtable = unsafe { *(mgr_ptr as *const *const usize) };
    let unadvise_fn: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(4)) };
    unsafe { unadvise_fn(mgr_ptr, tid) }
}

// ========== TextService COM ==========

#[repr(C)]
pub struct TextService {
    pub ref_count: i32,
    pub qi: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub tip_activate: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub tip_deactivate: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub tip_activate_ex: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    pub ks_on_set_focus: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    pub ks_on_test_key_down:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub ks_on_test_key_up:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub ks_on_key_down:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub ks_on_key_up:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub ks_on_preserved_key:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const Guid, *mut i32) -> HRESULT,
    pub client_id: u32,
    pub thread_mgr: *mut c_void,
}

unsafe impl Send for TextService {}
unsafe impl Sync for TextService {}

impl TextService {
    pub fn new() -> *mut Self {
        Box::into_raw(Box::new(Self {
            ref_count: 1,
            qi: ts_qi,
            add_ref: ts_add_ref,
            release: ts_release,
            tip_activate: ts_activate,
            tip_deactivate: ts_deactivate,
            tip_activate_ex: ts_activate_ex,
            ks_on_set_focus: ks_set_focus,
            ks_on_test_key_down: ks_test_key_down,
            ks_on_test_key_up: ks_test_key_up,
            ks_on_key_down: ks_key_down,
            ks_on_key_up: ks_key_up,
            ks_on_preserved_key: ks_preserved_key,
            client_id: 0,
            thread_mgr: std::ptr::null_mut(),
        }))
    }
}

// ========== IUnknown ==========

unsafe extern "system" fn ts_qi(
    this: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return -2147024809;
    }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX
        || *iid == IID_ITF_KEY_EVENT_SINK
    {
        unsafe {
            *ppv = this;
            ts_add_ref(this);
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
    -2147467263
}

unsafe extern "system" fn ts_add_ref(this: *mut c_void) -> ULONG {
    let ts = unsafe { &mut *(this as *mut TextService) };
    ts.ref_count += 1;
    ts.ref_count as ULONG
}

unsafe extern "system" fn ts_release(this: *mut c_void) -> ULONG {
    let ts = unsafe { &mut *(this as *mut TextService) };
    ts.ref_count -= 1;
    let count = ts.ref_count as ULONG;
    if count == 0 {
        unsafe {
            let _ = Box::from_raw(this as *mut TextService);
        }
    }
    count
}

// ========== ITfTextInputProcessorEx ==========

unsafe extern "system" fn ts_activate(this: *mut c_void, punk: *mut c_void) -> HRESULT {
    let ts = unsafe { &mut *(this as *mut TextService) };

    let mut thread_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(punk as *const *const usize));
        qi_fn(punk, &IID_ITF_THREAD_MGR, &mut thread_mgr)
    };
    if hr != S_OK || thread_mgr.is_null() {
        return -2147467259;
    }

    let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(thread_mgr as *const *const usize));
        qi_fn(thread_mgr, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
    };
    if hr != S_OK || keystroke_mgr.is_null() {
        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(thread_mgr as *const *const usize)).add(2));
            release_fn(thread_mgr);
        }
        return -2147467259;
    }

    ts.client_id = (this as usize & 0xFFFF) as u32;
    ts.thread_mgr = thread_mgr;
    THREAD_MGR.store(thread_mgr, std::sync::atomic::Ordering::SeqCst);
    CLIENT_ID.store(ts.client_id, std::sync::atomic::Ordering::SeqCst);

    let hr = unsafe { advise_key_sink(keystroke_mgr, ts.client_id, this, 1) };

    unsafe {
        let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(*(*(keystroke_mgr as *const *const usize)).add(2));
        release_fn(keystroke_mgr);
    }

    if hr != S_OK {
        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(thread_mgr as *const *const usize)).add(2));
            release_fn(thread_mgr);
            ts.thread_mgr = std::ptr::null_mut();
        }
        return hr;
    }

    S_OK
}

unsafe extern "system" fn ts_deactivate(this: *mut c_void) -> HRESULT {
    let ts = unsafe { &mut *(this as *mut TextService) };

    if !ts.thread_mgr.is_null() {
        let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
        let hr = unsafe {
            let qi_fn: unsafe extern "system" fn(
                *mut c_void,
                *const Guid,
                *mut *mut c_void,
            ) -> HRESULT = std::mem::transmute(**(ts.thread_mgr as *const *const usize));
            qi_fn(ts.thread_mgr, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
        };

        if hr == S_OK && !keystroke_mgr.is_null() {
            unsafe {
                unadvise_key_sink(keystroke_mgr, ts.client_id);
            }
            unsafe {
                let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                    std::mem::transmute(*(*(keystroke_mgr as *const *const usize)).add(2));
                release_fn(keystroke_mgr);
            }
        }

        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(ts.thread_mgr as *const *const usize)).add(2));
            release_fn(ts.thread_mgr);
        }
        ts.thread_mgr = std::ptr::null_mut();
        THREAD_MGR.store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
    }

    S_OK
}

unsafe extern "system" fn ts_activate_ex(
    this: *mut c_void,
    punk: *mut c_void,
    _flags: u32,
) -> HRESULT {
    unsafe { ts_activate(this, punk) }
}

// ========== ITfKeyEventSink ==========

unsafe extern "system" fn ks_set_focus(_this: *mut c_void, _f: i32) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ks_test_key_down(
    _this: *mut c_void,
    _pic: *mut c_void,
    w_param: u32,
    _l_param: u32,
    pf_eaten: *mut i32,
) -> HRESULT {
    ensure_state_loaded();
    unsafe {
        *pf_eaten = 0;
    }
    let mut state = match IME_STATE.lock() {
        Ok(s) => s,
        Err(_) => return S_OK,
    };
    let state = match state.as_mut() {
        Some(s) => s,
        None => return S_OK,
    };
    let vkey = w_param & 0xFF;
    if state.composing || (0x41..=0x5A).contains(&vkey) {
        unsafe {
            *pf_eaten = 1;
        }
    }
    S_OK
}

unsafe extern "system" fn ks_test_key_up(
    _this: *mut c_void,
    _pic: *mut c_void,
    _w: u32,
    _l: u32,
    pf_eaten: *mut i32,
) -> HRESULT {
    unsafe {
        *pf_eaten = 0;
    }
    S_OK
}

unsafe extern "system" fn ks_key_down(
    this: *mut c_void,
    _pic: *mut c_void,
    w_param: u32,
    _l_param: u32,
    pf_eaten: *mut i32,
) -> HRESULT {
    ensure_state_loaded();
    unsafe {
        *pf_eaten = 0;
    }
    let ts = unsafe { &mut *(this as *mut TextService) };

    let commit_text = {
        let mut guard = match IME_STATE.lock() {
            Ok(s) => s,
            Err(_) => return S_OK,
        };
        let state = match guard.as_mut() {
            Some(s) => s,
            None => return S_OK,
        };
        let vkey = w_param & 0xFF;
        state.process_key(vkey);
        if state.commit_text.is_some() {
            unsafe {
                *pf_eaten = 1;
            }
            state.commit_text.take()
        } else if state.composing {
            unsafe {
                *pf_eaten = 1;
            }
            None
        } else {
            None
        }
    };

    if let Some(text) = commit_text {
        if !ts.thread_mgr.is_null() {
            output::tsf_insert_text(ts.thread_mgr, ts.client_id, &text);
        } else {
            output::clipboard_paste(&text);
        }
    }

    // 根据新的组合状态向候选窗口发送 Show/Hide（尽力而为，失败静默忽略）
    crate::ipc::refresh_gui();

    S_OK
}

unsafe extern "system" fn ks_key_up(
    _this: *mut c_void,
    _pic: *mut c_void,
    _w: u32,
    _l: u32,
    pf_eaten: *mut i32,
) -> HRESULT {
    unsafe {
        *pf_eaten = 0;
    }
    S_OK
}

unsafe extern "system" fn ks_preserved_key(
    _this: *mut c_void,
    _pic: *mut c_void,
    _rguid: *const Guid,
    pf_eaten: *mut i32,
) -> HRESULT {
    unsafe {
        *pf_eaten = 0;
    }
    S_OK
}
