//! wbwIME Windows TSF 输入法模块
//!
//! 全手动 vtable COM 实现。不依赖 windows crate 的高层 API。
//! 注册: regsvr32 wbw_ime_tsf.dll

#![allow(clippy::upper_case_acronyms, dead_code, private_interfaces)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

use wbw_dict::{DictBuilder, FstDict};
use wbw_imekit::{ImeConfig, ImeHost};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::Ranker;
use wbw_types::{Candidate, InputContext, InputMode, RankConfig};

// ========== 基本类型 ==========

type HRESULT = i32;
type ULONG = u32;
type DWORD = u32;

const S_OK: HRESULT = 0;
const S_FALSE: HRESULT = 1;
const E_FAIL: HRESULT = -2147467259;
const E_NOTIMPL: HRESULT = -2147467263;
const E_INVALIDARG: HRESULT = -2147024809;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231;

// ========== GUID ==========

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const IID_IUNKNOWN: Guid = Guid { data1: 0x00000000, data2: 0x0000, data3: 0x0000, data4: [0xC0,0x00,0x00,0x00,0x00,0x00,0x00,0x46] };
const IID_ICLASSFACTORY: Guid = Guid { data1: 0x00000001, data2: 0x0000, data3: 0x0000, data4: [0xC0,0x00,0x00,0x00,0x00,0x00,0x00,0x46] };
const CLSID_WBW_IME: Guid = Guid { data1: 0xE8A3B0F2, data2: 0x1234, data3: 0x5678, data4: [0x9A,0xBC,0xDE,0xF0,0x12,0x34,0x56,0x78] };
const IID_ITF_THREAD_MGR: Guid = Guid { data1: 0xAA80E901, data2: 0x2021, data3: 0x11D2, data4: [0x93,0xE0,0x00,0x60,0xB0,0x67,0xB8,0x6E] };
const IID_ITF_KEY_STROKE_MGR: Guid = Guid { data1: 0xAA80E902, data2: 0x2021, data3: 0x11D2, data4: [0x93,0xE0,0x00,0x60,0xB0,0x67,0xB8,0x6E] };
const IID_ITF_TEXT_INPUT_PROCESSOR_EX: Guid = Guid { data1: 0x86462810, data2: 0x5174, data3: 0x11D4, data4: [0xB6,0x3F,0x83,0x63,0xED,0x0B,0x40,0x71] };
const IID_ITF_KEY_EVENT_SINK: Guid = Guid { data1: 0xAA80E900, data2: 0x2021, data3: 0x11D2, data4: [0x93,0xE0,0x00,0x60,0xB0,0x67,0xB8,0x6E] };

// ========== 全局状态 ==========

static DLL_REF_COUNT: AtomicI32 = AtomicI32::new(0);
static IME_STATE: Mutex<Option<ImeState>> = Mutex::new(None);

struct ImeState {
    _host: ImeHost,
    matcher: Matcher,
    ranker: Ranker,
    buffer: String,
    composing: bool,
    candidates: Vec<Candidate>,
    selected_index: usize,
}

impl ImeState {
    fn new(dict_path: &str) -> Option<Self> {
        let path = std::path::Path::new(dict_path);
        let dict = if path.extension().and_then(|e| e.to_str()) == Some("fst") {
            FstDict::from_file(path).ok()?
        } else {
            let mut builder = DictBuilder::new();
            builder.load_cin(path).ok()?;
            builder.deduplicate();
            builder.sort();
            builder.build_fst()
        };
        let matcher = Matcher::with_dict(
            MatcherConfig { fuzzy_enabled: true, ..MatcherConfig::default() },
            dict,
        );
        let ranker = Ranker::new(RankConfig::default());
        let host = ImeHost::new(ImeConfig::default());
        Some(Self { _host: host, matcher, ranker, buffer: String::new(), composing: false, candidates: Vec::new(), selected_index: 0 })
    }

    fn update_candidates(&mut self) {
        if self.buffer.is_empty() {
            self.candidates.clear();
            self.selected_index = 0;
            return;
        }
        let ctx = InputContext {
            buffer: self.buffer.clone(), cursor: self.buffer.len(),
            mode: InputMode::Pinyin, selected: Vec::new(), session_id: 0,
        };
        let matched = self.matcher.match_input(&ctx);
        self.candidates = self.ranker.rank(matched);
        self.selected_index = 0;
    }

