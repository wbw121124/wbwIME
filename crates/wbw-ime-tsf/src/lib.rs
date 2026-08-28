//! wbwIME Windows TSF 输入法模块
//!
//! 编译为 DLL，注册为 Windows Text Services Framework 输入法。
//! 注册: regsvr32 wbw_ime_tsf.dll
//! 使用: 在 Windows 设置中添加 "wbwIME" 输入法

#![allow(clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

use wbw_dict::{DictBuilder, FstDict};
use wbw_imekit::{ImeConfig, ImeHost};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::Ranker;
use wbw_types::{Candidate, RankConfig};

// ========== COM 基础设施 ==========

type HRESULT = i32;
type ULONG = u32;
type BOOL = i32;
type REFCLSID = *const Guid;
type REFIID = *const Guid;

const S_OK: HRESULT = 0;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231;
const NOERROR: HRESULT = 0;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

// wbwIME CLSID: {E8A3B0F2-1234-5678-9ABC-DEF012345678}
const CLSID_WBW_IME: Guid = Guid {
    data1: 0xE8A3B0F2,
    data2: 0x1234,
    data3: 0x5678,
    data4: [0x9A, 0xBC, 0xDE, 0xF0, 0x12, 0x34, 0x56, 0x78],
};

const IID_IUNKNOWN: Guid = Guid {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

const IID_ICLASS_FACTORY: Guid = Guid {
    data1: 0x00000001,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// ========== 全局状态 ==========

static DLL_REF_COUNT: AtomicI32 = AtomicI32::new(0);

// ========== VTable 定义 ==========

#[repr(C)]
struct IUnknownVtbl {
    query_interface:
        unsafe extern "system" fn(this: *mut c_void, iid: REFIID, ppv: *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
}

#[repr(C)]
struct IClassFactoryVtbl {
    query_interface:
        unsafe extern "system" fn(this: *mut c_void, iid: REFIID, ppv: *mut *mut c_void) -> HRESULT,
    add_ref: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    release: unsafe extern "system" fn(this: *mut c_void) -> ULONG,
    create_instance: unsafe extern "system" fn(
        this: *mut c_void,
        p_unk_outer: *mut c_void,
        riid: REFIID,
        ppv_object: *mut *mut c_void,
    ) -> HRESULT,
    lock_server: unsafe extern "system" fn(this: *mut c_void, f_lock: BOOL) -> HRESULT,
}

// ========== TextService ==========

#[repr(C)]
struct WbwTextService {
    vtbl: *const IUnknownVtbl,
    ref_count: AtomicI32,
    #[allow(dead_code)]
    ime: Option<WbwImeState>,
}

struct WbwImeState {
    #[allow(dead_code)]
    host: ImeHost,
    #[allow(dead_code)]
    matcher: Matcher,
    #[allow(dead_code)]
    ranker: Ranker,
    #[allow(dead_code)]
    buffer: String,
    #[allow(dead_code)]
    active: bool,
    #[allow(dead_code)]
    candidates: Vec<Candidate>,
}

impl WbwTextService {
    fn new() -> Self {
        Self {
            vtbl: &UNKNOWN_VTBL,
            ref_count: AtomicI32::new(1),
            ime: None,
        }
    }

    #[allow(dead_code)]
    fn init_ime(&mut self, dict_path: &str) {
        let path = std::path::Path::new(dict_path);
        let dict = if path.extension().and_then(|e| e.to_str()) == Some("fst") {
            FstDict::from_file(path).ok()
        } else {
            let mut builder = DictBuilder::new();
            if builder.load_cin(path).is_err() {
                return;
            }
            builder.deduplicate();
            builder.sort();
            Some(builder.build_fst())
        };

        if let Some(dict) = dict {
            let matcher_config = MatcherConfig {
                fuzzy_enabled: true,
                ..MatcherConfig::default()
            };
            let matcher = Matcher::with_dict(matcher_config, dict);
            let ranker = Ranker::new(RankConfig::default());
            let host = ImeHost::new(ImeConfig::default());

            self.ime = Some(WbwImeState {
                host,
                matcher,
                ranker,
                buffer: String::new(),
                active: false,
                candidates: Vec::new(),
            });
        }
    }
}

static UNKNOWN_VTBL: IUnknownVtbl = IUnknownVtbl {
    query_interface: tsf_query_interface,
    add_ref: tsf_add_ref,
    release: tsf_release,
};

unsafe extern "system" fn tsf_query_interface(
    this: *mut c_void,
    iid: REFIID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let service = &mut *(this as *mut WbwTextService);
    let iid_ref = &*iid;

    if *iid_ref == IID_IUNKNOWN {
        *ppv = this;
        service.ref_count.fetch_add(1, Ordering::SeqCst);
        return S_OK;
    }

    *ppv = std::ptr::null_mut();
    CLASS_E_CLASSNOTAVAILABLE
}

unsafe extern "system" fn tsf_add_ref(this: *mut c_void) -> ULONG {
    let service = &*(this as *mut WbwTextService);
    service.ref_count.fetch_add(1, Ordering::SeqCst) as ULONG + 1
}

unsafe extern "system" fn tsf_release(this: *mut c_void) -> ULONG {
    let service = &*(this as *mut WbwTextService);
    let count = service.ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count <= 0 {
        drop(Box::from_raw(this as *mut WbwTextService));
    }
    count as ULONG
}

// ========== ClassFactory ==========

#[repr(C)]
struct ClassFactory {
    vtbl: *const IClassFactoryVtbl,
    ref_count: AtomicI32,
}

impl ClassFactory {
    fn new() -> Self {
        Self {
            vtbl: &CLASSFACTORY_VTBL,
            ref_count: AtomicI32::new(1),
        }
    }
}

static CLASSFACTORY_VTBL: IClassFactoryVtbl = IClassFactoryVtbl {
    query_interface: cf_query_interface,
    add_ref: cf_add_ref,
    release: cf_release,
    create_instance: cf_create_instance,
    lock_server: cf_lock_server,
};

unsafe extern "system" fn cf_query_interface(
    this: *mut c_void,
    iid: REFIID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory = &mut *(this as *mut ClassFactory);
    let iid_ref = &*iid;

    if *iid_ref == IID_IUNKNOWN || *iid_ref == IID_ICLASS_FACTORY {
        *ppv = this;
        factory.ref_count.fetch_add(1, Ordering::SeqCst);
        return S_OK;
    }

    *ppv = std::ptr::null_mut();
    CLASS_E_CLASSNOTAVAILABLE
}

unsafe extern "system" fn cf_add_ref(this: *mut c_void) -> ULONG {
    let factory = &*(this as *mut ClassFactory);
    factory.ref_count.fetch_add(1, Ordering::SeqCst) as ULONG + 1
}

unsafe extern "system" fn cf_release(this: *mut c_void) -> ULONG {
    let factory = &*(this as *mut ClassFactory);
    let count = factory.ref_count.fetch_sub(1, Ordering::SeqCst) - 1;
    if count <= 0 {
        drop(Box::from_raw(this as *mut ClassFactory));
    }
    count as ULONG
}

unsafe extern "system" fn cf_create_instance(
    _this: *mut c_void,
    _p_unk_outer: *mut c_void,
    riid: REFIID,
    ppv_object: *mut *mut c_void,
) -> HRESULT {
    if ppv_object.is_null() {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let iid_ref = &*riid;
    if *iid_ref != IID_IUNKNOWN {
        *ppv_object = std::ptr::null_mut();
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let service = Box::new(WbwTextService::new());
    let ptr = Box::into_raw(service);
    *ppv_object = ptr as *mut c_void;
    S_OK
}

unsafe extern "system" fn cf_lock_server(_this: *mut c_void, _f_lock: BOOL) -> HRESULT {
    S_OK
}

// ========== DLL 导出 ==========

/// DLL 入口点。由 Windows 调用。
///
/// # Safety
/// `hinst` 和 `reserved` 由系统传入，必须为有效的指针或 null。
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    _hinst: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> BOOL {
    match reason {
        1 => {
            // DLL_PROCESS_ATTACH
            DLL_REF_COUNT.store(1, Ordering::SeqCst);
        }
        0 => {
            // DLL_PROCESS_DETACH
            DLL_REF_COUNT.store(0, Ordering::SeqCst);
        }
        _ => {}
    }
    1 // TRUE
}

/// COM 类工厂入口点。由 COM 运行时调用。
///
/// # Safety
/// `rclsid` 和 `ppv` 由系统传入，必须为有效的指针。
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: REFCLSID,
    _riid: REFIID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let clsid = &*rclsid;
    if *clsid != CLSID_WBW_IME {
        *ppv = std::ptr::null_mut();
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    let factory = Box::new(ClassFactory::new());
    let ptr = Box::into_raw(factory);
    *ppv = ptr as *mut c_void;
    S_OK
}

/// 查询 DLL 是否可以卸载。
///
/// # Safety
/// 无特殊安全要求，由 COM 运行时调用。
#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REF_COUNT.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        1 // S_FALSE
    }
}

/// 注册 COM 服务器和 TSF 输入法（需要管理员权限）。
///
/// `regsvr32 wbw_ime_tsf.dll`
///
/// # Safety
/// 写入注册表需要管理员权限，操作 HKLM 根键。
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}";
    let name = "wbwIME";

    let com_key = format!("CLSID\\{}", clsid);
    let _ = set_registry_value(&com_key, "", name);
    let _ = set_registry_value(&com_key, "InprocServer32", get_dll_path().to_str().unwrap_or(""));

    let tsf_key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid);
    let _ = set_registry_value(&tsf_key, "Description", name);

    let profile_key = format!(
        "{}\\LanguageProfile\\0x00000804\\{{00000000-0000-0000-0000-000000000000}}",
        tsf_key
    );
    let _ = set_registry_value(&profile_key, "Description", name);

    NOERROR
}

