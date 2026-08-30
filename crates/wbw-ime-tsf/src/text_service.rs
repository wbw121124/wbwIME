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
    // ITfKeystrokeMgr: 0=QI 1=AddRef 2=Release 3=SetFocus 4=AdviseKeyEventSink 5=Unadvise...
    let advise_fn: unsafe extern "system" fn(*mut c_void, u32, *mut c_void, i32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(4)) };
    unsafe { advise_fn(mgr_ptr, tid, sink, focus) }
}

/// # Safety
///
/// `mgr_ptr` must be a valid pointer to an `ITfKeystrokeMgr` interface that was
/// previously advised via [`advise_key_sink`] with the given `tid`.
pub unsafe fn unadvise_key_sink(mgr_ptr: *mut c_void, tid: u32) -> HRESULT {
    let vtable = unsafe { *(mgr_ptr as *const *const usize) };
    let unadvise_fn: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(5)) };
    unsafe { unadvise_fn(mgr_ptr, tid) }
}

// ========== KeyEventSink COM ==========
//
// ITfKeyEventSink vtable layout (IUnknown + 6 methods):
//   [0] QueryInterface
//   [1] AddRef
//   [2] Release
//   [3] OnSetFocus
//   [4] OnTestKeyDown
//   [5] OnTestKeyUp
//   [6] OnKeyDown
//   [7] OnKeyUp
//   [8] OnPreservedKey

/// ITfKeyEventSink vtable (slots 0..9).
#[repr(C)]
struct KeyEventSinkVtable {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub on_set_focus: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    pub on_test_key_down:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub on_test_key_up:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub on_key_down:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub on_key_up:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    pub on_preserved_key:
        unsafe extern "system" fn(*mut c_void, *mut c_void, *const Guid, *mut i32) -> HRESULT,
}

static KEY_EVENT_SINK_VTABLE: KeyEventSinkVtable = KeyEventSinkVtable {
    query_interface: ks_qi,
    add_ref: ks_add_ref,
    release: ks_release,
    on_set_focus: ks_set_focus,
    on_test_key_down: ks_test_key_down,
    on_test_key_up: ks_test_key_up,
    on_key_down: ks_key_down,
    on_key_up: ks_key_up,
    on_preserved_key: ks_preserved_key,
};

/// 独立的 `ITfKeyEventSink` COM 对象。
///
/// 关键：offset 0 必须是指向 vtable 的指针（标准 COM 布局），
/// 否则 TSF 读取 offset 0 时拿到的是 ref_count（值=1），当作 vtable 指针 → 崩溃。
/// 其方法不依赖 `this`，只读全局 `THREAD_MGR`/`CLIENT_ID`。
#[repr(C)]
struct KeyEventSink {
    lp_vtbl: *const KeyEventSinkVtable,
    ref_count: i32,
}

unsafe impl Send for KeyEventSink {}
unsafe impl Sync for KeyEventSink {}

static KEY_EVENT_SINK: KeyEventSink = KeyEventSink {
    lp_vtbl: &KEY_EVENT_SINK_VTABLE,
    ref_count: 1,
};

unsafe extern "system" fn ks_qi(
    this: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return -2147024809;
    }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN || *iid == IID_ITF_KEY_EVENT_SINK {
        unsafe {
            *ppv = this;
            ks_add_ref(this);
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
    -2147467263
}

unsafe extern "system" fn ks_add_ref(this: *mut c_void) -> ULONG {
    // ref_count 位于 lp_vtbl 之后（offset 4 on x64 is wrong - offset 8 on x64）
    // #[repr(C)] struct KeyEventSink { lp_vtbl: *const Vtable (8 bytes), ref_count: i32 }
    // ref_count 偏移 = size_of::<*const c_void>() / size_of::<i32>() = 2 (以 i32 单位)
    let s = unsafe { &mut *(this as *mut KeyEventSink) };
    s.ref_count += 1;
    s.ref_count as ULONG
}

unsafe extern "system" fn ks_release(this: *mut c_void) -> ULONG {
    let s = unsafe { &mut *(this as *mut KeyEventSink) };
    s.ref_count -= 1;
    s.ref_count as ULONG
}

// ========== TextService COM ==========
//
// ITfTextInputProcessorEx vtable layout (IUnknown + 3 methods):
//   [0] QueryInterface
//   [1] AddRef
//   [2] Release
//   [3] Activate      (ITfTextInputProcessor)
//   [4] Deactivate    (ITfTextInputProcessor)
//   [5] ActivateEx    (ITfTextInputProcessorEx)

/// TextService vtable.
#[repr(C)]
struct TextServiceVtable {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub tip_activate: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    pub tip_deactivate: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    pub tip_activate_ex:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32) -> HRESULT,
}