    fn process_key(&mut self, vkey: u32) -> Option<String> {
        match vkey {
            0x0D => { // Enter
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    let text = self.candidates[self.selected_index].text.clone();
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0; self.composing = false;
                    return Some(text);
                }
                None
            }
            0x08 => { // Backspace
                if !self.buffer.is_empty() { self.buffer.pop(); self.update_candidates(); self.composing = !self.buffer.is_empty(); }
                None
            }
            0x1B => { // Escape
                self.buffer.clear(); self.candidates.clear(); self.selected_index = 0; self.composing = false;
                None
            }
            0x20 => { // Space → 选第一个
                if !self.buffer.is_empty() && !self.candidates.is_empty() {
                    let text = self.candidates[0].text.clone();
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0; self.composing = false;
                    return Some(text);
                }
                None
            }
            0x31..=0x39 => {
                let idx = (vkey - 0x31) as usize;
                if !self.buffer.is_empty() && idx < self.candidates.len() {
                    let text = self.candidates[idx].text.clone();
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0; self.composing = false;
                    return Some(text);
                }
                None
            }
            0x30 => {
                if !self.buffer.is_empty() && self.candidates.len() > 9 {
                    let text = self.candidates[9].text.clone();
                    self.buffer.clear(); self.candidates.clear(); self.selected_index = 0; self.composing = false;
                    return Some(text);
                }
                None
            }
            0x41..=0x5A => { // A-Z
                self.buffer.push((vkey as u8 + 0x20) as char);
                self.update_candidates();
                self.composing = true;
                None
            }
            _ => None,
        }
    }
}

// ========== 剪贴板输出 ==========

fn clipboard_paste(text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;

        use windows_sys::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
        use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, KEYBDINPUT, KEYEVENTF_KEYUP};

        if OpenClipboard(std::ptr::null_mut()) == 0 { return; }
        EmptyClipboard();
        let h_mem = GlobalAlloc(0x0002, size); // GMEM_MOVEABLE
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                GlobalUnlock(h_mem);
                SetClipboardData(1, h_mem); // CF_UNICODETEXT
            }
        }
        CloseClipboard();

        std::thread::sleep(std::time::Duration::from_millis(50));

        let make_key = |vk: u16, scan: u16, flags: u32| INPUT { r#type: 1, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
        let inputs = [make_key(0x11, 0x1D, 0), make_key(0x56, 0x2F, 0), make_key(0x56, 0x2F, KEYEVENTF_KEYUP), make_key(0x11, 0x1D, KEYEVENTF_KEYUP)];
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

// ========== ITfKeystrokeMgr vtable (手动) ==========

/// ITfKeystrokeMgr vtable — 只声明我们要用的方法。
///
/// IUnknown (3) + AdviseKeyEventSink(4) + UnadviseKeyEventSink(5)
#[repr(C)]
struct TfKeystrokeMgrVtable {
    _qi: usize,
    _add_ref: usize,
    _release: usize,
    // ITfKeystrokeMgr
    _advise_key_sink: usize,
    _unadvise_key_sink: usize,
    // 后续方法省略
}

/// 调用 ITfKeystrokeMgr::AdviseKeyEventSink
///
/// # Safety
/// `mgr_ptr` 必须指向有效的 ITfKeystrokeMgr COM 对象。
unsafe fn advise_key_sink(
    mgr_ptr: *mut c_void,
    tid: u32,
    sink: *mut c_void,
    focus: i32,
) -> HRESULT {
    // vtable 布局: [QI, AddRef, Release, AdviseKeyEventSink, ...]
    // AdviseKeyEventSink 是第 4 个方法 (index 3)
    let vtable = unsafe { *(mgr_ptr as *const *const usize) };
    let advise_fn: unsafe extern "system" fn(*mut c_void, u32, *mut c_void, i32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(3)) };
    unsafe { advise_fn(mgr_ptr, tid, sink, focus) }
}

/// 调用 ITfKeystrokeMgr::UnadviseKeyEventSink
///
/// # Safety
/// `mgr_ptr` 必须指向有效的 ITfKeystrokeMgr COM 对象。
unsafe fn unadvise_key_sink(mgr_ptr: *mut c_void, tid: u32) -> HRESULT {
    let vtable = unsafe { *(mgr_ptr as *const *const usize) };
    let unadvise_fn: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT =
        unsafe { std::mem::transmute(*vtable.add(4)) };
    unsafe { unadvise_fn(mgr_ptr, tid) }
}

// ========== TextService COM 对象 ==========

