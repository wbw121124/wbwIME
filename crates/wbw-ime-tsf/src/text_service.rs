use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CONTROL, VK_MENU, VK_SHIFT};

use crate::guid::*;
use crate::output;
use crate::output::{HRESULT, S_OK, ULONG};

pub static IME_STATE: std::sync::Mutex<Option<crate::state::ImeState>> =
    std::sync::Mutex::new(None);

/// 当前会话激活的 TSF 上下文（thread_mgr + client_id）。
///
/// 用单一 Mutex 而非两个 Atomic* 保证两者读写的原子性——
/// 避免 ks_key_down 中两次 load 之间 deactivate 导致 UAF。
pub static TSF_CTX: std::sync::Mutex<TsfContext> = std::sync::Mutex::new(TsfContext::EMPTY);

/// 存储的 TSF 上下文值（不含引用计数，thread_mgr 由调用方管理生命周期）。
#[derive(Clone, Copy)]
pub struct TsfContext {
    pub thread_mgr: *mut c_void,
    pub client_id: u32,
}

impl TsfContext {
    pub const EMPTY: Self = Self {
        thread_mgr: std::ptr::null_mut(),
        client_id: 0,
    };
}

unsafe impl Send for TsfContext {}
unsafe impl Sync for TsfContext {}

/// 一次性读取当前 TSF 上下文（thread_mgr, client_id）。
///
/// 保证返回值内部一致：要么都是激活态，要么都是清空态。
pub fn current_tsf_ctx() -> (*mut c_void, u32) {
    let ctx = TSF_CTX.lock().unwrap();
    (ctx.thread_mgr, ctx.client_id)
}

/// 字典加载是否已尝试过（保证只初始化一次，且避免在 DllMain 的加载器锁下做重工作）。
static STATE_INITIALIZED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 当前活跃的 TextService 实例数（供 DllCanUnloadNow 判断）。
pub static TEXT_SERVICE_COUNT: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(0);

/// 惰性初始化输入法状态。
///
/// 从 DllMain 中移除字典/引擎的加载（那会在加载器锁下做内存分配和文件 IO，
/// 容易导致 regsvr32 及真实应用的崩溃/死锁——典型表现为 0xC000013A），
/// 改为在首次按键处理时懒加载。
pub fn ensure_state_loaded() {
    // 修改原因：原实现硬编码从 %USERPROFILE%\AppData\Roaming\wbwIME\dict.fst 加载，
    // 但安装脚本实际将字典（base.cin / 拼音码表）放到 %LOCALAPPDATA%\wbwIME\dicts\，
    // 导致 IME_STATE 永远为 None、TSF 不处理按键、GUI 不启动。
    // 改为多候选路径依次尝试，并将 STATE_INITIALIZED 改为仅在成功时置 true，
    // 允许失败后下次按键时重试。

    // 已成功加载过，直接返回
    if IME_STATE.lock().unwrap().is_some() {
        return;
    }
    // 仍在加载中（另一个线程正在尝试），也直接返回，避免并发重复 IO
    if STATE_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }

    let roaming = std::env::var("APPDATA").unwrap_or_default();
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();

    let candidates: Vec<std::path::PathBuf> = vec![
        std::path::PathBuf::from(&roaming).join("wbwIME").join("dict.fst"),
        std::path::PathBuf::from(&roaming).join("wbwIME").join("dicts").join("base.cin"),
        std::path::PathBuf::from(&local).join("wbwIME").join("dict.fst"),
        std::path::PathBuf::from(&local).join("wbwIME").join("dicts").join("base.cin"),
        std::path::PathBuf::from(&local).join("wbwIME").join("base.cin"),
    ];

    for p in &candidates {
        let exists = p.exists();
        crate::log::log(&format!(
            "ensure_state_loaded: try {} exists={}",
            p.display(),
            exists
        ));
        if exists {
            if let Some(state) = crate::state::ImeState::new(&p.to_string_lossy()) {
                crate::log::log(&format!(
                    "ensure_state_loaded: loaded from {}",
                    p.display()
                ));
                *IME_STATE.lock().unwrap() = Some(state);
                return;
            }
        }
    }

    crate::log::log("ensure_state_loaded: no dict found");
    // 全部失败：重置 STATE_INITIALIZED，允许下次按键时重试
    STATE_INITIALIZED.store(false, std::sync::atomic::Ordering::SeqCst);
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
    ref_count: AtomicI32,
}

unsafe impl Send for KeyEventSink {}
unsafe impl Sync for KeyEventSink {}

static KEY_EVENT_SINK: KeyEventSink = KeyEventSink {
    lp_vtbl: &KEY_EVENT_SINK_VTABLE,
    ref_count: AtomicI32::new(1),
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
    let s = unsafe { &*(this as *const KeyEventSink) };
    s.ref_count.fetch_add(1, Ordering::Relaxed) as ULONG + 1
}

unsafe extern "system" fn ks_release(this: *mut c_void) -> ULONG {
    let s = unsafe { &*(this as *const KeyEventSink) };
    s.ref_count.fetch_sub(1, Ordering::Relaxed) as ULONG - 1
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
    pub ref_count: AtomicI32,
    pub client_id: u32,
    pub thread_mgr: *mut c_void,
    pub key_sink: *mut c_void,
}

unsafe impl Send for TextService {}
unsafe impl Sync for TextService {}

