use std::ffi::c_void;

use crate::guid::*;
use crate::output::{HRESULT, S_OK, ULONG};
use crate::text_service::{self, TextService};

static DLL_REF_COUNT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// 本 DLL 的模块句柄（由 DllMain 传入），用于获取 DLL 自身路径。
static DLL_HINST: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

const E_INVALIDARG: HRESULT = -2147024809;
const E_NOTIMPL: HRESULT = -2147467263;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231;

// ========== ClassFactory VTable ==========

#[repr(C)]
struct ClassFactoryVtable {
    pub query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub release: unsafe extern "system" fn(*mut c_void) -> ULONG,
    pub create_instance: unsafe extern "system" fn(
        *mut c_void,
        *mut c_void,
        *const Guid,
        *mut *mut c_void,
    ) -> HRESULT,
    pub lock_server: unsafe extern "system" fn(*mut c_void, i32) -> HRESULT,
}

static CLASS_FACTORY_VTABLE: ClassFactoryVtable = ClassFactoryVtable {
    query_interface: cf_qi,
    add_ref: cf_add_ref,
    release: cf_release,
    create_instance: cf_create_instance,
    lock_server: cf_lock_server,
};

// ========== ClassFactory ==========

#[repr(C)]
struct ClassFactory {
    lp_vtbl: *const ClassFactoryVtable, // ✅ snake_case
    ref_count: i32,
}

// ========== IClassFactory methods ==========

unsafe extern "system" fn cf_qi(
    this: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_INVALIDARG;
    }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN || *iid == IID_ICLASSFACTORY {
        unsafe {
            *ppv = this;
            cf_add_ref(this);
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
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
    if count == 0 {
        unsafe {
            let _ = Box::from_raw(this as *mut ClassFactory);
        }
    }
    count
}

unsafe extern "system" fn cf_create_instance(
    _this: *mut c_void,
    _outer: *mut c_void,
    riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() {
        return E_INVALIDARG;
    }
    let iid = unsafe { &*riid };
    if *iid == IID_IUNKNOWN
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX
        || *iid == IID_ITF_KEY_EVENT_SINK
    {
        let ts = TextService::new();
        unsafe {
            *ppv = ts as *mut c_void;
        }
        return S_OK;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
    }
    E_NOTIMPL
}

unsafe extern "system" fn cf_lock_server(_this: *mut c_void, _lock: i32) -> HRESULT {
    S_OK
}

// ========== DLL 导出 ==========

/// # Safety
/// 由 Windows 调用。
///
/// 注意：DllMain 在 Windows 加载器锁下执行，绝不能在此做内存分配、文件 IO、
/// 锁竞争或初始化引擎（曾导致 regsvr32 以 0xC000013A 崩溃）。输入法状态改为
/// 在首次按键时经 [`text_service::ensure_state_loaded`] 惰性初始化。
#[no_mangle]
pub unsafe extern "system" fn DllMain(
    hinst: *mut c_void,
    reason: u32,
    _reserved: *mut c_void,
) -> i32 {
    match reason {
        1 => {
            DLL_REF_COUNT.store(1, std::sync::atomic::Ordering::SeqCst);
            DLL_HINST.store(hinst, std::sync::atomic::Ordering::SeqCst);
        }
        0 => {
            DLL_REF_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
            if let Ok(mut guard) = text_service::IME_STATE.lock() {
                *guard = None;
            }
        }
        _ => {}
    }
    1
}

/// # Safety
/// 参数由 COM 运行时传入。
#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const Guid,
    _riid: *const Guid,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if rclsid.is_null() || ppv.is_null() {
        return E_INVALIDARG;
    }
    let clsid = unsafe { &*rclsid };
    if *clsid != CLSID_WBW_IME {
        unsafe {
            *ppv = std::ptr::null_mut();
        }
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    let factory = Box::into_raw(Box::new(ClassFactory {
        lp_vtbl: &CLASS_FACTORY_VTABLE,
        ref_count: 1,
    }));
    unsafe {
        *ppv = factory as *mut c_void;
    }
    S_OK
}

/// # Safety
/// 无特殊安全要求。
#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if DLL_REF_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        S_OK
    } else {
        1
    }
}

