//! wbwIME native C API
//!
//! 将输入法引擎编译为 C 兼容的动态库（Windows DLL / Linux .so），
//! 供 IME 宿主框架（TSF/IMM32/IBus/Fcitx5）调用。

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;
use std::ptr;

use wbw_dict::{DictBuilder, FstDict};
use wbw_imekit::ime_host::ImeResponseType;
use wbw_imekit::key_mapper::KeyEvent;
use wbw_imekit::{ImeConfig, ImeHost, ImeResponse, ImeState};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::Ranker;
use wbw_types::{Candidate, InputMode, RankConfig};

/// IME 实例（opaque 指针，外部不可直接访问内部字段）
pub struct WbwIme {
    host: ImeHost,
    matcher: Matcher,
    ranker: Ranker,
}

/// 候选词信息（C 兼容）
#[repr(C)]
pub struct WbwCandidate {
    pub text: *mut c_char,
    pub code: *mut c_char,
    pub score: f64,
}

/// IME 响应结果
#[repr(C)]
pub struct WbwImeResult {
    pub response_type: u32,
    pub buffer: *mut c_char,
    pub cursor: u32,
    pub candidates: *mut WbwCandidate,
    pub candidate_count: u32,
    pub need_refresh: bool,
    pub need_hide: bool,
    pub confirmed_text: *mut c_char,
}

/// 状态枚举（C 兼容）
#[repr(C)]
#[derive(Clone, Copy)]
pub enum WbwImeState {
    Idle = 0,
    Inputting = 1,
    Selecting = 2,
    Confirming = 3,
    Error = 4,
}

// ========== 生命周期 ==========

/// 创建 IME 实例
///
/// # Safety
/// `dict_path` 必须是有效的 C 字符串指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_create(dict_path: *const c_char) -> *mut WbwIme {
    if dict_path.is_null() {
        return ptr::null_mut();
    }

    let path_str = match CStr::from_ptr(dict_path).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let path = Path::new(path_str);

    if !path.exists() {
        return ptr::null_mut();
    }

    // 从 .cin 或 .fst 加载词典
    let dict = match path.extension().and_then(|e| e.to_str()) {
        Some("fst") => match FstDict::from_file(path) {
            Ok(d) => d,
            Err(_) => return ptr::null_mut(),
        },
        _ => {
            let mut builder = DictBuilder::new();
            if builder.load_cin(path).is_err() {
                return ptr::null_mut();
            }
            builder.deduplicate();
            builder.sort();
            match builder.build_fst() {
                Ok(d) => d,
                Err(_) => return ptr::null_mut(),
            }
        }
    };

    let matcher_config = MatcherConfig {
        fuzzy_enabled: true,
        ..MatcherConfig::default()
    };
    let matcher = Matcher::with_dict(matcher_config, dict);

    let rank_config = RankConfig::default();
    let ranker = Ranker::new(rank_config);

    let host = ImeHost::new(ImeConfig::default());

    let ime = Box::new(WbwIme {
        host,
        matcher,
        ranker,
    });

    Box::into_raw(ime)
}

/// 销毁 IME 实例
///
/// # Safety
/// `ime` 必须是由 `wbw_ime_create` 返回的有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_destroy(ime: *mut WbwIme) {
    if !ime.is_null() {
        drop(Box::from_raw(ime));
    }
}

// ========== 输入处理 ==========

/// 处理按键事件
///
/// # Safety
/// `ime` 必须是有效指针。返回结果需通过 `wbw_ime_result_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_process_key(
    ime: *mut WbwIme,
    key_code: u32,
    key_char: u32,
) -> *mut WbwImeResult {
    if ime.is_null() {
        return ptr::null_mut();
    }
    let ime = &mut *ime;

    let ch = char::from_u32(key_char);
    let key = KeyEvent::new(key_code, ch);

    let response = match ime.host.process_key(key) {
        Ok(r) => r,
        Err(_) => return ptr::null_mut(),
    };

    // 如果有字符输入，做匹配
    let candidates = if matches!(response.response_type, ImeResponseType::InputChar) {
        let ctx = wbw_types::InputContext {
            buffer: response.buffer.clone(),
            cursor: response.cursor,
            mode: InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let matched = ime.matcher.match_input(&ctx);
        ime.ranker.rank(&matched)
    } else {
        Vec::new()
    };

    convert_response(&response, &candidates)
}

/// 直接输入字符串
///
/// # Safety
/// `ime` 和 `text` 必须是有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_input_text(
    ime: *mut WbwIme,
    text: *const c_char,
) -> *mut WbwImeResult {
    if ime.is_null() || text.is_null() {
        return ptr::null_mut();
    }
    let ime = &mut *ime;
    let text_str = match CStr::from_ptr(text).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };

    let mut buffer = String::new();
    for ch in text_str.chars() {
        if ch.is_ascii_alphanumeric() {
            buffer.push(ch);
        }
    }

    let mut candidates = Vec::new();
    if !buffer.is_empty() {
        let ctx = wbw_types::InputContext {
            buffer: buffer.clone(),
            cursor: buffer.len(),
            mode: InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let matched = ime.matcher.match_input(&ctx);
        candidates = ime.ranker.rank(&matched);
    }

    let buffer_len = buffer.len();
    let response = ImeResponse {
        response_type: ImeResponseType::InputChar,
        text: None,
        candidates: candidates.clone(),
        buffer,
        cursor: buffer_len,
        need_refresh: true,
        need_hide: false,
    };

    convert_response(&response, &candidates)
}

