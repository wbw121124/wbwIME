#![windows_subsystem = "windows"] // 修改原因：将 GUI 从控制台子系统改为 Windows GUI 子系统，消除 TSF 宿主进程 spawn 时弹出的黑底控制台窗口

//! wbwIME 候选词窗口（Slint 原生 Rust UI）
//!
//! 使用 Slint（一等 Rust GUI 框架）渲染候选词窗口，不依赖 Qt。
//!   - 窗口置顶、无边框、半透明，支持主题配置（字体/颜色/间距/圆角/翻页图标）；
//!   - 翻页图标支持 Unicode 文本或 SVG（.svg 路径 / 内联 <svg>）；
//!   - 翻页图标位置（both/left/right）与翻页键可配置；
//!   - 由 `window().on_key_pressed` 处理系统键盘事件并翻译为引擎按键；
//!   - 鼠标点击候选/翻页图标直接触发引擎。

use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use slint::{Color, PhysicalPosition, SharedString, SharedVector};
use wbw_ime_ipc::{ToDll, ToGui};

use wbw_ime_gui::{GuiConfig, GuiState, WbwIme};

slint::include_modules!();

/// 内联 <svg> 临时文件计数器
static ICON_ID: AtomicUsize = AtomicUsize::new(0);

/// 全局引擎（单实例，Slint 事件循环线程使用）
static ENGINE: std::sync::Mutex<Option<WbwIme>> = std::sync::Mutex::new(None);

/// 一个按键的解析结果：符号名 + VK 码 + 可选字符
struct Key {
    name: &'static str,
    code: u32,
    ch: Option<char>,
}

/// 将 Slint 按键文本（event.text）解析为一个按键。
/// 具名键（如 PageUp）会被 Slint 编码为私有 Unicode 字符，此处用
/// slint::platform::Key 的字符形式识别；可打印键直接取字符（含 Shift）。
fn parse_key(text: &str) -> Option<Key> {
    use slint::platform::Key as K;

    let mut it = text.chars();
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }

    if c == char::from(K::Backspace) {
        Some(Key { name: "Backspace", code: 8, ch: None })
    } else if c == char::from(K::Return) {
        Some(Key { name: "Return", code: 13, ch: None })
    } else if c == char::from(K::Escape) {
        Some(Key { name: "Escape", code: 27, ch: None })
    } else if c == char::from(K::UpArrow) {
        Some(Key { name: "Up", code: 38, ch: None })
    } else if c == char::from(K::DownArrow) {
        Some(Key { name: "Down", code: 40, ch: None })
    } else if c == char::from(K::LeftArrow) {
        Some(Key { name: "Left", code: 37, ch: None })
    } else if c == char::from(K::RightArrow) {
        Some(Key { name: "Right", code: 39, ch: None })
    } else if c == char::from(K::PageUp) {
        Some(Key { name: "PageUp", code: 33, ch: None })
    } else if c == char::from(K::PageDown) {
        Some(Key { name: "PageDown", code: 34, ch: None })
    } else if c == char::from(K::Space) {
        Some(Key { name: "Space", code: 32, ch: None })
    } else if c == '-' {
        Some(Key { name: "Minus", code: 189, ch: None })
    } else if c == '=' {
        Some(Key { name: "Equals", code: 187, ch: None })
    } else if c.is_ascii_alphabetic() {
        Some(Key { name: "Char", code: c.to_ascii_uppercase() as u32, ch: Some(c) })
    } else if c.is_ascii_digit() {
        Some(Key { name: "Digit", code: c as u32, ch: Some(c) })
    } else {
        None
    }
}

/// 根据配置的翻页键，将按键重映射为 PageUp/PageDown，返回实际应交给引擎的按键。
fn apply_page_keys(config: &GuiConfig, key: Key) -> Key {
    let pk = &config.behavior.page_keys;
    let name = key.name;
    if pk.previous.iter().any(|s| *s == name) {
        Key { name: "PageUp", code: 33, ch: None }
    } else if pk.next.iter().any(|s| *s == name) {
        Key { name: "PageDown", code: 34, ch: None }
    } else {
        key
    }
}