impl TextService {
    pub fn new() -> *mut Self {
        TEXT_SERVICE_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Box::into_raw(Box::new(Self {
            lp_vtbl: &TEXT_SERVICE_VTABLE,
            ref_count: AtomicI32::new(1),
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
    let ts = unsafe { &*(this as *const TextService) };
    ts.ref_count.fetch_add(1, Ordering::Relaxed) as ULONG + 1
}

unsafe extern "system" fn ts_release(this: *mut c_void) -> ULONG {
    let ts = unsafe { &*(this as *const TextService) };
    let count = ts.ref_count.fetch_sub(1, Ordering::Relaxed) as ULONG - 1;
    if count == 0 {
        TEXT_SERVICE_COUNT.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
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

    // TSF 规范：ActivateEx 的 punk 参数本身就是指向线程管理器的指针。
    // 标准 msctf.dll 中 ITfThreadMgr / ITfThreadMgr2 / ITfThreadMgrEx /
    // ITfKeystrokeMgr 全部位于同一个 COM 对象（CLSID_TF_ThreadMgr）上，
    // 因此从 punk（或任一接口指针）QI ITfKeystrokeMgr 必然成功。
    //
    // 这里直接对 punk QI ITfKeystrokeMgr，不使用"多接口三选一"再绕道的方式。
    // 部分受限宿主（TextInputHost/ApplicationFrameHost）可能仅暴露精简对象，
    // 若确实拿不到 keystroke mgr，则降级：保留线程上下文但跳过按键 sink，
    // 避免返回错误导致宿主崩溃。
    crate::log::log("ts_activate: before QI keystroke_mgr (from punk)");
    let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(punk as *const *const usize));
        qi_fn(punk as *mut c_void, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
    };
    crate::log::log(&format!("ts_activate: keystroke_mgr hr=0x{:08X} km={:p}", hr as u32, keystroke_mgr));

    // 需要被保留的 ITfThreadMgr/Ex 接口（取得第一个可用的，用于后续 edit session）。
    // 优先标准 ITfThreadMgr，其次 ITfThreadMgrEx / ITfThreadMgr2（同一对象）。
    let mut thread_mgr: *mut c_void = std::ptr::null_mut();
    unsafe {
        let qi_fn: unsafe extern "system" fn(
            *mut c_void,
            *const Guid,
            *mut *mut c_void,
        ) -> HRESULT = std::mem::transmute(**(punk as *const *const usize));
        let candidates = [
            IID_ITF_THREAD_MGR,
            IID_ITF_THREAD_MGR_EX,
            IID_ITF_THREAD_MGR2,
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
                break;
            }
        }
        // 若连一个线程管理器接口都拿不到，退回 punk 本身（有的宿主直接给的就是它）。
        // 修改原因：原代码 thread_mgr = punk 未经 AddRef，但 ts_deactivate 会
        // 对 ts.thread_mgr 无条件调用 Release，导致对未 AddRef 的裸指针做 Release，
        // 破坏宿主引用计数 / 潜在崩溃。此处补一次 AddRef 使后续 Release 配平。
        if thread_mgr.is_null() {
            thread_mgr = punk;
            let add_ref_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(punk as *const *const usize)).add(1));
            add_ref_fn(punk);
        }
    }
    crate::log::log(&format!("ts_activate: thread_mgr tm={:p}", thread_mgr));

    // 拿不到 keystroke mgr 时：保留线程上下文供后续优先 TSF 输出（若可用），
    // 但无按键 sink → 降级模式：启动钩子兜底 GUI（自捕获键盘+剪贴板上屏）。
    if hr != S_OK || keystroke_mgr.is_null() {
        crate::log::log("ts_activate: ITfKeystrokeMgr not available -> degraded, launch hook gui");
        ts.thread_mgr = thread_mgr;
        *TSF_CTX.lock().unwrap() = TsfContext { thread_mgr, client_id: ts.client_id };
        crate::ipc::launch_hook_gui();
        return S_OK;
    }

    ts.thread_mgr = thread_mgr;
    *TSF_CTX.lock().unwrap() = TsfContext { thread_mgr, client_id: ts.client_id };

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
            *TSF_CTX.lock().unwrap() = TsfContext::EMPTY;
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
        // 先清全局上下文——防止 ks_key_down 等在释放 thread_mgr 后仍读取到悬空指针
        *TSF_CTX.lock().unwrap() = TsfContext::EMPTY;

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

    let vkey = w_param & 0xFF;

    // 有 Ctrl/Alt/Shift 修饰键时不拦截——让快捷键（Ctrl+A/C/V 等）正常工作。
    // 仅在纯字母键且无修饰键时才考虑拦截。
    let ctrl = (GetKeyState(VK_CONTROL.into()) as u16 & 0x8000) != 0;
    let alt = (GetKeyState(VK_MENU.into()) as u16 & 0x8000) != 0;
    let shift = (GetKeyState(VK_SHIFT.into()) as u16 & 0x8000) != 0;
    if ctrl || alt {
        return S_OK;
    }

    let mut state = match IME_STATE.lock() {
        Ok(s) => s,
        Err(_) => return S_OK,
    };
    let state = match state.as_mut() {
        Some(s) => s,
        None => return S_OK,
    };
    if state.composing || (!shift && (0x41..=0x5A).contains(&vkey)) {
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
        let (thread_mgr, client_id) = current_tsf_ctx();
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
