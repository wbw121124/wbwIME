//! 输入上下文模块

use serde::{Deserialize, Serialize};
use wbw_types::{ImeResult, InputContext, InputMode};

/// 上下文管理器
pub struct ContextManager {
    /// 当前上下文
    current: InputContext,
    /// 历史上下文
    history: Vec<InputContext>,
    /// 最大历史记录数
    max_history: usize,
}

impl ContextManager {
    /// 创建新的上下文管理器
    pub fn new(session_id: u64) -> Self {
        let current = InputContext {
            buffer: String::new(),
            cursor: 0,
            mode: InputMode::Pinyin,
            selected: Vec::new(),
            session_id,
        };

        Self {
            current,
            history: Vec::new(),
            max_history: 100,
        }
    }

    /// 获取当前上下文
    pub fn current(&self) -> &InputContext {
        &self.current
    }

    /// 获取可变当前上下文
    pub fn current_mut(&mut self) -> &mut InputContext {
        &mut self.current
    }

    /// 添加字符到缓冲区
    pub fn push_char(&mut self, ch: char) {
        self.save_history();
        self.current.buffer.insert(self.current.cursor, ch);
        self.current.cursor += ch.len_utf8();
    }

    /// 删除缓冲区末尾字符
    pub fn pop_char(&mut self) -> Option<char> {
        if self.current.cursor > 0 {
            self.save_history();
            // 找到前一个字符的边界
            let prev_boundary = self.current.buffer[..self.current.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);

            let _removed = self.current.buffer[prev_boundary..self.current.cursor].to_string();
            self.current.buffer.truncate(prev_boundary);
            self.current.cursor = prev_boundary;

            // 返回被删除的字符（简化处理）
            Some('_')
        } else {
            None
        }
    }

    /// 清空缓冲区
    pub fn clear_buffer(&mut self) {
        self.save_history();
        self.current.buffer.clear();
        self.current.cursor = 0;
    }

    /// 设置输入模式
    pub fn set_mode(&mut self, mode: InputMode) {
        self.current.mode = mode;
    }

    /// 获取输入模式
    pub fn mode(&self) -> InputMode {
        self.current.mode
    }

    /// 添加选中的候选词
    pub fn add_selected(&mut self, word: String) {
        self.current.selected.push(word);
    }

    /// 获取已选中的词
    pub fn selected(&self) -> &[String] {
        &self.current.selected
    }

    /// 清空已选中的词
    pub fn clear_selected(&mut self) {
        self.current.selected.clear();
    }

    /// 获取缓冲区内容
    pub fn buffer(&self) -> &str {
        &self.current.buffer
    }

    /// 获取缓冲区长度
    pub fn buffer_len(&self) -> usize {
        self.current.buffer.len()
    }

    /// 检查缓冲区是否为空
    pub fn is_buffer_empty(&self) -> bool {
        self.current.buffer.is_empty()
    }

    /// 获取光标位置
    pub fn cursor(&self) -> usize {
        self.current.cursor
    }

    /// 保存历史记录
    fn save_history(&mut self) {
        self.history.push(self.current.clone());
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// 撤销操作
    pub fn undo(&mut self) -> bool {
        if let Some(prev) = self.history.pop() {
            self.current = prev;
            true
        } else {
            false
        }
    }

    /// 获取历史记录
    pub fn history(&self) -> &[InputContext] {
        &self.history
    }

    /// 清空历史记录
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// 上下文事件
#[derive(Debug, Clone)]
pub enum ContextEvent {
    /// 字符输入
    CharInput(char),
    /// 字符删除
    CharDelete,
    /// 缓冲区清空
    BufferClear,
    /// 模式切换
    ModeChange(InputMode),
    /// 候选词选择
    CandidateSelect(String),
    /// 确认输入
    Confirm,
    /// 取消输入
    Cancel,
    /// 撤销操作
    Undo,
}

/// 上下文事件处理函数类型
type ContextEventHandlerFn = Box<dyn FnMut(&mut ContextManager, ContextEvent) -> ImeResult<()>>;

/// 上下文事件处理器
pub struct ContextEventHandler {
    /// 处理器函数
    handler: ContextEventHandlerFn,
}

impl ContextEventHandler {
    /// 创建新的事件处理器
    pub fn new<F>(handler: F) -> Self
    where
        F: FnMut(&mut ContextManager, ContextEvent) -> ImeResult<()> + 'static,
    {
        Self {
            handler: Box::new(handler),
        }
    }

    /// 处理事件
    pub fn handle(&mut self, context: &mut ContextManager, event: ContextEvent) -> ImeResult<()> {
        (self.handler)(context, event)
    }
}

/// 上下文快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    /// 缓冲区
    pub buffer: String,
    /// 光标位置
    pub cursor: usize,
    /// 输入模式
    pub mode: InputMode,
    /// 已选词
    pub selected: Vec<String>,
    /// 时间戳
    pub timestamp: u64,
}

impl From<&InputContext> for ContextSnapshot {
    fn from(ctx: &InputContext) -> Self {
        Self {
            buffer: ctx.buffer.clone(),
            cursor: ctx.cursor,
            mode: ctx.mode,
            selected: ctx.selected.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

impl ContextSnapshot {
    /// 恢复到输入上下文
    pub fn to_input_context(&self, session_id: u64) -> InputContext {
        InputContext {
            buffer: self.buffer.clone(),
            cursor: self.cursor,
            mode: self.mode,
            selected: self.selected.clone(),
            session_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_push_char() {
        let mut ctx = ContextManager::new(1);
        ctx.push_char('a');
        ctx.push_char('b');
        assert_eq!(ctx.buffer(), "ab");
    }

    #[test]
    fn test_context_pop_char() {
        let mut ctx = ContextManager::new(1);
        ctx.push_char('a');
        ctx.push_char('b');
        ctx.pop_char();
        assert_eq!(ctx.buffer(), "a");
    }

    #[test]
    fn test_context_clear_buffer() {
        let mut ctx = ContextManager::new(1);
        ctx.push_char('a');
        ctx.clear_buffer();
        assert!(ctx.is_buffer_empty());
    }

    #[test]
    fn test_context_undo() {
        let mut ctx = ContextManager::new(1);
        ctx.push_char('a');
        ctx.push_char('b');
        assert!(ctx.undo());
        assert_eq!(ctx.buffer(), "a");
    }

    #[test]
    fn test_context_snapshot() {
        let mut ctx = ContextManager::new(1);
        ctx.push_char('a');
        let snapshot = ContextSnapshot::from(ctx.current());
        assert_eq!(snapshot.buffer, "a");
        let restored = snapshot.to_input_context(1);
        assert_eq!(restored.buffer, "a");
    }
}
