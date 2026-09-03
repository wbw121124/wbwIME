use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};

use crate::guid::*;
use crate::output::{HRESULT, S_OK, ULONG};
use crate::text_service::{self, TextService};

static DLL_HINST: std::sync::atomic::AtomicPtr<c_void> =
    std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());

const E_INVALIDARG: HRESULT = -2147024809;
const E_NOTIMPL: HRESULT = -2147467263;
const CLASS_E_CLASSNOTAVAILABLE: HRESULT = -2147221231;
const E_UNEXPECTED: HRESULT = -2147418113;
const E_FAIL: HRESULT = -2147467259;
const S_FALSE: HRESULT = 1;

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;

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
    ref_count: AtomicI32,
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
    crate::log::log(&format!(
        "cf_qi riid={:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        iid.data1, iid.data2, iid.data3, iid.data4[0], iid.data4[1], iid.data4[2], iid.data4[3],
        iid.data4[4], iid.data4[5], iid.data4[6], iid.data4[7]
    ));
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
    let f = unsafe { &*(this as *const ClassFactory) };
    f.ref_count.fetch_add(1, Ordering::Relaxed) as ULONG + 1
}

unsafe extern "system" fn cf_release(this: *mut c_void) -> ULONG {
    let f = unsafe { &*(this as *const ClassFactory) };
    let count = f.ref_count.fetch_sub(1, Ordering::Relaxed) as ULONG - 1;
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
    crate::log::log(&format!(
        "cf_create_instance riid={:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
        iid.data1, iid.data2, iid.data3, iid.data4[0], iid.data4[1], iid.data4[2], iid.data4[3],
        iid.data4[4], iid.data4[5], iid.data4[6], iid.data4[7]
    ));
    if *iid == IID_IUNKNOWN
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR
        || *iid == IID_ITF_TEXT_INPUT_PROCESSOR_EX
    {
        let ts = TextService::new();
        // new() 已将 ref_count 设为 1（对调用者的引用），不需要额外 AddRef。
        // 也不对 factory AddRef——COM 规范：CreateInstance 的输出指针
        // 由调用者通过 Release 释放，不持有 factory 的引用。
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
        DLL_PROCESS_ATTACH => {
            DLL_HINST.store(hinst, std::sync::atomic::Ordering::SeqCst);
        }
        DLL_PROCESS_DETACH => {
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
        ref_count: AtomicI32::new(1),
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
    if text_service::TEXT_SERVICE_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

/// # Safety
/// 写入注册表需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    let dll_path = get_dll_path();
    if dll_path.as_os_str().is_empty() {
        return E_UNEXPECTED;
    }
    let dll_path_str = dll_path.to_string_lossy();
    // 图标与 DLL 同目录：\wbwime.ico
    let icon_path = dll_path
        .parent()
        .map(|p| p.join("wbwime.ico"))
        .unwrap_or_else(|| std::path::PathBuf::from("wbwime.ico"));
    let icon_path_str = icon_path.to_string_lossy();

    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    let mut failures: Vec<String> = Vec::new();
    let write = |key: &str, name: &str, val: &str| -> Option<String> {
        set_reg(key, name, val)
            .err()
            .map(|e| format!("{key}\\{name}: {e}"))
    };
    let write_dw = |key: &str, name: &str, val: u32| -> Option<String> {
        set_reg_dword(key, name, val)
            .err()
            .map(|e| format!("{key}\\{name}: {e}"))
    };

    let mut put = |m: Option<String>| {
        if let Some(m) = m {
            failures.push(m);
        }
    };

    // ---- COM 类注册 ----
    put(write(&format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", clsid), "", "wbwIME"));
    put(write(
        &format!("SOFTWARE\\Classes\\CLSID\\{{{}}}\\InprocServer32", clsid),
        "",
        &dll_path_str,
    ));
    // 修改：ThreadModel 必须位于 InprocServer32 子键下，而非 CLSID 根键
    put(write(
        &format!("SOFTWARE\\Classes\\CLSID\\{{{}}}\\InprocServer32", clsid),
        "ThreadModel",
        "Both",
    ));

    // ---- TSF TIP (ITfTextInputProcessor / 现代输入法) ----
    let tip_key = format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid);
    put(write(&tip_key, "Description", "wbwIME"));
    put(write(&tip_key, "KeyboardLayout", "00000804"));
    put(write(&tip_key, "IconFile", &icon_path_str));
    put(write_dw(&tip_key, "IconIndex", 0));
    put(write_dw(&tip_key, "Enable", 1));

    // 图标子键（部分系统从 CTF\...\Icon\IconIndex / IconFile 读取）
    put(write(&format!("{}\\Icon", tip_key), "IconIndex", "0"));
    put(write(&format!("{}\\Icon", tip_key), "IconFile", &icon_path_str));

    // ---- 语言配置文件 ----
    let profile_key = format!("{}\\LanguageProfile\\0x00000804\\{{{}}}", tip_key, clsid);
    put(write(&profile_key, "Description", "wbwIME"));
    put(write(&profile_key, "Display Description", "wbwIME"));
    put(write_dw(&profile_key, "Enable", 1));
    put(write_dw(&profile_key, "Install", 1));
    // LanguageProfile ��ͼ�꣨���IMO�°�UI�����ڴ���ȡ��
    put(write(&profile_key, "IconFile", &icon_path_str));
    put(write_dw(&profile_key, "IconIndex", 0));

    // ---- TSF Category ע�ᣨ�ؼ���ʹ���뷨�ɵ�ѡ�� �ǵ������棩 ----
    // ģ�� RegisterCategory(tipclsid, catid, tipclsid) д��������ṹ��
    //   Category\Category\{catid}\{CLSID}   Ĭ��ֵΪ��
    //   Category\Item\{CLSID}\{catid}       Ĭ��ֵΪ��
    // �ؼ�����ע�� GUID_TFCAT_TIP_KEYBOARD �ͣ�����ģʽ��
    let category_guids: [&str; 7] = [
        // GUID_TFCAT_TIP_KEYBOARD - �������뷨
        "34745C63-B2F0-4784-8B67-5E12C8701A31",
        // GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT - �����½�ģʽ�����"����"����
        "13A016DF-560B-46CD-947A-4C3AF1E0E35D",
        // GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT
        "25504FB4-7BAB-4BC1-9C69-CF81890F0EF5",
        // GUID_TFCAT_TIPCAP_UIELEMENTENABLED - UIԪ�أ�������
        "49D2F9CF-1F5E-11D7-A6D3-00065B84435C",
        // GUID_TFCAT_TIPCAP_SECUREMODE
        "49D2F9CE-1F5E-11D7-A6D3-00065B84435C",
        // GUID_TFCAT_TIPCAP_COMLESS
        "364215D9-75BC-11D7-A6EF-00065B84435C",
        // GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER - ��ʾ���ԣ���ϴ���»���
        "046B8C80-1647-40F7-9B21-B93B81AABC1B",
    ];
    for cat in &category_guids {
        put(write(
            &format!("{}\\Category\\Category\\{{{}}}\\{{{}}}", tip_key, cat, clsid),
            "",
            "",
        ));
        put(write(
            &format!("{}\\Category\\Item\\{{{}}}\\{{{}}}", tip_key, clsid, cat),
            "",
            "",
        ));
    }

    if failures.is_empty() {
        S_OK
    } else {
        eprintln!("DllRegisterServer failures: {failures:?}");
        E_FAIL
    }
}

/// # Safety
/// 删除注册表项需要管理员权限。
#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    let clsid = "E8A3B0F2-1234-5678-9ABC-DEF012345678";
    let _ = del_reg(&format!("SOFTWARE\\Classes\\CLSID\\{{{}}}", clsid));
    let _ = del_reg(&format!("SOFTWARE\\Microsoft\\CTF\\TIP\\{{{}}}", clsid));
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

unsafe fn set_reg(key: &str, name: &str, value: &str) -> Result<(), i32> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let val_w: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    let rc = RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey);
    if rc != 0 {
        return Err(rc as i32);
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
        Err(ret as i32)
    } else {
        Ok(())
    }
}

unsafe fn set_reg_dword(key: &str, name: &str, value: u32) -> Result<(), i32> {
    use windows_sys::Win32::System::Registry::*;
    let key_w: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut hkey: HKEY = std::ptr::null_mut();
    let rc = RegCreateKeyW(HKEY_LOCAL_MACHINE, key_w.as_ptr(), &mut hkey);
    if rc != 0 {
        return Err(rc as i32);
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
        Err(ret as i32)
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
