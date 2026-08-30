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

use slint::{Color, SharedString, SharedVector};

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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "resources/gui-config.yaml".to_string());
    let page_size_arg: Option<usize> = args.get(2).and_then(|s| s.parse().ok());

    let config = GuiConfig::from_file(&config_path);
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