/// TextService 实现 IUnknown + ITfTextInputProcessorEx + ITfKeyEventSink
///
/// vtable 布局:
/// - [0..2] IUnknown (QI/AddRef/Release)
/// - [3..5] ITfTextInputProcessorEx (Activate/Deactivate/ActivateEx)
/// - [6..11] ITfKeyEventSink (OnSetFocus/OnTestKeyDown/OnTestKeyUp/OnKeyDown/OnKeyUp/OnPreservedKey)
#[repr(C)]
struct TextService {
    ref_count: i32,
    // IUnknown vtable
    qi: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    // ITfTextInputProcessorEx vtable
    tip_activate: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    tip_deactivate: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    tip_activate_ex: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
    // ITfKeyEventSink vtable
    ks_on_set_focus: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
    ks_on_test_key_down: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    ks_on_test_key_up: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    ks_on_key_down: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    ks_on_key_up: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, *mut i32) -> HRESULT,
    ks_on_preserved_key: unsafe extern "system" fn(*mut c_void, *mut c_void, *const Guid, *mut i32) -> HRESULT,
    // 状态
    client_id: u32,
    thread_mgr: *mut c_void,
}

unsafe impl Send for TextService {}
unsafe impl Sync for TextService {}

impl TextService {
    fn new() -> *mut Self {
        Box::into_raw(Box::new(Self {
            ref_count: 1,
            qi: ts_qi, add_ref: ts_add_ref, release: ts_release,
            tip_activate: ts_activate, tip_deactivate: ts_deactivate, tip_activate_ex: ts_activate_ex,
            ks_on_set_focus: ks_set_focus, ks_on_test_key_down: ks_test_key_down,
            ks_on_test_key_up: ks_test_key_up, ks_on_key_down: ks_key_down,
            ks_on_key_up: ks_key_up, ks_on_preserved_key: ks_preserved_key,
            client_id: 0,
            thread_mgr: std::ptr::null_mut(),
        }))
    }
}

// ========== IUnknown ==========

unsafe extern "system" fn ts_qi(this: *mut c_void, riid: *const Guid, ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() { return E_INVALIDARG; }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX {
        unsafe { *ppv = this; ts_add_ref(this); }
        return S_OK;
    }
    if *iid == IID_ITF_KEY_EVENT_SINK {
        unsafe { *ppv = this; ts_add_ref(this); }
        return S_OK;
    }
    unsafe { *ppv = std::ptr::null_mut(); }
    E_NOTIMPL
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
    if count == 0 { unsafe { let _ = Box::from_raw(this as *mut TextService); } }
    count
}

// ========== ITfTextInputProcessorEx ==========

unsafe extern "system" fn ts_activate(this: *mut c_void, punk: *mut c_void) -> HRESULT {
    let ts = unsafe { &mut *(this as *mut TextService) };

    // punk 是 ITfThreadMgr，但我们只拿到 IUnknown*，需要 QI
    // QI for ITfThreadMgr
    let mut thread_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(**(punk as *const *const usize));
        qi_fn(punk, &IID_ITF_THREAD_MGR, &mut thread_mgr)
    };
    if hr != S_OK || thread_mgr.is_null() {
        return E_FAIL;
    }

    // QI for ITfKeystrokeMgr (同一对象，不同接口)
    let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
    let hr = unsafe {
        let qi_fn: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(**(thread_mgr as *const *const usize));
        qi_fn(thread_mgr, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
    };
    if hr != S_OK || keystroke_mgr.is_null() {
        // 释放 thread_mgr
        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(thread_mgr as *const *const usize)).add(2));
            release_fn(thread_mgr);
        }
        return E_FAIL;
    }

    // 分配 client_id (用 this 指针的低位作为简单 ID)
    ts.client_id = (this as usize & 0xFFFF) as u32;

    // 保存 thread_mgr 用于后续使用
    ts.thread_mgr = thread_mgr;

    // 注册 ITfKeyEventSink
    let hr = unsafe { advise_key_sink(keystroke_mgr, ts.client_id, this, 1) };

    // 释放 keystroke_mgr (我们不再需要它，但 thread_mgr 保留)
    unsafe {
        let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(*(*(keystroke_mgr as *const *const usize)).add(2));
        release_fn(keystroke_mgr);
    }

    if hr != S_OK {
        // 释放 thread_mgr
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
        // QI for ITfKeystrokeMgr
        let mut keystroke_mgr: *mut c_void = std::ptr::null_mut();
        let hr = unsafe {
            let qi_fn: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT =
                std::mem::transmute(**(ts.thread_mgr as *const *const usize));
            qi_fn(ts.thread_mgr, &IID_ITF_KEY_STROKE_MGR, &mut keystroke_mgr)
        };

        if hr == S_OK && !keystroke_mgr.is_null() {
            unsafe { unadvise_key_sink(keystroke_mgr, ts.client_id); }
            unsafe {
                let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                    std::mem::transmute(*(*(keystroke_mgr as *const *const usize)).add(2));
                release_fn(keystroke_mgr);
            }
        }

        // 释放 thread_mgr
        unsafe {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*(*(ts.thread_mgr as *const *const usize)).add(2));
            release_fn(ts.thread_mgr);
        }
        ts.thread_mgr = std::ptr::null_mut();
    }

    S_OK
}