/// 获取当前候选列表
///
/// # Safety
/// `ime` 必须是有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_get_candidates(ime: *const WbwIme) -> *mut WbwImeResult {
    if ime.is_null() {
        return ptr::null_mut();
    }
    // 返回当前候选列表（stub：非 InputChar 操作时宿主已维护候选状态）
    let _ime = &*ime;
    let response = ImeResponse {
        response_type: ImeResponseType::ShowCandidates,
        text: None,
        candidates: vec![],
        buffer: String::new(),
        cursor: 0,
        need_refresh: false,
        need_hide: false,
    };
    convert_response(&response, &[])
}

// ========== 查询 ==========

/// 获取当前状态
///
/// # Safety
/// `ime` 必须是有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_get_state(ime: *const WbwIme) -> WbwImeState {
    if ime.is_null() {
        return WbwImeState::Error;
    }
    let ime = &*ime;
    match ime.host.state() {
        ImeState::Idle => WbwImeState::Idle,
        ImeState::Inputting => WbwImeState::Inputting,
        ImeState::Selecting => WbwImeState::Selecting,
        ImeState::Confirming => WbwImeState::Confirming,
        ImeState::Error => WbwImeState::Error,
    }
}

/// 获取当前输入缓冲区
///
/// # Safety
/// 返回的字符串需通过 `wbw_ime_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_get_buffer(ime: *const WbwIme) -> *mut c_char {
    if ime.is_null() {
        return ptr::null_mut();
    }
    let ime = &*ime;
    CString::new(ime.host.buffer().to_string())
        .unwrap_or_default()
        .into_raw()
}

/// 重置 IME 状态
///
/// # Safety
/// `ime` 必须是有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_reset(ime: *mut WbwIme) {
    if ime.is_null() {
        return;
    }
    let ime = &mut *ime;
    ime.host.reset();
}

/// 获取版本号
///
/// 返回的字符串需通过 `wbw_ime_string_free` 释放。
#[no_mangle]
pub extern "C" fn wbw_ime_version() -> *mut c_char {
    CString::new(env!("CARGO_PKG_VERSION"))
        .unwrap_or_default()
        .into_raw()
}

// ========== 内存管理 ==========

/// 释放 IME 响应结果
///
/// # Safety
/// `result` 必须是由 `wbw_ime_process_key` 或 `wbw_ime_input_text` 返回的有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_result_free(result: *mut WbwImeResult) {
    if result.is_null() {
        return;
    }
    let result = Box::from_raw(result);

    if !result.buffer.is_null() {
        drop(CString::from_raw(result.buffer));
    }
    if !result.confirmed_text.is_null() {
        drop(CString::from_raw(result.confirmed_text));
    }
    if !result.candidates.is_null() && result.candidate_count > 0 {
        let slice = std::slice::from_raw_parts_mut(result.candidates, result.candidate_count as usize);
        let boxed: Box<[WbwCandidate]> = Box::from_raw(slice);
        for c in boxed.iter() {
            if !c.text.is_null() {
                drop(CString::from_raw(c.text));
            }
            if !c.code.is_null() {
                drop(CString::from_raw(c.code));
            }
        }
    }
}

/// 释放 C 字符串
///
/// # Safety
/// `s` 必须是由本模块中函数返回的有效指针。
#[no_mangle]
pub unsafe extern "C" fn wbw_ime_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

// ========== 内部辅助 ==========

unsafe fn convert_response(response: &ImeResponse, candidates: &[Candidate]) -> *mut WbwImeResult {
    let buffer = CString::new(response.buffer.clone()).unwrap_or_default();
    let confirmed_text = response
        .text
        .as_ref()
        .map(|t| CString::new(t.clone()).unwrap_or_default());

    let response_type = match response.response_type {
        ImeResponseType::None => 0,
        ImeResponseType::InputChar => 1,
        ImeResponseType::DeleteChar => 2,
        ImeResponseType::Confirm => 3,
        ImeResponseType::Cancel => 4,
        ImeResponseType::SwitchMode => 5,
        ImeResponseType::ShowCandidates => 6,
        ImeResponseType::HideCandidates => 7,
        ImeResponseType::Error => 8,
    };

    let c_candidates: Vec<WbwCandidate> = candidates
        .iter()
        .map(|c| WbwCandidate {
            text: CString::new(c.text.clone()).unwrap_or_default().into_raw(),
            code: CString::new(c.code.clone()).unwrap_or_default().into_raw(),
            score: c.score,
        })
        .collect();

    let candidate_count = c_candidates.len() as u32;
    let candidates_ptr = if c_candidates.is_empty() {
        ptr::null_mut()
    } else {
        let mut boxed = c_candidates.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        std::mem::forget(boxed);
        ptr
    };

    Box::into_raw(Box::new(WbwImeResult {
        response_type,
        buffer: buffer.into_raw(),
        cursor: (response.cursor as u32).min(response.buffer.len() as u32),
        candidates: candidates_ptr,
        candidate_count,
        need_refresh: response.need_refresh,
        need_hide: response.need_hide,
        confirmed_text: confirmed_text
            .map(|s| s.into_raw())
            .unwrap_or(ptr::null_mut()),
    }))
}
