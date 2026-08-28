//! IME 宿主模块

use thiserror::Error;
use wbw_types::{ImeResult, InputMode, Candidate};
use crate::candidate_window::CandidateWindowManager;
use crate::key_mapper::{KeyMapper, KeyEvent, KeyAction};

/// IME 宿主错误类型
#[derive(Error, Debug)]
pub enum ImeHostError {
    #[error("初始化失败: {0}")]
    InitError(String),
    
    #[error("输入处理失败: {0}")]
    InputError(String),
    
    #[error("候选词处理失败: {0}")]
    CandidateError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("状态错误: {0}")]
    StateError(String),
}

/// IME 状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeState {
    /// 空闲状态
    Idle,
    /// 输入中
    Inputting,
    /// 候选词选择中
    Selecting,
    /// 确认中
    Confirming,
    /// 错误状态
    Error,
}

/// IME 配置
#[derive(Debug, Clone)]
pub struct ImeConfig {
    /// 输入模式
    pub input_mode: InputMode,
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 最大候选词数量
    pub max_candidates: usize,
    /// 自动确认阈值
    pub auto_confirm_threshold: f64,
    /// 是否启用 L0 学习
    pub l0_enabled: bool,
    /// 候选窗口样式
    pub window_style: crate::candidate_window::WindowStyle,
}

impl Default for ImeConfig {
    fn default() -> Self {
        Self {
            input_mode: InputMode::Pinyin,
            fuzzy_enabled: true,
            max_candidates: 10,
            auto_confirm_threshold: 0.8,
            l0_enabled: true,
            window_style: crate::candidate_window::WindowStyle::default(),
        }
    }
}

/// IME 宿主
pub struct ImeHost {
    /// 配置
    config: ImeConfig,
    /// 当前状态
    state: ImeState,
    /// 输入缓冲区
    buffer: String,
    /// 光标位置
    cursor: usize,
    /// 当前输入模式
    mode: InputMode,
    /// 候选词窗口管理器
    window_manager: CandidateWindowManager,
    /// 按键映射器
    key_mapper: KeyMapper,
    /// 已确认的文本
    confirmed_text: String,
    /// 会话 ID
    session_id: u64,
}

impl ImeHost {
    /// 创建新的 IME 宿主
    pub fn new(config: ImeConfig) -> Self {
        let window_manager = CandidateWindowManager::new();
        let key_mapper = KeyMapper::new();
        
        Self {
            config,
            state: ImeState::Idle,
            buffer: String::new(),
            cursor: 0,
            mode: InputMode::Pinyin,
            window_manager,
            key_mapper,
            confirmed_text: String::new(),
            session_id: 1,
        }
    }

    /// 初始化 IME
    pub fn initialize(&mut self) -> ImeResult<()> {
        self.state = ImeState::Idle;
        self.buffer.clear();
        self.cursor = 0;
        Ok(())
    }

    /// 处理按键事件
    pub fn process_key(&mut self, key: KeyEvent) -> ImeResult<ImeResponse> {
        // 获取按键动作，若没有匹配则返回空响应
        let Some(action) = self.key_mapper.process_key(&key) else {
            return Ok(ImeResponse::empty());
        };

        match action {
            KeyAction::InputChar(ch) => self.input_char(ch),
            KeyAction::DeleteChar => self.delete_char(),
            KeyAction::Confirm => self.confirm(),
            KeyAction::Cancel => self.cancel(),
            KeyAction::PageUp => self.page_up(),
            KeyAction::PageDown => self.page_down(),
            KeyAction::SelectUp => self.select_up(),
            KeyAction::SelectDown => self.select_down(),
            KeyAction::SwitchMode => Ok(ImeResponse {
                response_type: ImeResponseType::SwitchMode,
                buffer: self.buffer.clone(),
                cursor: self.cursor,
                ..ImeResponse::empty()
            }),
            KeyAction::TriggerFuzzy | KeyAction::Other(_) => Ok(ImeResponse {
                response_type: ImeResponseType::None,
                buffer: self.buffer.clone(),
                cursor: self.cursor,
                ..ImeResponse::empty()
            }),
        }
    }

