//! GUI 配置模块
//!
//! 使用 YAML 提供高度可配置的候选窗口外观与行为：
//! 窗口、缓冲栏、候选栏、候选条目、翻页图标等。

use serde::Deserialize;

/// 应用程序配置
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GuiConfig {
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
        }
    }
}

impl GuiConfig {
    /// 从 YAML 字符串解析配置，失败时回退到默认值
    pub fn from_yaml(yaml: &str) -> Self {
        serde_yaml::from_str(yaml).unwrap_or_default()
    }

    /// 从文件加载配置，失败或文件不存在时回退到默认值
    pub fn from_file(path: &str) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => Self::from_yaml(&content),
            Err(_) => Self::default(),
        }
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
}