unsafe extern "system" fn ts_activate_ex(this: *mut c_void, punk: *mut c_void, _flags: u32) -> HRESULT {
    unsafe { ts_activate(this, punk) }
}

// ========== ITfKeyEventSink ==========

unsafe extern "system" fn ks_set_focus(_this: *mut c_void, _f: i32) -> HRESULT { S_OK }

unsafe extern "system" fn ks_test_key_down(
    _this: *mut c_void, _pic: *mut c_void, w_param: u32, _l_param: u32, pf_eaten: *mut i32,
) -> HRESULT {
    unsafe { *pf_eaten = 0; }
    let mut state = match IME_STATE.lock() { Ok(s) => s, Err(_) => return S_OK };
    let state = match state.as_mut() { Some(s) => s, None => return S_OK };
    let vkey = w_param & 0xFF;
    if state.composing || (0x41..=0x5A).contains(&vkey) || state.composing && matches!(vkey, 0x08 | 0x0D | 0x1B | 0x20 | 0x30..=0x39) {
        unsafe { *pf_eaten = 1; }
    }
    S_OK
}

unsafe extern "system" fn ks_test_key_up(
    _this: *mut c_void, _pic: *mut c_void, _w: u32, _l: u32, pf_eaten: *mut i32,
) -> HRESULT { unsafe { *pf_eaten = 0; } S_OK }

unsafe extern "system" fn ks_key_down(
    _this: *mut c_void, _pic: *mut c_void, w_param: u32, _l_param: u32, pf_eaten: *mut i32,
) -> HRESULT {
    unsafe { *pf_eaten = 0; }
    let mut state = match IME_STATE.lock() { Ok(s) => s, Err(_) => return S_OK };
    let state = match state.as_mut() { Some(s) => s, None => return S_OK };
    let vkey = w_param & 0xFF;
    if let Some(text) = state.process_key(vkey) {
        unsafe { *pf_eaten = 1; }
        clipboard_paste(&text);
    } else if state.composing {
        unsafe { *pf_eaten = 1; }
    }
    S_OK
}

unsafe extern "system" fn ks_key_up(
    _this: *mut c_void, _pic: *mut c_void, _w: u32, _l: u32, pf_eaten: *mut i32,
) -> HRESULT { unsafe { *pf_eaten = 0; } S_OK }

unsafe extern "system" fn ks_preserved_key(
    _this: *mut c_void, _pic: *mut c_void, _rguid: *const Guid, pf_eaten: *mut i32,
) -> HRESULT { unsafe { *pf_eaten = 0; } S_OK }

// ========== ClassFactory ==========

struct ClassFactory {
    ref_count: i32,
}

impl ClassFactory {
    fn new() -> *mut Self { Box::into_raw(Box::new(Self { ref_count: 1 })) }
}

unsafe extern "system" fn cf_qi(this: *mut c_void, riid: *const Guid, ppv: *mut *mut c_void) -> HRESULT {
    if ppv.is_null() { return E_INVALIDARG; }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN || *iid == IID_ICLASSFACTORY {
        unsafe { *ppv = this; cf_add_ref(this); }
        return S_OK;
    }
    unsafe { *ppv = std::ptr::null_mut(); }
    CLASS_E_CLASSNOTAVAILABLE
}

unsafe extern "system" fn cf_add_ref(this: *mut c_void) -> ULONG {
    let f = unsafe { &mut *(this as *mut ClassFactory) };
    f.ref_count += 1;
    f.ref_count as ULONG
}

unsafe extern "system" fn cf_release(this: *mut c_void) -> ULONG {
    let f = unsafe { &mut *(this as *mut ClassFactory) };
    f.ref_count -= 1;
    let count = f.ref_count as ULONG;
    if count == 0 { unsafe { let _ = Box::from_raw(this as *mut ClassFactory); } }
    count
}

unsafe extern "system" fn cf_create_instance(
    _this: *mut c_void, _outer: *mut c_void, riid: *const Guid, ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() { return E_INVALIDARG; }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX || *iid == IID_ITF_KEY_EVENT_SINK {
        let ts = TextService::new();
        unsafe { *ppv = ts as *mut c_void; }
        return S_OK;
    }
    unsafe { *ppv = std::ptr::null_mut(); }
    E_NOTIMPL
}