/// 将引擎状态写入 Slint 组件属性，并按 visible 显隐窗口
fn apply_state(ui: &CandidateWindow, state: GuiState) {
    ui.set_buffer(state.buffer.as_str().into());
    let candidates: SharedVector<SharedString> =
        state.candidates.iter().map(|c| c.as_str().into()).collect();
    ui.set_candidates((&candidates[..]).into());
    ui.set_selected_index(state.selected_index as i32);
    ui.set_page(state.page as i32);
    ui.set_total_pages(state.total_pages as i32);
    ui.set_input_mode(state.mode.as_str().into());

    if state.visible {
        ui.window().show().ok();
    } else {
        ui.window().hide().ok();
    }
}

/// 在锁内执行一次按键处理并应用 UI 状态
fn handle_key(code: u32, ch: Option<char>, ui: &CandidateWindow) {
    let state = match ENGINE.lock() {
        Ok(mut g) => match g.as_mut() {
            Some(e) => e.process_key(code, ch),
            None => return,
        },
        Err(_) => return,
    };
    apply_state(ui, state);
}

/// 处理候选点击（第 idx 个候选，0 起），等价于数字键 idx+1
fn handle_item_click(idx: i32, ui: &CandidateWindow) {
    let code = 49 + idx as u32;
    let ch = char::from_u32(49 + idx as u32);
    handle_key(code, ch, ui);
}

/// 解析翻页图标：若为 SVG（.svg 路径或内联 <svg>）则加载为 image，否则作为文本
fn resolve_icon(value: &str) -> (slint::Image, String) {
    let t = value.trim();
    if t.ends_with(".svg") && Path::new(t).exists() {
        if let Ok(img) = slint::Image::load_from_path(Path::new(t)) {
            return (img, String::new());
        }
    } else if t.contains("<svg") {
        // 内联 SVG：写入临时文件后加载
        let id = ICON_ID.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("wbw-ime-gui-{}-{}.svg", std::process::id(), id));
        if std::fs::write(&path, t).is_ok() {
            if let Ok(img) = slint::Image::load_from_path(&path) {
                return (img, String::new());
            }
        }
    }
    (slint::Image::default(), t.to_string())
}

/// 解析 "#RRGGBB" / "#RRGGBBAA" 颜色，带透明度，失败回退白色；返回 Brush
fn color(hex: &str, alpha: f64) -> slint::Brush {
    let mut s = hex.trim().trim_start_matches('#');
    if s.len() == 6 {
        s = &s[..6];
    }
    match u32::from_str_radix(s, 16) {
        Ok(v) => {
            let r = (v >> 16) & 0xff;
            let g = (v >> 8) & 0xff;
            let b = v & 0xff;
            let a = ((alpha.clamp(0.0, 1.0) * 255.0) as u32) & 0xff;
            let encoded = (a << 24) | (r << 16) | (g << 8) | b;
            slint::Brush::SolidColor(Color::from_argb_encoded(encoded))
        }
        Err(_) => slint::Brush::SolidColor(Color::from_rgb_u8(255, 255, 255)),
    }
}

/// 将 GUI 配置（主题）应用到 Slint 组件
fn apply_config(ui: &CandidateWindow, config: &GuiConfig) {
    let win = &config.window;

    ui.set_buffer_visible(config.buffer_bar.visible);
    ui.set_pagination_visible(config.pagination.visible);
    ui.set_show_index(config.candidate_item.show_index);
    ui.set_vertical_layout(config.candidate_bar.layout.eq_ignore_ascii_case("vertical"));

    ui.set_font_family(win.font_family.as_str().into());
    ui.set_font_size(win.font_size as f32);

    ui.set_window_background(color(&win.background_color, win.opacity));
    ui.set_window_border(color(&win.border_color, 1.0));
    ui.set_window_border_width(win.border_width as f32);
    ui.set_window_radius(win.border_radius as f32);
    ui.set_window_padding(win.padding as f32);
    ui.set_window_opacity(win.opacity as f32);

    let buffer = &config.buffer_bar;
    ui.set_buffer_background(color(&buffer.background_color, 1.0));
    ui.set_buffer_text(color(&buffer.text_color, 1.0));
    ui.set_buffer_font_size(buffer.font_size as f32);
    ui.set_buffer_height(buffer.height as f32);

    let cand = &config.candidate_bar;
    ui.set_candidate_background(color(&cand.background_color, 1.0));
    ui.set_candidate_spacing(cand.spacing as f32);

    let item = &config.candidate_item;
    ui.set_item_text(color(&item.text_color, 1.0));
    ui.set_item_index_text(color(&item.index_color, 1.0));
    ui.set_item_selected_background(color(&item.selected_background_color, 1.0));
    ui.set_item_selected_text(color(&item.selected_text_color, 1.0));
    ui.set_item_radius(item.selected_border_radius as f32);
    ui.set_item_padding_h(item.padding_h as f32);
    ui.set_item_padding_v(item.padding_v as f32);
    ui.set_item_font_size(item.font_size as f32);

    let pagi = &config.pagination;
    ui.set_pagination_position(pagi.position.as_str().into());
    ui.set_icon_color(color(&pagi.icon_color, 1.0));
    ui.set_info_color(color(&pagi.info_color, 1.0));

    let (prev_img, prev_text) = resolve_icon(&pagi.prev_icon);
    ui.set_prev_image(prev_img);
    let prev_is_svg = !pagi.prev_icon.trim().is_empty()
        && (pagi.prev_icon.trim().ends_with(".svg") || pagi.prev_icon.contains("<svg"));
    ui.set_prev_is_svg(prev_is_svg);
    ui.set_prev_icon_text(if prev_text.is_empty() {
        SharedString::new()
    } else {
        prev_text.as_str().into()
    });

    let (next_img, next_text) = resolve_icon(&pagi.next_icon);
    ui.set_next_image(next_img);
    ui.set_next_is_svg(!pagi.next_icon.trim().is_empty()
        && (pagi.next_icon.trim().ends_with(".svg") || pagi.next_icon.contains("<svg")));
    ui.set_next_icon_text(if next_text.is_empty() {
        SharedString::new()
    } else {
        next_text.as_str().into()
    });
}

