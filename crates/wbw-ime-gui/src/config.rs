//! GUI 配置模块
//!
//! 使用 YAML 提供高度可配置的候选窗口外观与行为：
//! 窗口、缓冲栏、候选栏、候选条目、翻页图标等。

use serde::Deserialize;

/// 应用程序配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    /// 预设主题名称：dark / light / dark_plus / light_plus。
    /// 预设只决定颜色，其它非颜色字段由 YAML/默认值决定。
    pub theme: String,
    /// 词典（.cin 或 .fst）路径
    pub dict_path: String,
    /// 每页候选词数量
    pub page_size: usize,
    /// 窗口配置
    pub window: WindowConfig,
    /// 缓冲栏配置
    pub buffer_bar: BufferBarConfig,
    /// 候选栏配置
    pub candidate_bar: CandidateBarConfig,
    /// 候选条目配置
    pub candidate_item: CandidateItemConfig,
    /// 翻页图标配置
    pub pagination: PaginationConfig,
    /// 行为配置
    pub behavior: BehaviorConfig,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            theme: "custom".to_string(),
            dict_path: "resources/dicts/base.cin".to_string(),
            page_size: 10,
            window: WindowConfig::default(),
            buffer_bar: BufferBarConfig::default(),
            candidate_bar: CandidateBarConfig::default(),
            candidate_item: CandidateItemConfig::default(),
            pagination: PaginationConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

/// 窗口配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// 背景颜色（十六进制，如 #FFFFFF）
    pub background_color: String,
    /// 边框颜色
    pub border_color: String,
    /// 边框宽度
    pub border_width: u32,
    /// 圆角半径
    pub border_radius: u32,
    /// 内边距
    pub padding: u32,
    /// 透明度（0.0 ~ 1.0）
    pub opacity: f64,
    /// 字体族（支持多个，逗号分隔，如 "Microsoft YaHei, SimHei, sans-serif"）
    pub font_family: String,
    /// 字体大小
    pub font_size: u32,
    /// OpenType 字体特性（如 "liga 1, calt 1, tnum 1"）。
    /// 注：Slint 当前版本暂未提供 font-feature-settings 渲染属性，
    /// 该字段作为配置数据保留，待 Slint 支持后启用。
    pub font_feature_settings: String,
    /// 是否置顶
    pub always_on_top: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            background_color: "#FFFFFF".to_string(),
            border_color: "#CCCCCC".to_string(),
            border_width: 1,
            border_radius: 4,
            padding: 8,
            opacity: 1.0,
            font_family: "Microsoft YaHei, SimHei, sans-serif".to_string(),
            font_size: 14,
            font_feature_settings: String::new(),
            always_on_top: true,
        }
    }
}

/// 缓冲栏（当前拼写）配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BufferBarConfig {
    /// 是否显示缓冲栏
    pub visible: bool,
    /// 文本颜色
    pub text_color: String,
    /// 背景颜色
    pub background_color: String,
    /// 字体大小
    pub font_size: u32,
    /// 高度
    pub height: u32,
    /// 文字对齐（left/center/right）
    pub align: String,
}

impl Default for BufferBarConfig {
    fn default() -> Self {
        Self {
            visible: true,
            text_color: "#333333".to_string(),
            background_color: "#F5F5F5".to_string(),
            font_size: 14,
            height: 28,
            align: "left".to_string(),
        }
    }
}

/// 候选栏配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CandidateBarConfig {
    /// 背景颜色
    pub background_color: String,
    /// 候选词间距
    pub spacing: u32,
    /// 候选词排列方向（horizontal/vertical）
    pub layout: String,
}

impl Default for CandidateBarConfig {
    fn default() -> Self {
        Self {
            background_color: "#FFFFFF".to_string(),
            spacing: 4,
            layout: "horizontal".to_string(),
        }
    }
}

/// 候选条目配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct CandidateItemConfig {
    /// 文本颜色
    pub text_color: String,
    /// 选中项文本颜色
    pub selected_text_color: String,
    /// 选中项背景颜色
    pub selected_background_color: String,
    /// 选中项圆角半径
    pub selected_border_radius: u32,
    /// 水平内边距
    pub padding_h: u32,
    /// 垂直内边距
    pub padding_v: u32,
    /// 字体大小
    pub font_size: u32,
    /// 是否显示序号（1. 第一候选）
    pub show_index: bool,
    /// 序号颜色
    pub index_color: String,
}

impl Default for CandidateItemConfig {
    fn default() -> Self {
        Self {
            text_color: "#000000".to_string(),
            selected_text_color: "#FFFFFF".to_string(),
            selected_background_color: "#0078D4".to_string(),
            selected_border_radius: 4,
            padding_h: 6,
            padding_v: 2,
            font_size: 15,
            show_index: true,
            index_color: "#888888".to_string(),
        }
    }
}