unsafe extern "system" fn cf_lock_server(_this: *mut c_void, _lock: i32) -> HRESULT { S_OK }

// ========== DLL 导出 ==========

/// DLL 入口点
///
/// # Safety
/// 由 Windows 调用。
#[no_mangle]
pub unsafe extern "system" fn DllMain(_hinst: *mut c_void, reason: u32, _reserved: *mut c_void) -> i32 {
    match reason {
        1 => { // DLL_PROCESS_ATTACH
            DLL_REF_COUNT.store(1, Ordering::SeqCst);
            let home = std::env::var("USERPROFILE").unwrap_or_default();
            let dict_path = std::path::PathBuf::from(&home)
                .join("AppData").join("Roaming").join("wbwIME").join("dict.fst");
            if dict_path.exists() {
                if let Some(state) = ImeState::new(&dict_path.to_string_lossy()) {
                    *IME_STATE.lock().unwrap() = Some(state);
                }
            }
        }
        0 => { // DLL_PROCESS_DETACH
            DLL_REF_COUNT.store(0, Ordering::SeqCst);
            *IME_STATE.lock().unwrap() = None;
        }
        _ => {}
    }
    1 // TRUE
}

/// COM 类工厂入口
///
/// # Safety
/// 参数由 COM 运行时传入。
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const Guid, _riid: *const Guid, ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || ppv.is_null() { return E_INVALIDARG; }
    let clsid = unsafe { &*rclsid };
    if *clsid != CLSID_WBW_IME {
        unsafe { *ppv = std::ptr::null_mut(); }
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory = ClassFactory::new();
    unsafe { *ppv = factory as *mut c_void; }
    S_OK
}

/// 查询 DLL 是否可卸载
///
/// # Safety
/// 无特殊安全要求。
#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 { S_OK } else { S_FALSE }
}

/// 注册 COM 服务器 + TSF TIP
///
/// # Safety
/// 写入注册表需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let dll_path = get_dll_path();
    let dll_path_str = dll_path.to_string_lossy();

    // COM CLSID 注册
    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    let _ = set_reg(&format!("CLSID\\{{{}}}", clsid), "", "wbwIME");
    let _ = set_reg(&format!("CLSID\\{{{}}}\\InprocServer32", clsid), "", &dll_path_str);

    // TSF TIP 注册
    let tip_key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}\\LanguageProfile\\0x00000804\\{{{}}}", clsid, clsid);
    let _ = set_reg(&tip_key, "Description", "wbwIME Pinyin Input");
    let _ = set_reg_dword(&tip_key, "Enable", 1);

    // Keyboard Layouts 注册
    let kl_key = "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\E0200804";
    let _ = set_reg(kl_key, "Ime File", &dll_path_str);
    let _ = set_reg(kl_key, "Layout Text", "wbwIME");
    let _ = set_reg_dword(kl_key, "Language Id", 0x0804);

    S_OK
}

/// 取消注册
///
/// # Safety
/// 删除注册表项需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    let _ = del_reg(&format!("CLSID\\{{{}}}", clsid));
    let _ = del_reg(&format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid));
    let _ = del_reg("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\E0200804");
    S_OK
}

// ========== 辅助函数 ==========

fn get_dll_path() -> std::path::PathBuf {
    unsafe {
        let mut buf = [0u16; 260];
        let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
            std::ptr::null_mut(), buf.as_mut_ptr(), 260,
        );
        if len > 0 { std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])) }
        else { std::path::PathBuf::new() }
    }
}

unsafe fn set_reg(key: &str, name: &str, value: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let val_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    if RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey) != 0 { return Err(()); }
    let ret = RegSetValueExW(hkey, name_w.as_ptr(), 0, REG_SZ, val_w.as_ptr() as *const u8, (val_w.len() * 2) as u32);
    RegCloseKey(hkey);
    if ret != 0 { Err(()) } else { Ok(()) }
}

unsafe fn set_reg_dword(key: &str, name: &str, value: u32) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    if RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey) != 0 { return Err(()); }
    let ret = RegSetValueExW(hkey, name_w.as_ptr(), 0, 4, &value as *const u32 as *const u8, 4); // REG_DWORD = 4
    RegCloseKey(hkey);
    if ret != 0 { Err(()) } else { Ok(()) }
}

unsafe fn del_reg(key: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    if RegDeleteKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr()) != 0 { Err(()) } else { Ok(()) }
}