/// 估算文本像素宽度（CJK 字符≈字号，ASCII 字符≈0.55×字号）
fn estimate_text_width(text: &str, font_size: f32) -> f32 {
    let mut width = 0.0f32;
    for ch in text.chars() {
        if ch.is_ascii() {
            width += font_size * 0.55;
        } else {
            // CJK 字符或其它非 ASCII 字符
            width += font_size;
        }
    }
    width
}

/// 根据缓冲和候选词估算窗口宽度
fn calc_window_width(config: &GuiConfig, buffer: &str, candidates: &[String]) -> f32 {
    let padding = config.window.padding as f32 * 2.0;
    let mode_indicator_width = 24.0; // 模式指示器宽度
    let spacing = config.candidate_bar.spacing as f32;

    // 缓冲栏宽度 = 模式指示器 + 缓冲文本
    let buffer_text_width = estimate_text_width(buffer, config.buffer_bar.font_size as f32);
    let buffer_width = mode_indicator_width + buffer_text_width;

    // 候选栏宽度
    let item_font_size = config.candidate_item.font_size as f32;
    let mut candidates_width = 0.0f32;
    for (i, candidate) in candidates.iter().enumerate() {
        let idx_width = if config.candidate_item.show_index {
            estimate_text_width(&(i + 1).to_string(), item_font_size) + estimate_text_width(". ", item_font_size)
        } else {
            0.0
        };
        candidates_width += idx_width + estimate_text_width(candidate, item_font_size);
        if i < candidates.len() - 1 {
            candidates_width += spacing;
        }
    }

    // 取缓冲栏和候选栏中较宽的
    let content_width = buffer_width.max(candidates_width);
    (content_width + padding).max(100.0) // 最小宽度 100px
}

/// 将 IPC 下发的 Show 消息应用到窗口并定位到光标屏幕坐标
fn apply_ipc_show(ui: &CandidateWindow, msg: ToGui, config: &GuiConfig) {
    let ToGui::Show {
        buffer,
        candidates,
        selected,
        page,
        total_pages,
        x,
        y,
        mode,
    } = msg
    else {
        return;
    };

    ui.set_buffer(buffer.as_str().into());
    let vec: SharedVector<SharedString> =
        candidates.iter().map(|c| c.as_str().into()).collect();
    ui.set_candidates((&vec[..]).into());
    ui.set_selected_index(selected as i32);
    ui.set_page(page as i32);
    ui.set_total_pages(total_pages as i32);
    ui.set_input_mode(mode.as_str().into());

    // 动态计算窗口宽度
    let width = calc_window_width(config, &buffer, &candidates);
    let height = ui.window().size().height;
    ui.window().set_size(slint::LogicalSize::new(width, height as f32));

    // 窗口左上角定位在光标下方（该坐标为 TSF GetScreenCoords 给出的物理像素）
    // x/y 由 DLL 通过 IPC 推送，非 fallback 值（fallback 为 -320/-120）
    let is_fallback = x <= -320 && y <= -120;
    if !is_fallback {
        ui.window().set_position(PhysicalPosition::new(x, y + 2));
    } else {
        // 使用 fallback 坐标（DLL 未提供有效 TSF 坐标时的兜底）
        ui.window().set_position(PhysicalPosition::new(x, y + 2));
    }
    ui.window().show().ok();
}