static TEXT_SERVICE_VTABLE: TextServiceVtable = TextServiceVtable {
    query_interface: ts_qi,
    add_ref: ts_add_ref,
    release: ts_release,
    tip_activate: ts_activate,
    tip_deactivate: ts_deactivate,
    tip_activate_ex: ts_activate_ex,
};

/// TextService COM 对象。
///
/// 关键：offset 0 必须是指向 vtable 的指针（标准 COM 布局），
/// 与 `ClassFactory` / `KeyEventSink` 保持一致。
#[repr(C)]
pub struct TextService {
    lp_vtbl: *const TextServiceVtable,
    pub ref_count: i32,
    pub client_id: u32,
    pub thread_mgr: *mut c_void,
    pub key_sink: *mut c_void,
}

unsafe impl Send for TextService {}
unsafe impl Sync for TextService {}

impl TextService {
    pub fn new() -> *mut Self {
        Box::into_raw(Box::new(Self {
            lp_vtbl: &TEXT_SERVICE_VTABLE,
            ref_count: 1,
            client_id: 0,
            thread_mgr: std::ptr::null_mut(),
            key_sink: std::ptr::null_mut(),
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
    crate::log::log(&format!(
        "ts_qi riid={:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        iid.data1, iid.data2, iid.data3, iid.data4[0], iid.data4[1], iid.data4[2], iid.data4[3],
        iid.data4[4], iid.data4[5], iid.data4[6], iid.data4[7]
    ));
    if *iid == IID_IUNKNOWN
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX
    {
        unsafe {
            *ppv = this;
            ts_add_ref(this);
        }
        return S_OK;
    }
    if *iid == IID_ITF_KEY_EVENT_SINK {
        // 返回独立的 KeyEventSink 对象（避免与 ITfTextInputProcessorEx 的
        // vtable 布局冲突导致错误派发到激活函数）。
        unsafe {
            *ppv = std::ptr::addr_of!(KEY_EVENT_SINK) as *mut c_void;
            ks_add_ref(*ppv);
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
    -2147467263
}

pub(crate) unsafe extern "system" fn ts_add_ref(this: *mut c_void) -> ULONG {
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

unsafe extern "system" fn ts_activate(this: *mut c_void, punk: *mut c_void, tid: u32) -> HRESULT {
    crate::log::log(&format!(
        "ts_activate this={:p} punk={:p} tid={}",
        this, punk, tid
    ));
    let ts = unsafe { &mut *(this as *mut TextService) };

    // `ITfTextInputProcessor::Activate(ptim, tid)`��`punk` ���̹߳�������
    // `tid` �� TSF �������ʵ client id��AdviseKeyEventSink/RequestEditSession ��������
    ts.client_id = tid;
    crate::log::log("ts_activate: before QI thread_mgr");

    // punk ������ͨ�ĵ��߹����������Ǹ���ܵ����̻᷵�� E_NOINTERFACE��
    // ���δ��Ž�Ϊ�ɵ�ģʽ��S_OK���������� E_FAIL �������/���� crash��
    let mut thread_mgr: *mut c_void = std::ptr::null_mut();
    let mut hr: HRESULT = -2147467263;
    unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(punk as *const *const usize));
        let candidates = [
            IID_ITF_THREAD_MGR,
            IID_ITF_THREAD_MGR2,
            IID_ITF_THREAD_MGR_EX,
        ];
        for cand in &candidates {
            let mut out: *mut c_void = std::ptr::null_mut();
            let h = qi_fn(punk as *mut c_void, cand, &mut out);
            crate::log::log(&format!(
                "ts_activate: punk QI {:08X}-{:04X} hr=0x{:08X} ptr={:p}",
                cand.data1, cand.data2, h as u32, out
            ));
            if h == S_OK && !out.is_null() {
                thread_mgr = out;
                hr = h;
                break;
            }
        }
    }
    crate::log::log(&format!("ts_activate: thread_mgr hr=0x{:08X} tm={:p}", hr as u32, thread_mgr));
    if hr != S_OK || thread_mgr.is_null() {
        // ��ȡ�����߹������� E_FAIL��TSF/���� crash�������� S_OK �|���ģʽ��
        crate::log::log("ts_activate: NULL thread_mgr -> degraded S_OK (no key sink)");
        return S_OK;
    }

    crate::log::log("ts_activate: before QI keystroke_mgr");
    let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(thread_mgr as *const *const usize));
        qi_fn(thread_mgr, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
    };
    crate::log::log(&format!("ts_activate: keystroke_mgr hr=0x{:08X} km={:p}", hr as u32, keystroke_mgr));
    if hr != S_OK || keystroke_mgr.is_null() {
        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(thread_mgr as *const *const usize)).add(2));
            release_fn(thread_mgr);
        }
        return -2147467259;
    }

    ts.thread_mgr = thread_mgr;
    THREAD_MGR.store(thread_mgr, std::sync::atomic::Ordering::SeqCst);
    CLIENT_ID.store(ts.client_id, std::sync::atomic::Ordering::SeqCst);

    // 用独立的 KeyEventSink 对象做按键 sink，避免 vtable 布局冲突。
    let sink = std::ptr::addr_of!(KEY_EVENT_SINK) as *mut c_void;
    ts.key_sink = sink;
    crate::log::log("ts_activate: before AdviseKeyEventSink");
    let hr = unsafe { advise_key_sink(keystroke_mgr, ts.client_id, sink, 1) };
    crate::log::log(&format!("ts_activate: AdviseKeyEventSink hr=0x{:08X}", hr as u32));

    unsafe {
        let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(*(*(keystroke_mgr as *const *const usize)).add(2));
        release_fn(keystroke_mgr);
    }

    if hr != S_OK {
        unsafe {
            // 先清全局指针——防止释放 thread_mgr 后 ks_key_down 等读到悬空指针
            THREAD_MGR.store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
            CLIENT_ID.store(0, std::sync::atomic::Ordering::SeqCst);
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
    crate::log::log(&format!("ts_deactivate this={:p}", this));
    let ts = unsafe { &mut *(this as *mut TextService) };

    if !ts.thread_mgr.is_null() {
        // 先清全局指针——防止 ks_key_down 等在释放 thread_mgr 后仍读取到悬空指针
        THREAD_MGR.store(std::ptr::null_mut(), std::sync::atomic::Ordering::SeqCst);
        CLIENT_ID.store(0, std::sync::atomic::Ordering::SeqCst);

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
    }

    S_OK
}

unsafe extern "system" fn ts_activate_ex(
    this: *mut c_void,
    punk: *mut c_void,
    tid: u32,
    _flags: u32,
) -> HRESULT {
    unsafe { ts_activate(this, punk, tid) }
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
    unsafe {
        *pf_eaten = 0;
    }
    crate::log::log(&format!("ks_test_key_down w={w_param}"));
    ensure_state_loaded();
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
        let thread_mgr = THREAD_MGR.load(std::sync::atomic::Ordering::SeqCst);
        let client_id = CLIENT_ID.load(std::sync::atomic::Ordering::SeqCst);
        if !thread_mgr.is_null() {
            output::tsf_insert_text(thread_mgr, client_id, &text);
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