/// 翻页图标配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PaginationConfig {
    /// 是否显示翻页区
    pub visible: bool,
    /// 翻页图标位置：both（两端）/ left（靠左）/ right（靠右）
    pub position: String,
    /// 上一页图标：Unicode 文本（如 ◀）或 SVG（.svg 路径 / data:image/svg+xml 字符串）
    pub prev_icon: String,
    /// 下一页图标：Unicode 文本（如 ▶）或 SVG（.svg 路径 / data:image/svg+xml 字符串）
    pub next_icon: String,
    /// 图标颜色（仅文本图标生效）
    pub icon_color: String,
    /// 提示文字颜色（如 1/3 页）
    pub info_color: String,
}

impl Default for PaginationConfig {
    fn default() -> Self {
        Self {
            visible: true,
            position: "both".to_string(),
            prev_icon: "◀".to_string(),
            next_icon: "▶".to_string(),
            icon_color: "#666666".to_string(),
            info_color: "#999999".to_string(),
        }
    }
}

/// 翻页键（触发上一页 / 下一页的按键）
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct PageKeys {
    /// 触发上一页的按键，可选：PageUp / Up / Left / Minus
    pub previous: Vec<String>,
    /// 触发下一页的按键，可选：PageDown / Down / Right / Equals
    pub next: Vec<String>,
}

impl Default for PageKeys {
    fn default() -> Self {
        Self {
            previous: vec!["PageUp".to_string()],
            next: vec!["PageDown".to_string()],
        }
    }
}

/// 行为配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 是否启用学习
    pub l0_enabled: bool,
    /// 空格是否确认（true=上屏候选/缓冲，false=输入空格）
    pub space_confirms: bool,
    /// 数字键 1-9 是否选候选
    pub digit_selects: bool,
    /// 是否启用回车确认
    pub enter_confirms: bool,
    /// 翻页键设置
    pub page_keys: PageKeys,
    /// 中英文切换键（默认 "Shift"）
    pub toggle_key: String,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            fuzzy_enabled: true,
            l0_enabled: true,
            space_confirms: true,
            digit_selects: true,
            enter_confirms: true,
            page_keys: PageKeys::default(),
            toggle_key: "Shift".to_string(),
        }
    }
}

impl GuiConfig {
    /// 从 YAML 字符串解析配置，失败时回退到默认值
    pub fn from_yaml(yaml: &str) -> Self {
        let mut cfg: Self = serde_yaml::from_str(yaml).unwrap_or_default();
        cfg.apply_theme(&cfg.theme.clone());
        cfg
    }