/// 跟随光标定位窗口并应用到 UI 状态（钩子模式无 TSF 坐标可用）
fn apply_state_cursor(ui: &CandidateWindow, state: GuiState) {
    let was_before = ui.window().is_visible();
    apply_state(ui, state.clone());
    let show_now = ui.window().is_visible();
    if show_now != was_before {
        wbw_ime_gui::logf!("window visibility {was_before} -> {show_now} (buffer={:?})", state.buffer);
    }
    if show_now && (!was_before || state.visible)
        && state.fallback_position {
        // hook 模式：回退到鼠标指针位置
        unsafe {
            let mut pt: windows_sys::Win32::Foundation::POINT = std::mem::zeroed();
            if windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt) != 0 {
                ui.window().set_position(PhysicalPosition::new(pt.x as i32, pt.y as i32 + 2));
            }
        }
    }
    // TSF 模式：坐标已由 IPC 推送，无需额外定位
}

/// 待上屏文本队列（钩子线程提交，UI 线程异步粘贴）
static PENDING_PASTE: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// 剪贴板操作互斥锁（防止多线程同时操作剪贴板）
static CLIPBOARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 剪贴板 + 模拟 Ctrl+V 粘贴（须在钩子线程执行，避免影响 UI 事件循环）
fn hook_paste(text: &str) {
    let _guard = CLIPBOARD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    wbw_ime_gui::logf!("hook_paste begin text={:?}", text);
    unsafe {
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
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let size = wide.len() * 2;
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
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

/// 钩子模式主入口：全局键盘钩子喂键给本地引擎，候选窗跟随光标，剪贴板上屏。
fn run_hook_mode(config: &GuiConfig) {
    if wbw_ime_gui::hook::acquire_single_instance().is_none() {
        // 已有实例，直接退出本进程
        return;
    }

    let engine = WbwIme::new(config.clone(), config.page_size);
    *ENGINE.lock().unwrap() = Some(engine);

    let ui: Rc<CandidateWindow> = CandidateWindow::new().unwrap().into();
    apply_config(&ui, config);

    // 鼠标：选候选 / 翻页（复用引擎，缓解钩子线程锁竞争）
    let ui2 = ui.clone();
    ui.on_item_clicked(move |idx| handle_item_click(idx, &ui2));
    let ui3 = ui.clone();
    ui.on_prev_page(move || handle_key(33, None, &ui3));
    let ui4 = ui.clone();
    ui.on_next_page(move || handle_key(34, None, &ui4));

    // 钩子线程 → UI 线程的回传通道
    let (tx, rx) = mpsc::channel::<GuiState>();
    let tx2 = tx.clone();
    wbw_ime_gui::hook::start(Box::new(move |code, ch| {
        wbw_ime_gui::logf!("hook_key code={} ch={:?}", code, ch);
        let state = {
            let mut guard = match ENGINE.lock() {
                Ok(g) => g,
                Err(_) => return GuiState::default(),
            };
            match guard.as_mut() {
                Some(e) => e.process_key(code, ch),
                None => return GuiState::default(),
            }
        };
        // hook 模式标记使用 fallback 位置
        let mut state = state;
        state.fallback_position = true;
        if let Some(text) = state.committed.as_ref() {
            if !text.is_empty() {
                wbw_ime_gui::logf!("hook commit text={:?} buffer={:?}", text, state.buffer);
                PENDING_PASTE.lock().unwrap().push(text.clone());
            }
        }
        let _ = tx2.send(state.clone());
        state
    }));

    // 每 10ms 排空钩子回传，应用 UI 状态，并异步粘贴待上屏文本
    let ui_timer = ui.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
        let mut first = true;
        while let Ok(state) = rx.try_recv() {
            if first {
                wbw_ime_gui::logf!("ui recv buffer={:?} visible={} pending_commit={:?}", state.buffer, state.visible, state.committed);
                first = false;
            }
            apply_state_cursor(&ui_timer, state);
        }
        let texts: Vec<String> = {
            let mut guard = PENDING_PASTE.lock().unwrap_or_else(|e| e.into_inner());
            std::mem::take(&mut *guard)
        };
        for text in texts {
            wbw_ime_gui::logf!("perform paste text={:?}", text);
            hook_paste(&text);
        }
    });

    ui.window().hide().ok();
    slint::run_event_loop_until_quit().unwrap();
}

