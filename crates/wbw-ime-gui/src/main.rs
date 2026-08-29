//! wbwIME Qt/QML 候选窗口入口
//!
//! 仅在启用 `qt` feature 时编译（见 Cargo.toml 的 `required-features = ["qt"]`）。
#![allow(non_snake_case)]
//!
//! 本文件：
//!   - 用 `qmetaobject` 绑定 QML 渲染候选词窗口；
//!   - 将 Qt 键值翻译为引擎按键并实时刷新候选栏；
//!   - 窗口置顶、半透明、无边框，支持主题配置与翻页。
//!
//! 参考键值：
//!   Qt.Key_Return = 16777220, Key_Backspace = 16777219, Key_Escape = 16777216,
//!   Key_Up = 16777235, Key_Down = 16777237, Key_PageUp = 16777238, Key_PageDown = 16777239,
//!   字母 = 65..=90, 数字 = 48..=57, 空格 = 32。
//!   引擎（imekit）侧采用 VK 码：Enter=13, Backspace=8, Esc=27, Up=38, Down=40,
//!   PageUp=33, PageDown=34，故此处需要做一次映射。

use std::sync::Mutex;

use cstr::cstr;
use qmetaobject::prelude::*;
use qmetaobject::{QmlEngine, QStringList};

use wbw_ime_gui::{GuiConfig, WbwIme};

/// 全局引擎（单实例，Qt 主线程使用）
static ENGINE: Mutex<Option<WbwIme>> = Mutex::new(None);

/// 将 Qt 键值映射到 imekit/VK 键码
fn qt_key_to_vk(qt_key: i32, shift: bool) -> Option<(u32, Option<char>)> {
    match qt_key {
        k if (65..=90).contains(&k) => {
            let ch = if shift {
                (k as u8) as char
            } else {
                ((k as u8) + 32) as char
            };
            Some((k as u32, Some(ch)))
        }
        k if (48..=57).contains(&k) => Some((k as u32, Some((k as u8) as char))),
        32 => Some((32, None)),
        16777220 => Some((13, None)), // Return
        16777221 => Some((13, None)), // Enter (keypad)
        16777219 => Some((8, None)),  // Backspace
        16777216 => Some((27, None)), // Esc
        16777235 => Some((38, None)), // Up
        16777237 => Some((40, None)), // Down
        16777238 => Some((33, None)), // PageUp
        16777239 => Some((34, None)), // PageDown
        _ => None,
    }
}

/// 候选窗口控制器（由 QML 实例化，向 QML 暴露属性与方法）
#[derive(Default, QObject)]
struct CandidateController {
    base: qt_base_class!(trait QObject),

    /// 缓冲栏文本
    buffer: qt_property!(QString; NOTIFY stateChanged),
    /// 当前页候选词
    candidates: qt_property!(QStringList; NOTIFY stateChanged),
    /// 当前选中索引
    selected_index: qt_property!(i32; NOTIFY stateChanged),
    /// 当前页（0 起）
    page: qt_property!(i32; NOTIFY stateChanged),
    /// 总页数
    total_pages: qt_property!(i32; NOTIFY stateChanged),
    /// 是否有候选（用于显示缓冲栏切换）
    has_candidates: qt_property!(bool; NOTIFY stateChanged),

    stateChanged: qt_signal!(),

    /// 处理 Qt 按键（由 QML 的 Keys 回调）
    key_pressed: qt_method!(fn key_pressed(&mut self, qt_key: i32, shift: bool) {
        let Some((code, ch)) = qt_key_to_vk(qt_key, shift) else {
            return;
        };
        let state = {
            let mut guard = ENGINE.lock().unwrap();
            let engine = match guard.as_mut() {
                Some(e) => e,
                None => return,
            };
            engine.process_key(code, ch)
        };
        self.apply_state(state);
    }),
}

impl CandidateController {
    /// 将引擎状态写入 QML 属性
    fn apply_state(&mut self, state: wbw_ime_gui::GuiState) {
        let mut list = QStringList::new();
        for c in &state.candidates {
            list.push(c.clone().into());
        }
        self.buffer = state.buffer.into();
        self.candidates = list;
        self.selected_index = state.selected_index as i32;
        self.page = state.page as i32;
        self.total_pages = state.total_pages as i32;
        self.has_candidates = state.visible;
        self.stateChanged();
    }
}

