use std::ffi::c_void;

pub type HRESULT = i32;
pub type ULONG = u32;

pub const S_OK: HRESULT = 0;

pub fn clipboard_paste(text: &str) {
    unsafe {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;

        use windows_sys::Win32::System::DataExchange::{OpenClipboard, CloseClipboard, EmptyClipboard, SetClipboardData};
        use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SendInput, INPUT, INPUT_0, KEYBDINPUT, KEYEVENTF_KEYUP};

        if OpenClipboard(std::ptr::null_mut()) == 0 { return; }
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

        let make_key = |vk: u16, scan: u16, flags: u32| INPUT { r#type: 1, Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } } };
        let inputs = [make_key(0x11, 0x1D, 0), make_key(0x56, 0x2F, 0), make_key(0x56, 0x2F, KEYEVENTF_KEYUP), make_key(0x11, 0x1D, KEYEVENTF_KEYUP)];
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

pub fn tsf_insert_text(thread_mgr: *mut c_void, _client_id: u32, text: &str) {
    if thread_mgr.is_null() || text.is_empty() {
        clipboard_paste(text);
        return;
    }

    let wide: Vec<u16> = text.encode_utf16().collect();

    unsafe {
        let tm_vtable = *(thread_mgr as *const *const usize);

        let get_active_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(*tm_vtable.add(8));
        let mut doc_mgr: *mut c_void = std::ptr::null_mut();
        if get_active_fn(thread_mgr, &mut doc_mgr) != S_OK || doc_mgr.is_null() {
            clipboard_paste(text);
            return;
        }

        let dm_vtable = *(doc_mgr as *const *const usize);
        let get_ctx_fn: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(*dm_vtable.add(4));
        let mut context: *mut c_void = std::ptr::null_mut();
        let hr = get_ctx_fn(doc_mgr, &mut context);
        {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*dm_vtable.add(2));
            release_fn(doc_mgr);
        }
        if hr != S_OK || context.is_null() {
            clipboard_paste(text);
            return;
        }

        let ctx_vtable = *(context as *const *const usize);
        let qi_fn: unsafe extern "system" fn(*mut c_void, *const crate::guid::Guid, *mut *mut c_void) -> HRESULT =
            std::mem::transmute(*ctx_vtable.add(0));
        let mut insert_sel: *mut c_void = std::ptr::null_mut();
        let hr = qi_fn(context, &crate::guid::IID_ITF_INSERT_AT_SELECTION, &mut insert_sel);
        if hr != S_OK || insert_sel.is_null() {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*ctx_vtable.add(2));
            release_fn(context);
            clipboard_paste(text);
            return;
        }

        let insert_vtable = *(insert_sel as *const *const usize);
        let insert_fn: unsafe extern "system" fn(*mut c_void, u32, *const u16, u32, *mut u32) -> HRESULT =
            std::mem::transmute(*insert_vtable.add(3));
        let mut written: u32 = 0;
        let hr = insert_fn(insert_sel, 0, wide.as_ptr(), wide.len() as u32, &mut written);

        {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*insert_vtable.add(2));
            release_fn(insert_sel);
        }
        {
            let release_fn: unsafe extern "system" fn(*mut c_void) -> u32 =
                std::mem::transmute(*ctx_vtable.add(2));
            release_fn(context);
        }

        if hr != S_OK {
            clipboard_paste(text);
        }
    }
}
