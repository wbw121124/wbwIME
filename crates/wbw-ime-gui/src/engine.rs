//! GUI 引擎模块（纯 Rust，不依赖 Qt）
//!
//! 封装 `ImeHost` + `Matcher` + `Ranker`，将拼音输入流程接入
//! imekit 的候选窗口状态机，并处理 imekit 未映射的按键
//!（字母、空格、数字键选候选等）。

use std::path::Path;

use wbw_dict::{DictBuilder, FstDict};
use wbw_imekit::candidate_window::{CandidateWindow, WindowPosition, WindowStyle};
use wbw_imekit::ime_host::{ImeHost, ImeResponseType};
use wbw_imekit::key_mapper::KeyEvent;
use wbw_imekit::{ImeConfig, ImeResponse};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::Ranker;
use wbw_types::{InputContext, InputMode, RankConfig};

use crate::config::GuiConfig;
use crate::hook::CHINESE_MODE;

/// 按键处理后的 UI 状态，供 Qt 前端渲染
#[derive(Debug, Clone, Default)]
pub struct GuiState {
    /// 当前缓冲区（拼写）
    pub buffer: String,
    /// 当前页候选词
    pub candidates: Vec<String>,
    /// 当前选中索引
    pub selected_index: usize,
    /// 当前页码（0 起）
    pub page: usize,
    /// 总页数
    pub total_pages: usize,
    /// 是否显示窗口
    pub visible: bool,
    /// 最近确认上屏的文本
    pub committed: Option<String>,
    /// 当前输入模式（"中"/"英"）
    pub mode: String,
}

/// wbwIME GUI 引擎
pub struct WbwIme {
    host: ImeHost,
    matcher: Matcher,
    ranker: Ranker,
    config: GuiConfig,
    /// 最近一次确认上屏的文本（`snapshot` 读取后清空，供调用方上屏）。
    pending_commit: Option<String>,
}

impl WbwIme {
    /// 创建引擎并加载词典
    pub fn new(config: GuiConfig, page_size: usize) -> Self {
        let mut config = config;
        // 每页候选词数量限制在 1-10
        config.page_size = page_size.clamp(1, 10);

        // 加载词典（解析兜底路径）
        let dict = load_dict(&resolve_dict_path(&config.dict_path)).unwrap_or_default_fst();

        let matcher_config = MatcherConfig {
            fuzzy_enabled: config.behavior.fuzzy_enabled,
            max_candidates: config.page_size * 4,
            ..MatcherConfig::default()
        };
        let matcher = Matcher::with_dict(matcher_config, dict);

        let rank_config = RankConfig::default();
        let ranker = Ranker::new(rank_config);

        let ime_config = ImeConfig {
            fuzzy_enabled: config.behavior.fuzzy_enabled,
            l0_enabled: config.behavior.l0_enabled,
            max_candidates: config.page_size,
            input_mode: InputMode::Pinyin,
            window_style: build_ime_window_style(&config),
            ..ImeConfig::default()
        };
        let mut host = ImeHost::new(ime_config);

        // 注册默认候选窗口并设为活动窗口
        let window = CandidateWindow::new(
            WindowPosition::new(0, 0, 300, 200),
            build_ime_window_style(&config),
        );
        let idx = host.window_manager_mut().add_window(window);
        host.window_manager_mut().set_active_window(idx);

        Self {
            host,
            matcher,
            ranker,
            config,
            pending_commit: None,
        }
    }

    /// 处理一次按键，返回更新后的 UI 状态
    pub fn process_key(&mut self, code: u32, ch: Option<char>) -> GuiState {
        let ch = ch.filter(|c| c.is_ascii_alphanumeric());

        // 1) 数字键：输入中时选当前页候选（第 1-9 候选）
        if self.config.behavior.digit_selects && self.host.is_inputting() {
            if let Some(c) = ch {
                if let Some(digit) = c.to_digit(10) {
                    if (1..=9).contains(&digit) {
                        return self.select_index(digit as usize - 1);
                    }
                }
            }
        }

        // 2) 空格：配置为确认时上屏
        if code == 32 && self.config.behavior.space_confirms && self.host.is_inputting() {
            return self.confirm_pair();
        }

        // 3) 字母输入：imekit 的 KeyMapper 默认未映射字母，需直接 input_char
        if let Some(c) = ch {
            if c.is_ascii_alphabetic() {
                let response = match self.host.input_char(c) {
                    Ok(r) => r,
                    Err(_) => return self.snapshot(),
                };
                return self.after_response(response);
            }
            // digit_selects 关闭时，数字作为普通字符输入
            if c.is_ascii_digit() {
                let response = match self.host.input_char(c) {
                    Ok(r) => r,
                    Err(_) => return self.snapshot(),
                };
                return self.after_response(response);
            }
        }

        // 4) 其余功能键（Enter/Backspace/Esc/方向/翻页）交给 imekit 状态机
        let key = KeyEvent::new(code, ch);
        let response = match self.host.process_key(key) {
            Ok(r) => r,
            Err(_) => return self.snapshot(),
        };
        self.after_response(response)
    }