/// 生成 QML 源码（内联，避免运行时文件依赖）
fn qml_source(config: &GuiConfig) -> String {
    let window = &config.window;
    let buffer = &config.buffer_bar;
    let candidate_bar = &config.candidate_bar;
    let item = &config.candidate_item;
    let pagi = &config.pagination;

    let buffer_visible = if buffer.visible { "true" } else { "false" };
    let pagi_visible = if pagi.visible { "true" } else { "false" };
    let show_index = if item.show_index { "true" } else { "false" };

    let win_h = buffer.height as i64 + 40 + if pagi.visible { 26 } else { 0 };

    format!(
        r#"
import QtQuick 2.6
import QtQuick.Window 2.0
import WbwIme 1.0

Window {{
    id: win
    visible: true
    flags: Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus
    color: "transparent"
    width: 380
    height: {win_h}
    opacity: {opacity}

    CandidateController {{
        id: controller
        objectName: "controller"
    }}

    Rectangle {{
        id: root
        anchors.fill: parent
        color: "{bg}"
        radius: {radius}
        border.color: "{border_color}"
        border.width: {border_width}
        clip: true

        Keys.onPressed: {{
            if (event.key >= 16777249 && event.key <= 16777254) {{ event.accepted = true; return }}
            controller.key_pressed(event.key, event.modifiers & Qt.ShiftModifier)
            event.accepted = true
        }}
        focus: true
        Component.onCompleted: forceActiveFocus()

        Column {{
            anchors.fill: parent
            anchors.margins: {padding}

            // 缓冲栏
            Rectangle {{
                visible: {buffer_visible} && controller.buffer !== ""
                width: parent.width
                height: {buffer_h}
                color: "{bar_bg}"
                radius: 3
                Text {{
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: 6
                    text: controller.buffer
                    color: "{bar_text}"
                    font.pixelSize: {bar_font}
                    font.family: "{font_name}"
                }}
            }}

            // 候选栏
            Row {{
                width: parent.width
                height: {item_row_h}
                spacing: {spacing}
                Repeater {{
                    model: controller.candidates
                    Rectangle {{
                        height: parent.height
                        width: text.width + {pad_h} * 2
                        color: (index === controller.selectedIndex) ? "{sel_bg}" : "transparent"
                        radius: {sel_radius}
                        Text {{
                            id: text
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.left: parent.left
                            anchors.leftMargin: {pad_h}
                            color: (parent.color === "{sel_bg}") ? "{sel_text}" : "{text_color}"
                            font.pixelSize: {item_font}
                            font.family: "{font_name}"
                            text: ({show_index}) ? (index+1) + ". " + modelData : modelData
                        }}
                        MouseArea {{
                            anchors.fill: parent
                            onClicked: controller.key_pressed(49 + index, false)
                        }}
                    }}
                }}
            }}

            // 翻页区
            Rectangle {{
                visible: {pagi_visible}
                width: parent.width
                height: 22
                color: "transparent"
                Text {{
                    anchors.left: parent.left
                    anchors.verticalCenter: parent.verticalCenter
                    text: "{prev_icon}"
                    color: "{icon_color}"
                    font.pixelSize: 14
                    MouseArea {{ anchors.fill: parent; onClicked: controller.key_pressed(16777238, false) }}
                }}
                Text {{
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.verticalCenter: parent.verticalCenter
                    text: (controller.page + 1) + "/" + controller.totalPages
                    color: "{info_color}"
                    font.pixelSize: 12
                }}
                Text {{
                    anchors.right: parent.right
                    anchors.verticalCenter: parent.verticalCenter
                    text: "{next_icon}"
                    color: "{icon_color}"
                    font.pixelSize: 14
                    MouseArea {{ anchors.fill: parent; onClicked: controller.key_pressed(16777239, false) }}
                }}
            }}
        }}
    }}
}}
"#,
        opacity = window.opacity,
        bg = window.background_color,
        radius = window.border_radius,
        border_color = window.border_color,
        border_width = window.border_width,
        padding = window.padding,
        buffer_visible = buffer_visible,
        buffer_h = buffer.height,
        bar_bg = buffer.background_color,
        bar_text = buffer.text_color,
        bar_font = buffer.font_size,
        font_name = window.font_name,
        item_row_h = item.font_size + item.padding_v * 2,
        spacing = candidate_bar.spacing,
        pad_h = item.padding_h,
        sel_bg = item.selected_background_color,
        sel_radius = item.selected_border_radius,
        sel_text = item.selected_text_color,
        text_color = item.text_color,
        item_font = item.font_size,
        show_index = show_index,
        pagi_visible = pagi_visible,
        prev_icon = pagi.prev_icon,
        next_icon = pagi.next_icon,
        icon_color = pagi.icon_color,
        info_color = pagi.info_color,
        win_h = win_h,
    )
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

    // 初始化全局引擎（单实例）
    *ENGINE.lock().unwrap() = Some(engine);

    let mut engine_qml = QmlEngine::new();

    // 注册 Rust 类型到 QML
    qml_register_type::<CandidateController>(
        cstr!("WbwIme"),
        1,
        0,
        cstr!("CandidateController"),
    );

    let qml = qml_source(&config);
    engine_qml.load_data(qml.into());

    engine_qml.exec();
}
