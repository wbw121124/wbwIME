//! 候选窗口模块

use thiserror::Error;
use wbw_types::{Candidate, ImeResult};

/// 候选窗口错误类型
#[derive(Error, Debug)]
pub enum CandidateWindowError {
    #[error("窗口初始化失败: {0}")]
    InitError(String),
    
    #[error("窗口更新失败: {0}")]
    UpdateError(String),
    
    #[error("窗口显示失败: {0}")]
    ShowError(String),
    
    #[error("窗口隐藏失败: {0}")]
    HideError(String),
}

/// 候选窗口位置
#[derive(Debug, Clone)]
pub struct WindowPosition {
    /// X 坐标
    pub x: i32,
    /// Y 坐标
    pub y: i32,
    /// 宽度
    pub width: u32,
    /// 高度
    pub height: u32,
}

impl WindowPosition {
    /// 创建新的窗口位置
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// 检查点是否在窗口内
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }

    /// 移动窗口
    pub fn move_to(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// 调整大小
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }
}

/// 候选窗口样式
#[derive(Debug, Clone)]
pub struct WindowStyle {
    /// 背景颜色
    pub background_color: String,
    /// 文本颜色
    pub text_color: String,
    /// 选中项背景颜色
    pub selected_background_color: String,
    /// 选中项文本颜色
    pub selected_text_color: String,
    /// 边框颜色
    pub border_color: String,
    /// 边框宽度
    pub border_width: u32,
    /// 字体大小
    pub font_size: u32,
    /// 字体名称
    pub font_name: String,
    /// 圆角半径
    pub border_radius: u32,
    /// 内边距
    pub padding: u32,
    /// 透明度
    pub opacity: f64,
}

impl Default for WindowStyle {
    fn default() -> Self {
        Self {
            background_color: "#FFFFFF".to_string(),
            text_color: "#000000".to_string(),
            selected_background_color: "#0078D4".to_string(),
            selected_text_color: "#FFFFFF".to_string(),
            border_color: "#CCCCCC".to_string(),
            border_width: 1,
            font_size: 14,
            font_name: "Microsoft YaHei".to_string(),
            border_radius: 4,
            padding: 8,
            opacity: 1.0,
        }
    }
}

/// 候选窗口
pub struct CandidateWindow {
    /// 窗口位置
    position: WindowPosition,
    /// 窗口样式
    style: WindowStyle,
    /// 是否可见
    visible: bool,
    /// 当前选中索引
    selected_index: usize,
    /// 候选词列表
    candidates: Vec<Candidate>,
    /// 每页显示数量
    page_size: usize,
    /// 当前页码
    page: usize,
}

impl CandidateWindow {
    /// 创建新的候选窗口
    pub fn new(position: WindowPosition, style: WindowStyle) -> Self {
        Self {
            position,
            style,
            visible: false,
            selected_index: 0,
            candidates: Vec::new(),
            page_size: 10,
            page: 0,
        }
    }

    /// 设置候选词
    pub fn set_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.page = 0;
        self.selected_index = 0;
    }

    /// 更新候选词
    pub fn update_candidates(&mut self, candidates: Vec<Candidate>) {
        self.candidates = candidates;
        self.selected_index = 0;
    }

    /// 显示窗口
    pub fn show(&mut self) -> ImeResult<()> {
        self.visible = true;
        Ok(())
    }

    /// 隐藏窗口
    pub fn hide(&mut self) -> ImeResult<()> {
        self.visible = false;
        Ok(())
    }

    /// 选择下一个
    pub fn select_next(&mut self) -> bool {
        let page_candidates = self.current_page_candidates();
        if self.selected_index + 1 < page_candidates.len() {
            self.selected_index += 1;
            true
        } else if self.next_page() {
            self.selected_index = 0;
            true
        } else {
            false
        }
    }

    /// 选择上一个
    pub fn select_prev(&mut self) -> bool {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            true
        } else if self.prev_page() {
            self.selected_index = self.current_page_candidates().len().saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// 选择指定索引
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.current_page_candidates().len() {
            self.selected_index = index;
            true
        } else {
            false
        }
    }

    /// 获取当前选中的候选词
    pub fn selected_candidate(&self) -> Option<&Candidate> {
        self.current_page_candidates().get(self.selected_index)
    }

    /// 获取当前页候选词
    pub fn current_page_candidates(&self) -> &[Candidate] {
        let start = self.page * self.page_size;
        let end = std::cmp::min(start + self.page_size, self.candidates.len());
        if start < self.candidates.len() {
            &self.candidates[start..end]
        } else {
            &[]
        }
    }

    /// 翻到下一页
    pub fn next_page(&mut self) -> bool {
        let total_pages = (self.candidates.len() + self.page_size - 1) / self.page_size;
        if self.page + 1 < total_pages {
            self.page += 1;
            self.selected_index = 0;
            true
        } else {
            false
        }
    }

    /// 翻到上一页
    pub fn prev_page(&mut self) -> bool {
        if self.page > 0 {
            self.page -= 1;
            self.selected_index = 0;
            true
        } else {
            false
        }
    }

    /// 获取窗口位置
    pub fn position(&self) -> &WindowPosition {
        &self.position
    }

    /// 获取可变窗口位置
    pub fn position_mut(&mut self) -> &mut WindowPosition {
        &mut self.position
    }

    /// 获取窗口样式
    pub fn style(&self) -> &WindowStyle {
        &self.style
    }

    /// 获取可变窗口样式
    pub fn style_mut(&mut self) -> &mut WindowStyle {
        &mut self.style
    }

    /// 检查窗口是否可见
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// 获取当前页码
    pub fn page(&self) -> usize {
        self.page
    }

    /// 获取总页数
    pub fn total_pages(&self) -> usize {
        (self.candidates.len() + self.page_size - 1) / self.page_size
    }

    /// 获取候选词总数
    pub fn total_candidates(&self) -> usize {
        self.candidates.len()
    }

    /// 获取当前选中索引
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// 渲染窗口
    pub fn render(&self) -> ImeResult<()> {
        // 逐项输出当前页候选词 "索引. 词文本"
        for (i, candidate) in self.current_page_candidates().iter().enumerate() {
            println!("{}. {}", i, candidate.text);
        }
        Ok(())
    }

    /// 处理点击事件
    pub fn handle_click(&mut self, x: i32, y: i32) -> Option<usize> {
        const ITEM_WIDTH: i32 = 30;
        // y 超出窗口高度范围则忽略
        if y < 0 || y >= self.position.height as i32 {
            return None;
        }
        let index = if x < 0 { 0 } else { (x / ITEM_WIDTH) as usize };
        if index < self.current_page_candidates().len() {
            self.select(index);
            Some(index)
        } else {
            None
        }
    }
}