/// IPC 模式主入口：监听 DLL，仅显示 + 点击回传，键盘由 DLL 处理。
fn run_ipc_mode(config: &GuiConfig) {
    // 修改原因：多宿主进程都会 spawn 本 GUI，靠命名 Mutex 保证全系统只有一个 IPC 实例，
    // 其余进程直接退出，避免无限弹窗与端口冲突。
    if wbw_ime_gui::hook::acquire_single_instance_ipc().is_none() {
        return;
    }

    let ui: Rc<CandidateWindow> = CandidateWindow::new().unwrap().into();
    apply_config(&ui, config);
    ui.window().hide().ok();

    // 鼠标点击 → 回传 DLL（键盘由 DLL 处理，窗口仅作显示）
    ui.on_item_clicked(move |idx| wbw_ime_gui::ipc::send(&ToDll::Select(idx as usize)));
    ui.on_prev_page(move || wbw_ime_gui::ipc::send(&ToDll::PageUp));
    ui.on_next_page(move || wbw_ime_gui::ipc::send(&ToDll::PageDown));

    // 启动 IPC 服务端，收到 Show/Hide 推入通道
    let (tx, rx) = mpsc::channel::<ToGui>();
    // 修改原因：端口绑定失败说明已有其他进程占用（或端口不可用），直接退出，不弹无效空窗口
    if !wbw_ime_gui::ipc::spawn(tx) {
        return;
    }

    // UI 线程定时排空通道并应用状态
    let ui_timer = ui.clone();
    let config_timer = config.clone();
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(10), move || {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                ToGui::Show { .. } => apply_ipc_show(&ui_timer, msg, &config_timer),
                ToGui::Hide => {
                    ui_timer.window().hide().ok();
                }
            }
        }
    });

    // IPC 模式窗口会频繁 show/hide，需让事件循环持续运行直到进程退出，
    // 避免最后一次窗口隐藏导致事件循环自行结束。
    slint::run_event_loop_until_quit().unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ipc_mode = args.iter().any(|a| a == "--ipc");
    let hook_mode = args.iter().any(|a| a == "--hook");
    let non_flag: Vec<String> = args
        .iter()
        .filter(|a| *a != "--ipc" && *a != "--hook")
        .cloned()
        .collect();
    let config_path = non_flag
        .get(1)
        .cloned()
        .unwrap_or_else(|| "resources/gui-config.yaml".to_string());
    let page_size_arg: Option<usize> = non_flag.get(2).and_then(|s| s.parse().ok());

    let config = GuiConfig::from_file(&config_path);

    if hook_mode {
        run_hook_mode(&config);
        return;
    }

    if ipc_mode {
        run_ipc_mode(&config);
        return;
    }

    let page_size = page_size_arg.unwrap_or(config.page_size);
    let engine = WbwIme::new(config.clone(), page_size);

    *ENGINE.lock().unwrap() = Some(engine);

    let ui: Rc<CandidateWindow> = CandidateWindow::new().unwrap().into();

    apply_config(&ui, &config);

    // 鼠标：选候选
    let ui2 = ui.clone();
    ui.on_item_clicked(move |idx| handle_item_click(idx, &ui2));

    // 鼠标：翻页
    let ui3 = ui.clone();
    ui.on_prev_page(move || handle_key(33, None, &ui3));
    let ui4 = ui.clone();
    ui.on_next_page(move || handle_key(34, None, &ui4));

    // 键盘（来自 FocusScope.route-key，含翻页键重映射）
    let cfg = config.clone();
    let ui5 = ui.clone();
    ui.on_route_key(move |text| {
        let text = text.to_string();
        if let Some(key) = parse_key(&text) {
            let key = apply_page_keys(&cfg, key);
            handle_key(key.code, key.ch, &ui5);
        }
    });

    // 初始隐藏（输入时才显示）
    ui.window().hide().ok();

    slint::run_event_loop().unwrap();
}