/// # Safety
/// 写入注册表需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let dll_path = get_dll_path();
    let dll_path_str = dll_path.to_string_lossy();

    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    // COM 类注册的 CLSID 必须位于 SOFTWARE\Classes\CLSID（HKLM\CLSID 字面根不会被
    // COM/TSF 查找）。Tip 键则位于 SOFTWARE\Microsoft\CTF\TIP。
    let _ = set_reg(
        &format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", clsid),
        "",
        "wbwIME",
    );
    let _ = set_reg(
        &format!("SOFTWARE\\Classes\\CLSID\\{{{}}}\\InprocServer32", clsid),
        "",
        &dll_path_str,
    );
    let _ = set_reg(
        &format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", clsid),
        "ThreadModel",
        "Both",
    );

    let tip_key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid);
    let _ = set_reg(&tip_key, "Description", "wbwIME Pinyin Input");
    let _ = set_reg_dword(&tip_key, "Enable", 1);

    let profile_key = format!("{}\\LanguageProfile\\0x00000804\\{{{}}}", tip_key, clsid);
    let _ = set_reg(&profile_key, "Description", "wbwIME");
    let _ = set_reg(&profile_key, "Display Description", "wbwIME");
    let _ = set_reg_dword(&profile_key, "Enable", 1);
    let _ = set_reg_dword(&profile_key, "Install", 1);

    let kl_key = "SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\E0200804";
    let _ = set_reg(kl_key, "Ime File", &dll_path_str);
    let _ = set_reg(kl_key, "Layout Text", "wbwIME");
    let _ = set_reg_dword(kl_key, "Language Id", 0x0804);

    S_OK
}

/// # Safety
/// 删除注册表项需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    let _ = del_reg(&format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", clsid));
    let _ = del_reg(&format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid));
    let _ = del_reg("SYSTEM\\CurrentControlSet\\Control\\Keyboard Layouts\\E0200804");
    S_OK
}

// ========== 辅助函数 ==========

fn get_dll_path() -> std::path::PathBuf {
    // 必须用本 DLL 的模块句柄（而非 NULL，NULL 返回的是宿主进程 exe，例如 regsvr32.exe）。
    let hmod = DLL_HINST.load(std::sync::atomic::Ordering::SeqCst);
    if hmod.is_null() {
        return std::path::PathBuf::new();
    }
    unsafe {
        let mut buf = [0u16; 260];
        let len = windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW(
            hmod as _,
            buf.as_mut_ptr(),
            260,
        );
        if len > 0 {
            std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]))
        } else {
            std::path::PathBuf::new()
        }
    }
}

unsafe fn set_reg(key: &str, name: &str, value: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let val_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    if RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey) != 0 {
        return Err(());
    }
    let ret = RegSetValueExW(
        hkey,
        name_w.as_ptr(),
        0,
        REG_SZ,
        val_w.as_ptr() as *const u8,
        (val_w.len() * 2) as u32,
    );
    RegCloseKey(hkey);
    if ret != 0 {
        Err(())
    } else {
        Ok(())
    }
}

unsafe fn set_reg_dword(key: &str, name: &str, value: u32) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    if RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey) != 0 {
        return Err(());
    }
    let ret = RegSetValueExW(
        hkey,
        name_w.as_ptr(),
        0,
        4,
        &value as *const u32 as *const u8,
        4,
    );
    RegCloseKey(hkey);
    if ret != 0 {
        Err(())
    } else {
        Ok(())
    }
}

unsafe fn del_reg(key: &str) -> Result<(), ()> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    if RegDeleteKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr()) != 0 {
        Err(())
    } else {
        Ok(())
    }
}