/// 候选窗口事件
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// 窗口显示
    Show,
    /// 窗口隐藏
    Hide,
    /// 窗口移动
    Move(i32, i32),
    /// 窗口调整大小
    Resize(u32, u32),
    /// 候选词选择
    Select(usize),
    /// 翻页
    PageChange(usize),
    /// 点击事件
    Click(i32, i32),
    /// 关闭事件
    Close,
}

/// 候选窗口事件处理器
pub struct WindowEventHandler {
    /// 处理器函数
    handler: Box<dyn FnMut(WindowEvent)>,
}

impl WindowEventHandler {
    /// 创建新的事件处理器
    pub fn new<F>(handler: F) -> Self
    where
        F: FnMut(WindowEvent) + 'static,
    {
        Self {
            handler: Box::new(handler),
        }
    }

    /// 处理事件
    pub fn handle(&mut self, event: WindowEvent) {
        (self.handler)(event);
    }
}

/// 候选窗口管理器
pub struct CandidateWindowManager {
    /// 窗口列表
    windows: Vec<CandidateWindow>,
    /// 当前活动窗口
    active_window: Option<usize>,
}

impl CandidateWindowManager {
    /// 创建新的窗口管理器
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            active_window: None,
        }
    }

    /// 添加窗口
    pub fn add_window(&mut self, window: CandidateWindow) -> usize {
        let index = self.windows.len();
        self.windows.push(window);
        index
    }

    /// 移除窗口
    pub fn remove_window(&mut self, index: usize) -> bool {
        if index < self.windows.len() {
            self.windows.remove(index);
            if self.active_window == Some(index) {
                self.active_window = None;
            }
            true
        } else {
            false
        }
    }

    /// 获取窗口
    pub fn get_window(&self, index: usize) -> Option<&CandidateWindow> {
        self.windows.get(index)
    }

    /// 获取可变窗口
    pub fn get_window_mut(&mut self, index: usize) -> Option<&mut CandidateWindow> {
        self.windows.get_mut(index)
    }

    /// 设置活动窗口
    pub fn set_active_window(&mut self, index: usize) -> bool {
        if index < self.windows.len() {
            self.active_window = Some(index);
            true
        } else {
            false
        }
    }

    /// 获取活动窗口
    pub fn active_window(&self) -> Option<&CandidateWindow> {
        self.active_window.and_then(|i| self.windows.get(i))
    }

    /// 获取活动窗口
    pub fn active_window_mut(&mut self) -> Option<&mut CandidateWindow> {
        self.active_window.and_then(|i| self.windows.get_mut(i))
    }

    /// 获取窗口数量
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// 清空所有窗口
    pub fn clear(&mut self) {
        self.windows.clear();
        self.active_window = None;
    }
}

impl Default for CandidateWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(text: &str) -> Candidate {
        Candidate {
            text: text.to_string(),
            code: String::new(),
            score: 0.0,
            source: wbw_types::CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        }
    }

    fn make_window() -> CandidateWindow {
        CandidateWindow::new(
            WindowPosition::new(0, 0, 300, 200),
            WindowStyle::default(),
        )
    }

    #[test]
    fn test_show_hide() {
        let mut window = make_window();
        assert!(!window.is_visible());
        window.show().unwrap();
        assert!(window.is_visible());
        window.hide().unwrap();
        assert!(!window.is_visible());
    }

    #[test]
    fn test_pagination_and_select_next() {
        let mut window = make_window();
        let candidates: Vec<Candidate> = (0..25).map(|i| make_candidate(&format!("词{}", i))).collect();
        window.set_candidates(candidates);
        assert_eq!(window.page(), 0);
        assert_eq!(window.current_page_candidates().len(), 10);

        // select_next 在本页内移动
        assert!(window.select_next());
        assert_eq!(window.selected_index(), 1);

        // 翻到下一页
        assert!(window.next_page());
        assert_eq!(window.page(), 1);
        assert_eq!(window.selected_index(), 0);
        // 再翻到上一页
        assert!(window.prev_page());
        assert_eq!(window.page(), 0);
    }

    #[test]
    fn test_handle_click_select() {
        let mut window = make_window();
        let candidates: Vec<Candidate> = (0..10).map(|i| make_candidate(&format!("词{}", i))).collect();
        window.set_candidates(candidates);
        // 第 2 个候选（索引 1）位置在 [30, 60)
        let selected = window.handle_click(45, 10);
        assert_eq!(selected, Some(1));
        assert_eq!(window.selected_index(), 1);
    }
}