    /// 输入字符
    pub fn input_char(&mut self, ch: char) -> ImeResult<ImeResponse> {
        // 仅接受 ASCII 字母或数字
        if !ch.is_ascii_alphanumeric() {
            return Ok(ImeResponse::empty());
        }

        self.buffer.push(ch);
        self.cursor += ch.len_utf8();
        self.state = ImeState::Inputting;

        // 使用会话 ID 参与候选逻辑（简化处理）
        let _session = self.session_id;

        Ok(ImeResponse {
            response_type: ImeResponseType::InputChar,
            text: Some(ch.to_string()),
            candidates: Vec::new(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 删除字符
    pub fn delete_char(&mut self) -> ImeResult<ImeResponse> {
        if self.cursor == 0 || self.buffer.is_empty() {
            return Ok(ImeResponse::empty());
        }
        // 找到前一个字符边界
        let prev_boundary = self.buffer[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.buffer.truncate(prev_boundary);
        self.cursor = prev_boundary;

        if self.buffer.is_empty() {
            self.state = ImeState::Idle;
        }

        Ok(ImeResponse {
            response_type: ImeResponseType::DeleteChar,
            text: None,
            candidates: Vec::new(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 确认输入
    pub fn confirm(&mut self) -> ImeResult<ImeResponse> {
        // 若窗口有选中候选词，优先使用选中文本；否则使用缓冲区
        let selected_text = self
            .window_manager
            .active_window()
            .and_then(|w| w.selected_candidate().map(|c| c.text.clone()));

        if self.buffer.is_empty() && selected_text.is_none() {
            return Ok(ImeResponse::empty());
        }

        let text = selected_text.unwrap_or_else(|| self.buffer.clone());
        if !text.is_empty() {
            self.confirmed_text.push_str(&text);
        }
        // 清空缓冲区
        self.buffer.clear();
        self.cursor = 0;
        self.state = ImeState::Idle;

        Ok(ImeResponse {
            response_type: ImeResponseType::Confirm,
            text: Some(text),
            candidates: Vec::new(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: false,
            need_hide: true,
        })
    }

    /// 取消输入
    pub fn cancel(&mut self) -> ImeResult<ImeResponse> {
        let was_inputting = !self.buffer.is_empty() || self.state != ImeState::Idle;

        self.buffer.clear();
        self.cursor = 0;
        self.state = ImeState::Idle;

        Ok(ImeResponse {
            response_type: ImeResponseType::Cancel,
            text: None,
            candidates: Vec::new(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: was_inputting,
            need_hide: was_inputting,
        })
    }

    /// 选择候选词
    pub fn select_candidate(&mut self, index: usize) -> ImeResult<ImeResponse> {
        let selected = {
            let Some(window) = self.window_manager.active_window_mut() else {
                return Ok(ImeResponse::empty());
            };
            if !window.select(index) {
                return Ok(ImeResponse::empty());
            }
            window.selected_candidate().map(|c| c.text.clone())
        };

        let Some(text) = selected else {
            return Ok(ImeResponse::empty());
        };

        // 会话 ID 参与生成响应
        let session_id = self.session_id;
        let _ = session_id;

        self.state = ImeState::Confirming;
        self.confirmed_text.push_str(&text);

        Ok(ImeResponse {
            response_type: ImeResponseType::Confirm,
            text: Some(text),
            candidates: Vec::new(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_hide: true,
            need_refresh: false,
        })
    }

    /// 翻页
    pub fn page_up(&mut self) -> ImeResult<ImeResponse> {
        if let Some(window) = self.window_manager.active_window_mut() {
            window.prev_page();
        }
        Ok(ImeResponse {
            response_type: ImeResponseType::ShowCandidates,
            text: None,
            candidates: self
                .window_manager
                .active_window()
                .map(|w| w.current_page_candidates().to_vec())
                .unwrap_or_default(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 翻页
    pub fn page_down(&mut self) -> ImeResult<ImeResponse> {
        if let Some(window) = self.window_manager.active_window_mut() {
            window.next_page();
        }
        Ok(ImeResponse {
            response_type: ImeResponseType::ShowCandidates,
            text: None,
            candidates: self
                .window_manager
                .active_window()
                .map(|w| w.current_page_candidates().to_vec())
                .unwrap_or_default(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 选择上一个候选
    fn select_up(&mut self) -> ImeResult<ImeResponse> {
        if let Some(window) = self.window_manager.active_window_mut() {
            window.select_prev();
        }
        Ok(ImeResponse {
            response_type: ImeResponseType::ShowCandidates,
            text: None,
            candidates: self
                .window_manager
                .active_window()
                .map(|w| w.current_page_candidates().to_vec())
                .unwrap_or_default(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 选择下一个候选
    fn select_down(&mut self) -> ImeResult<ImeResponse> {
        if let Some(window) = self.window_manager.active_window_mut() {
            window.select_next();
        }
        Ok(ImeResponse {
            response_type: ImeResponseType::ShowCandidates,
            text: None,
            candidates: self
                .window_manager
                .active_window()
                .map(|w| w.current_page_candidates().to_vec())
                .unwrap_or_default(),
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            need_refresh: true,
            need_hide: false,
        })
    }

    /// 切换输入模式
    pub fn switch_mode(&mut self, mode: InputMode) -> ImeResult<()> {
        self.mode = mode;
        self.config.input_mode = mode;
        Ok(())
    }

    /// 获取当前状态
    pub fn state(&self) -> &ImeState {
        &self.state
    }

    /// 获取输入缓冲区
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// 获取光标位置
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 获取当前输入模式
    pub fn mode(&self) -> InputMode {
        self.mode
    }

    /// 获取已确认的文本
    pub fn confirmed_text(&self) -> &str {
        &self.confirmed_text
    }

    /// 获取配置
    pub fn config(&self) -> &ImeConfig {
        &self.config
    }

    /// 获取窗口管理器
    pub fn window_manager(&self) -> &CandidateWindowManager {
        &self.window_manager
    }

    /// 获取窗口管理器
    pub fn window_manager_mut(&mut self) -> &mut CandidateWindowManager {
        &mut self.window_manager
    }

    /// 获取按键映射器
    pub fn key_mapper(&self) -> &KeyMapper {
        &self.key_mapper
    }

    /// 获取按键映射器
    pub fn key_mapper_mut(&mut self) -> &mut KeyMapper {
        &mut self.key_mapper
    }

    /// 重置 IME 状态
    pub fn reset(&mut self) {
        self.state = ImeState::Idle;
        self.buffer.clear();
        self.cursor = 0;
        self.confirmed_text.clear();
    }

    /// 检查是否在输入中
    pub fn is_inputting(&self) -> bool {
        self.state == ImeState::Inputting || self.state == ImeState::Selecting
    }
}

/// IME 响应
#[derive(Debug, Clone)]
pub struct ImeResponse {
    /// 响应类型
    pub response_type: ImeResponseType,
    /// 输入的文本（如果有）
    pub text: Option<String>,
    /// 候选词列表（如果有）
    pub candidates: Vec<Candidate>,
    /// 当前缓冲区
    pub buffer: String,
    /// 光标位置
    pub cursor: usize,
    /// 是否需要刷新窗口
    pub need_refresh: bool,
    /// 是否需要隐藏窗口
    pub need_hide: bool,
}

impl ImeResponse {
    /// 创建空响应
    pub fn empty() -> Self {
        Self {
            response_type: ImeResponseType::None,
            text: None,
            candidates: Vec::new(),
            buffer: String::new(),
            cursor: 0,
            need_refresh: false,
            need_hide: false,
        }
    }
}

/// IME 响应类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeResponseType {
    /// 无操作
    None,
    /// 输入字符
    InputChar,
    /// 删除字符
    DeleteChar,
    /// 确认输入
    Confirm,
    /// 取消输入
    Cancel,
    /// 显示候选词
    ShowCandidates,
    /// 隐藏候选词
    HideCandidates,
    /// 切换模式
    SwitchMode,
    /// 错误
    Error,
}

/// IME 事件
#[derive(Debug, Clone)]
pub enum ImeEvent {
    /// 状态变化
    StateChange(ImeState),
    /// 输入缓冲区变化
    BufferChange(String),
    /// 候选词更新
    CandidateUpdate(Vec<Candidate>),
    /// 模式切换
    ModeSwitch(InputMode),
    /// 文本确认
    TextConfirm(String),
    /// 错误发生
    Error(String),
}

/// IME 事件处理器
pub struct ImeEventHandler {
    /// 处理器函数
    handler: Box<dyn FnMut(ImeEvent)>,
}

impl ImeEventHandler {
    /// 创建新的事件处理器
    pub fn new<F>(handler: F) -> Self
    where
        F: FnMut(ImeEvent) + 'static,
    {
        Self {
            handler: Box::new(handler),
        }
    }

    /// 处理事件
    pub fn handle(&mut self, event: ImeEvent) {
        (self.handler)(event);
    }
}

/// IME 适配器特征
pub trait ImeAdapter {
    /// 初始化适配器
    fn initialize(&mut self) -> ImeResult<()>;
    
    /// 处理按键
    fn process_key(&mut self, key: KeyEvent) -> ImeResult<ImeResponse>;
    
    /// 获取状态
    fn state(&self) -> &ImeState;
    
    /// 获取缓冲区
    fn buffer(&self) -> &str;
    
    /// 重置
    fn reset(&mut self);
}

/// IME 工厂
pub struct ImeFactory;

impl ImeFactory {
    /// 创建 IME 宿主
    pub fn create_ime(config: ImeConfig) -> ImeHost {
        ImeHost::new(config)
    }

    /// 创建默认 IME
    pub fn create_default() -> ImeHost {
        ImeHost::new(ImeConfig::default())
    }

    /// 创建拼音 IME
    pub fn create_pinyin() -> ImeHost {
        let config = ImeConfig {
            input_mode: InputMode::Pinyin,
            ..Default::default()
        };
        ImeHost::new(config)
    }

    /// 创建五笔 IME
    pub fn create_wubi() -> ImeHost {
        let config = ImeConfig {
            input_mode: InputMode::Wubi,
            ..Default::default()
        };
        ImeHost::new(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_host() -> ImeHost {
        ImeHost::new(ImeConfig::default())
    }

    #[test]
    fn test_initialize() {
        let mut host = make_host();
        host.initialize().unwrap();
        assert_eq!(host.state(), &ImeState::Idle);
    }

    #[test]
    fn test_input_char() {
        let mut host = make_host();
        let resp = host.input_char('n').unwrap();
        assert_eq!(resp.response_type, ImeResponseType::InputChar);
        assert_eq!(host.buffer(), "n");
        assert_eq!(host.cursor(), 1);
        assert_eq!(host.state(), &ImeState::Inputting);
    }

    #[test]
    fn test_input_char_rejects_non_alnum() {
        let mut host = make_host();
        let resp = host.input_char(' ').unwrap();
        assert_eq!(resp.response_type, ImeResponseType::None);
        assert!(host.buffer().is_empty());
    }

    #[test]
    fn test_input_char_confirm_flow() {
        let mut host = make_host();
        host.input_char('h').unwrap();
        host.input_char('i').unwrap();
        assert_eq!(host.buffer(), "hi");

        let resp = host.confirm().unwrap();
        assert_eq!(resp.response_type, ImeResponseType::Confirm);
        assert_eq!(resp.text.as_deref(), Some("hi"));
        assert!(resp.need_hide);
        assert!(host.buffer().is_empty());
        assert_eq!(host.state(), &ImeState::Idle);
        assert_eq!(host.confirmed_text(), "hi");
    }

    #[test]
    fn test_delete_char() {
        let mut host = make_host();
        host.input_char('h').unwrap();
        host.input_char('i').unwrap();
        let resp = host.delete_char().unwrap();
        assert_eq!(resp.response_type, ImeResponseType::DeleteChar);
        assert_eq!(host.buffer(), "h");
    }

    #[test]
    fn test_cancel() {
        let mut host = make_host();
        host.input_char('h').unwrap();
        let resp = host.cancel().unwrap();
        assert_eq!(resp.response_type, ImeResponseType::Cancel);
        assert!(host.buffer().is_empty());
        assert_eq!(host.state(), &ImeState::Idle);
    }

    #[test]
    fn test_process_key_enter_confirms() {
        let mut host = make_host();
        host.input_char('a').unwrap();
        let enter = KeyEvent::new(13, None);
        let resp = host.process_key(enter).unwrap();
        assert_eq!(resp.response_type, ImeResponseType::Confirm);
    }

    #[test]
    fn test_select_candidate() {
        let mut host = make_host();
        let window = crate::candidate_window::CandidateWindow::new(
            crate::candidate_window::WindowPosition::new(0, 0, 300, 200),
            crate::candidate_window::WindowStyle::default(),
        );
        let idx = host.window_manager_mut().add_window(window);
        host.window_manager_mut().set_active_window(idx);
        host.window_manager_mut().get_window_mut(idx).unwrap().set_candidates(vec![
            Candidate {
                text: "你好".to_string(),
                code: "nihao".to_string(),
                score: 1.0,
                source: wbw_types::CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
        ]);
        let resp = host.select_candidate(0).unwrap();
        assert_eq!(resp.response_type, ImeResponseType::Confirm);
        assert_eq!(resp.text.as_deref(), Some("你好"));
    }
}