/// 取消注册。
///
/// # Safety
/// 删除注册表项需要管理员权限，操作 HKLM 根键。
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    let clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}";
    let _ = delete_registry_key(&format!("CLSID\\{}", clsid));
    let _ = delete_registry_key(&format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid));
    NOERROR
}

// ========== 辅助函数 ==========

fn get_dll_path() -> std::path::PathBuf {
    let mut path = std::path::PathBuf::new();
    unsafe {
        let mut buf = [0u16; 260];
        let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            260,
        );
        if len > 0 {
            let s = String::from_utf16_lossy(&buf[..len as usize]);
            path = std::path::PathBuf::from(s);
        }
    }
    path
}

unsafe fn set_registry_value(key: &str, value_name: &str, value: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;

    let key_wide: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_wide: Vec<u16> = value_name.encode_utf16().chain(std::iter::once(0)).collect();
    let value_wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();

    let mut hkey: HKEY = std::ptr::null_mut();
    let result = RegCreateKeyW(HKEY_LOCAL_MACHINE, key_wide.as_ptr(), &mut hkey);

    if result != 0 {
        return Err(());
    }

    let ret = RegSetValueExW(
        hkey,
        name_wide.as_ptr(),
        0,
        REG_SZ,
        value_wide.as_ptr() as *const u8,
        (value_wide.len() * 2) as u32,
    );

    RegCloseKey(hkey);
    if ret != 0 {
        Err(())
    } else {
        Ok(())
    }
}

unsafe fn delete_registry_key(key: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;

    let key_wide: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let result = RegDeleteKeyW(HKEY_LOCAL_MACHINE, key_wide.as_ptr());
    if result != 0 {
        Err(())
    } else {
        Ok(())
    }
}