    /// 通过 imekit 选择第 index 个候选（跨页纠正：imekit 内部以页内索引为准）
    fn select_index(&mut self, index: usize) -> GuiState {
        if !self.host.is_inputting() {
            return self.snapshot();
        }
        if let Some(window) = self.host.window_manager().active_window() {
            if index >= window.current_page_candidates().len() {
                return self.snapshot();
            }
        }
        let response = match self.host.select_candidate(index) {
            Ok(r) => r,
            Err(_) => return self.snapshot(),
        };
        self.after_response(response)
    }

    /// 确认：优先选中候选，否则提交缓冲
    fn confirm_pair(&mut self) -> GuiState {
        if !self.host.is_inputting() {
            return self.snapshot();
        }
        let response = match self.host.confirm() {
            Ok(r) => r,
            Err(_) => return self.snapshot(),
        };
        self.after_response(response)
    }

    /// 处理 imekit 返回的响应，注入候选词并生成 UI 状态
    fn after_response(&mut self, response: ImeResponse) -> GuiState {
        match response.response_type {
            ImeResponseType::InputChar | ImeResponseType::DeleteChar => {
                self.refresh_candidates();
            }
            ImeResponseType::Confirm => {
                // 记录确认上屏的文本（供 hook 模式剪贴板上屏）
                if let Some(text) = response.text.as_ref() {
                    if !text.is_empty() {
                        self.pending_commit = Some(text.clone());
                    }
                }
                // 不论由 confirm 还是 select_candidate 触发，确认后清空缓冲并复位状态
                self.host.reset();
                self.host.window_manager_mut().active_window_mut().map(|w| w.hide());
            }
            ImeResponseType::Cancel => {
                self.host.window_manager_mut().active_window_mut().map(|w| w.hide());
            }
            ImeResponseType::ShowCandidates => {}
            _ => {}
        }
        self.snapshot()
    }

    /// 根据当前缓冲重新匹配并更新候选窗口
    fn refresh_candidates(&mut self) {
        let buffer = self.host.buffer().to_string();
        let window = self.host.window_manager_mut().active_window_mut();
        let Some(window) = window else {
            return;
        };

        if buffer.is_empty() {
            let _ = window.hide();
            window.set_candidates(Vec::new());
            return;
        }

        let ctx = InputContext {
            buffer: buffer.clone(),
            cursor: buffer.len(),
            mode: InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let matched = self.matcher.match_input(&ctx);
        let ranked = self.ranker.rank(&matched);
        window.set_candidates(ranked);
        let _ = window.show();
    }

    /// 生成当前 UI 状态快照
    fn snapshot(&mut self) -> GuiState {
        let window = self.host.window_manager().active_window();
        let selected_index = window.map(|w| w.selected_index()).unwrap_or(0);
        let page = window.map(|w| w.page()).unwrap_or(0);
        let total_pages = window.map(|w| w.total_pages()).unwrap_or(0);
        let candidates: Vec<String> = window
            .map(|w| w.current_page_candidates().iter().map(|c| c.text.clone()).collect())
            .unwrap_or_default();
        let visible = window.map(|w| w.is_visible() && !self.host.buffer().is_empty()).unwrap_or(false);

        GuiState {
            buffer: self.host.buffer().to_string(),
            candidates,
            selected_index,
            page,
            total_pages,
            visible,
            committed: self.pending_commit.take(),
            mode: if CHINESE_MODE.load(std::sync::atomic::Ordering::Acquire) {
                "中".into()
            } else {
                "英".into()
            },
        }
    }

    /// 获取页大小
    pub fn page_size(&self) -> usize {
        self.config.page_size
    }

    /// 检查是否在输入中
    pub fn is_inputting(&self) -> bool {
        self.host.is_inputting()
    }

    /// 重置引擎状态
    pub fn reset(&mut self) {
        self.host.reset();
        self.host.window_manager_mut().active_window_mut().map(|w| w.hide());
    }
}

/// 从 .cin / .fst 加载词典
fn load_dict(path: &str) -> Result<FstDict, String> {
    let path = Path::new(path);
    if !path.exists() {
        return Err(format!("词典不存在: {}", path.display()));
    }
    match path.extension().and_then(|e| e.to_str()) {
        Some("fst") => FstDict::from_file(path).map_err(|e| e.to_string()),
        _ => {
            let mut builder = DictBuilder::new();
            builder.load_cin(path).map_err(|e| e.to_string())?;
            builder.deduplicate();
            builder.sort();
            Ok(builder.build_fst())
        }
    }
}

/// 解析词典实际路径：优先配置相对路径；相对路径不存在时回退到
/// 可执行文件附近的 `dicts/base.cin`（安装布局）或
/// `../resources/dicts/base.cin`（仓库 target/release 布局）。
fn resolve_dict_path(configured: &str) -> String {
    if Path::new(configured).exists() {
        return configured.to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidates = [
                dir.join("dicts").join("base.cin"),
                dir.parent().unwrap_or(dir).join("resources").join("dicts").join("base.cin"),
            ];
            for c in &candidates {
                if c.exists() {
                    return c.to_string_lossy().into_owned();
                }
            }
        }
    }
    configured.to_string()
}