    /// 从文件加载配置，失败或文件不存在时回退到默认值
    pub fn from_file(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => Self::from_yaml(&content),
            Err(e) => {
                eprintln!("[config] Failed to read {}: {}, using defaults", path, e);
                Self::default()
            }
        }
    }

    /// 应用预设主题（对颜色类字段覆写）。
    ///
    /// 支持的预设：`dark` / `dark_plus` / `light` / `light_plus`（仿 VSCode 配色）。
    /// 非颜色字段（窗口布局、字号、行为等）不受预设影响，由 YAML / 默认值决定。
    /// 传入其它值（如空字符串或 `custom`）时不修改任何字段。
    pub fn apply_theme(&mut self, name: &str) {
        type ThemePreset = (
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
            &'static str,
        );

        // (窗口背景, 边框, 缓冲文本, 缓冲背景, 候选栏背景,
        //  候选文本, 选中文本, 选中背景, 序号, 图标, 提示)
        let preset: Option<ThemePreset> = match name {
                "dark" => Some((
                    "#1E1E1E", "#454545", "#D4D4D4", "#252526", "#252526", "#CCCCCC", "#FFFFFF",
                    "#0E639C", "#8A8A8A", "#6E6E6E", "#8A8A8A",
                )),
                "dark_plus" => Some((
                    "#1E1E1E", "#454545", "#D4D4D4", "#252526", "#252526", "#CCCCCC", "#FFFFFF",
                    "#094771", "#8A8A8A", "#569CD6", "#6E6E6E",
                )),
                "light" => Some((
                    "#FFFFFF", "#C8C8C8", "#333333", "#F3F3F3", "#F3F3F3", "#000000", "#FFFFFF",
                    "#0066FF", "#606060", "#6E6E6E", "#808080",
                )),
                "light_plus" => Some((
                    "#FFFFFF", "#C8C8C8", "#333333", "#F3F3F3", "#F3F3F3", "#000000", "#FFFFFF",
                    "#005FB8", "#606060", "#007ACC", "#808080",
                )),
                _ => None,
            };

        let Some((bg, border, buf_text, buf_bg, cand_bg, item_text, sel_text, sel_bg, index, icon, info)) =
            preset
        else {
            return;
        };

        self.window.background_color = bg.to_string();
        self.window.border_color = border.to_string();
        self.buffer_bar.text_color = buf_text.to_string();
        self.buffer_bar.background_color = buf_bg.to_string();
        self.candidate_bar.background_color = cand_bg.to_string();
        self.candidate_item.text_color = item_text.to_string();
        self.candidate_item.selected_text_color = sel_text.to_string();
        self.candidate_item.selected_background_color = sel_bg.to_string();
        self.candidate_item.index_color = index.to_string();
        self.pagination.icon_color = icon.to_string();
        self.pagination.info_color = info.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GuiConfig::default();
        assert_eq!(config.page_size, 10);
        assert!(!config.window.background_color.is_empty());
        assert_eq!(config.candidate_item.selected_background_color, "#0078D4");
        // 新增字段默认值
        assert!(config.window.font_family.contains("Microsoft YaHei"));
        assert!(config.window.font_feature_settings.is_empty());
        assert_eq!(config.pagination.position, "both");
        assert_eq!(config.pagination.prev_icon, "◀");
        assert_eq!(config.pagination.next_icon, "▶");
        assert_eq!(config.behavior.page_keys.previous, vec!["PageUp".to_string()]);
        assert_eq!(config.behavior.page_keys.next, vec!["PageDown".to_string()]);
    }

    #[test]
    fn test_parse_yaml_partial() {
        let yaml = r##"
dict_path: "resources/dicts/base.cin"
page_size: 8
window:
  background_color: "#202020"
  opacity: 0.95
  font_family: "Source Han Serif SC"
  font_feature_settings: "ss01"
candidate_item:
  font_size: 18
pagination:
  position: "left"
  prev_icon: "←"
behavior:
  page_keys:
    previous: ["Left"]
    next: ["Right"]
"##;
        let config = GuiConfig::from_yaml(yaml);
        assert_eq!(config.page_size, 8);
        assert_eq!(config.window.background_color, "#202020");
        assert_eq!(config.window.opacity, 0.95);
        assert_eq!(config.window.font_family, "Source Han Serif SC");
        assert_eq!(config.window.font_feature_settings, "ss01");
        // 未提供的字段使用默认值
        assert_eq!(config.candidate_item.text_color, "#000000");
        assert_eq!(config.candidate_item.font_size, 18);
        assert_eq!(config.pagination.prev_icon, "←");
        assert_eq!(config.pagination.position, "left");
        assert_eq!(config.behavior.page_keys.previous, vec!["Left".to_string()]);
        assert_eq!(config.behavior.page_keys.next, vec!["Right".to_string()]);
        // 翻页区可见性默认保持
        assert!(config.pagination.visible);
    }

    #[test]
    fn test_parse_invalid_yaml_falls_back_to_default() {
        let config = GuiConfig::from_yaml("::: not valid :::");
        let default = GuiConfig::default();
        assert_eq!(config.page_size, default.page_size);
    }

    #[test]
    fn test_from_file_nonexistent_falls_back() {
        let config = GuiConfig::from_file("this/file/does/not/exist.yaml");
        assert_eq!(config.window.font_size, 14);
    }

    #[test]
    fn test_theme_dark_preset() {
        let config = GuiConfig::from_yaml("theme: dark\n");
        assert_eq!(config.window.background_color, "#1E1E1E");
        assert_eq!(config.buffer_bar.background_color, "#252526");
        assert_eq!(config.candidate_bar.background_color, "#252526");
        assert_eq!(config.candidate_item.selected_background_color, "#0E639C");
    }

    #[test]
    fn test_theme_dark_plus_preset() {
        let config = GuiConfig::from_yaml("theme: dark_plus\n");
        assert_eq!(config.window.background_color, "#1E1E1E");
        assert_eq!(config.candidate_item.selected_background_color, "#094771");
    }

    #[test]
    fn test_theme_light_preset() {
        let config = GuiConfig::from_yaml("theme: light\n");
        assert_eq!(config.window.background_color, "#FFFFFF");
        assert_eq!(config.candidate_item.text_color, "#000000");
        assert_eq!(config.candidate_item.selected_background_color, "#0066FF");
    }

    #[test]
    fn test_theme_unknown_keeps_default() {
        let config = GuiConfig::from_yaml("theme: custom\n");
        assert_eq!(config.window.background_color, "#FFFFFF");
        assert_eq!(config.candidate_item.selected_background_color, "#0078D4");
    }

    #[test]
    fn test_theme_only_overrides_colors() {
        // 温度：theme 不应修改非颜色字段（如字号、布局、行为）
        let config = GuiConfig::from_yaml("theme: dark\npage_size: 7\ncandidate_item:\n  font_size: 20\n");
        assert_eq!(config.page_size, 7);
        assert_eq!(config.candidate_item.font_size, 20);
        assert_eq!(config.window.background_color, "#1E1E1E");
    }
}