/// 将 GUI 配置翻译为 imekit 的 WindowStyle（供 confirm/select 协同使用）
fn build_ime_window_style(config: &GuiConfig) -> WindowStyle {
    WindowStyle {
        background_color: config.window.background_color.clone(),
        text_color: config.candidate_item.text_color.clone(),
        selected_background_color: config.candidate_item.selected_background_color.clone(),
        selected_text_color: config.candidate_item.selected_text_color.clone(),
        border_color: config.window.border_color.clone(),
        border_width: config.window.border_width,
        font_size: config.window.font_size,
        font_name: config.window.font_family.clone(),
        border_radius: config.window.border_radius,
        padding: config.window.padding,
        opacity: config.window.opacity,
    }
}

/// 为 `FstDict` 提供一个空默认值的便捷方法
trait DefaultFst {
    fn unwrap_or_default_fst(self) -> FstDict;
}

impl DefaultFst for Result<FstDict, String> {
    fn unwrap_or_default_fst(self) -> FstDict {
        self.unwrap_or_else(|_| {
            let builder = wbw_dict::FstDictBuilder::new();
            builder.build(wbw_dict::entry::DictSource::Base)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> GuiConfig {
        // 解析到 workspace 根的词典路径（crates/wbw-ime-gui -> ../../resources/dicts/base.cin）
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let dict_path = std::path::Path::new(manifest_dir)
            .join("..")
            .join("..")
            .join("resources")
            .join("dicts")
            .join("base.cin");
        GuiConfig {
            dict_path: dict_path.to_string_lossy().to_string(),
            ..GuiConfig::default()
        }
    }

    fn test_ime() -> WbwIme {
        WbwIme::new(test_config(), 10)
    }

    #[test]
    fn test_input_letter_generates_candidates() {
        let mut ime = test_ime();
        // 字母按键（ASCII），例如 'w'、'o'
        let state = ime.process_key('w' as u32, Some('w'));
        assert_eq!(state.buffer, "w");
        let state = ime.process_key('o' as u32, Some('o'));
        assert_eq!(state.buffer, "wo");
        // base.cin 含 "wo->我"，候选应非空
        assert!(!state.candidates.is_empty(), "wo 应有候选词");
        assert!(state.visible);
    }

    #[test]
    fn test_select_candidate_commits() {
        let mut ime = test_ime();
        ime.process_key('w' as u32, Some('w'));
        let state = ime.process_key('o' as u32, Some('o'));
        assert!(!state.candidates.is_empty());

        // 选第 1 个候选（数字键 1）
        let state = ime.process_key('1' as u32, Some('1'));
        assert!(!state.visible, "确认后窗口应隐藏");
        // 引擎已复位缓冲
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn test_backspace_deletes() {
        let mut ime = test_ime();
        ime.process_key('w' as u32, Some('w'));
        ime.process_key('o' as u32, Some('o'));
        // imekit 的 Backspace 键码为 8，无字符
        let state = ime.process_key(8, None);
        assert_eq!(state.buffer, "w");
    }

    #[test]
    fn test_enter_confirms_buffer() {
        let mut ime = test_ime();
        ime.process_key('w' as u32, Some('w'));
        // imekit 的 Enter 键码为 13
        let state = ime.process_key(13, None);
        assert!(state.buffer.is_empty());
    }

    #[test]
    fn test_esc_cancels() {
        let mut ime = test_ime();
        ime.process_key('w' as u32, Some('w'));
        let state = ime.process_key(27, None);
        assert!(state.buffer.is_empty());
        assert!(!state.visible);
    }

    #[test]
    fn test_non_ascii_ignored() {
        let mut ime = test_ime();
        // 中文字符不作为输入字符
        let state = ime.process_key('中' as u32, Some('中'));
        assert!(state.buffer.is_empty());
    }
}
