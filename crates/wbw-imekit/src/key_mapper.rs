//! 按键映射模块

use std::fmt;
use thiserror::Error;
use wbw_types::{ImeError, ImeResult};

/// 按键映射错误类型
#[derive(Error, Debug)]
pub enum KeyMapperError {
    #[error("按键映射无效: {0}")]
    InvalidMapping(String),
    
    #[error("按键配置错误: {0}")]
    ConfigError(String),
    
    #[error("按键处理失败: {0}")]
    ProcessingError(String),
}

/// 按键类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyType {
    /// 普通字符
    Char,
    /// 功能键
    Function,
    /// 控制键
    Control,
    /// 修饰键
    Modifier,
    /// 方向键
    Direction,
    /// 特殊键
    Special,
}

/// 按键事件
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// 按键码
    pub code: u32,
    /// 按键字符
    pub char: Option<char>,
    /// 按键类型
    pub key_type: KeyType,
    /// 是否按下 Shift
    pub shift: bool,
    /// 是否按下 Ctrl
    pub ctrl: bool,
    /// 是否按下 Alt
    pub alt: bool,
    /// 是否按下 Meta/Command
    pub meta: bool,
    /// 按键时间戳
    pub timestamp: u64,
}

impl KeyEvent {
    /// 创建新的按键事件
    pub fn new(code: u32, char: Option<char>) -> Self {
        Self {
            code,
            char,
            key_type: KeyType::Char,
            shift: false,
            ctrl: false,
            alt: false,
            meta: false,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// 设置修饰键
    pub fn with_modifiers(mut self, shift: bool, ctrl: bool, alt: bool, meta: bool) -> Self {
        self.shift = shift;
        self.ctrl = ctrl;
        self.alt = alt;
        self.meta = meta;
        self
    }

    /// 检查是否是组合键
    pub fn is_modifier_only(&self) -> bool {
        self.code == 16 || self.code == 17 || self.code == 18 || self.code == 91
    }

    /// 获取修饰键状态
    pub fn modifiers(&self) -> (bool, bool, bool, bool) {
        (self.shift, self.ctrl, self.alt, self.meta)
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.meta {
            parts.push("Meta".to_string());
        }
        
        if let Some(ch) = self.char {
            parts.push(ch.to_string());
        } else {
            parts.push(format!("Key({})", self.code));
        }
        
        write!(f, "{}", parts.join("+"))
    }
}

/// 按键映射
#[derive(Debug, Clone)]
pub struct KeyMapping {
    /// 源按键
    pub from: KeyEvent,
    /// 目标操作
    pub to: KeyAction,
    /// 是否启用
    pub enabled: bool,
    /// 描述
    pub description: String,
}

/// 按键操作
#[derive(Debug, Clone)]
pub enum KeyAction {
    /// 输入字符
    InputChar(char),
    /// 删除字符
    DeleteChar,
    /// 确认输入
    Confirm,
    /// 取消输入
    Cancel,
    /// 翻页
    PageUp,
    /// 翻页
    PageDown,
    /// 上下选择
    SelectUp,
    /// 上下选择
    SelectDown,
    /// 切换输入模式
    SwitchMode,
    /// 触发模糊匹配
    TriggerFuzzy,
    /// 其他操作
    Other(String),
}

/// 按键映射器
pub struct KeyMapper {
    /// 映射表
    mappings: Vec<KeyMapping>,
    /// 默认映射
    default_mappings: Vec<KeyMapping>,
}

impl KeyMapper {
    /// 创建新的按键映射器
    pub fn new() -> Self {
        Self {
            mappings: Vec::new(),
            default_mappings: Self::default_mappings(),
        }
    }

    /// 获取默认映射
    fn default_mappings() -> Vec<KeyMapping> {
        // TODO: 实现默认映射
        todo!("实现默认按键映射")
    }

    /// 添加映射
    pub fn add_mapping(&mut self, mapping: KeyMapping) {
        self.mappings.push(mapping);
    }

    /// 移除映射
    pub fn remove_mapping(&mut self, from: &KeyEvent) -> bool {
        let len_before = self.mappings.len();
        self.mappings.retain(|m| m.from.code != from.code);
        self.mappings.len() < len_before
    }

    /// 查找映射
    pub fn find_mapping(&self, key: &KeyEvent) -> Option<&KeyMapping> {
        // 先查找自定义映射
        if let Some(mapping) = self.mappings.iter().find(|m| m.from.code == key.code && m.enabled) {
            return Some(mapping);
        }
        
        // 再查找默认映射
        self.default_mappings.iter().find(|m| m.from.code == key.code && m.enabled)
    }

    /// 处理按键
    pub fn process_key(&self, key: &KeyEvent) -> Option<KeyAction> {
        self.find_mapping(key).map(|m| m.to.clone())
    }

    /// 获取所有映射
    pub fn mappings(&self) -> &[KeyMapping] {
        &self.mappings
    }

    /// 获取默认映射
    pub fn default_mappings_ref(&self) -> &[KeyMapping] {
        &self.default_mappings
    }

    /// 清空自定义映射
    pub fn clear_mappings(&mut self) {
        self.mappings.clear();
    }

    /// 加载映射配置
    pub fn load_config(&mut self, config: &str) -> ImeResult<()> {
        // TODO: 实现配置加载
        todo!("实现按键映射配置加载")
    }

    /// 保存映射配置
    pub fn save_config(&self) -> ImeResult<String> {
        // TODO: 实现配置保存
        todo!("实现按键映射配置保存")
    }
}

impl Default for KeyMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// 按键预设
pub struct KeyPresets;

impl KeyPresets {
    /// 获取拼音输入法预设
    pub fn pinyin_preset() -> Vec<KeyMapping> {
        // TODO: 实现拼音输入法预设
        todo!("实现拼音输入法按键预设")
    }

    /// 获取五笔输入法预设
    pub fn wubi_preset() -> Vec<KeyMapping> {
        // TODO: 实现五笔输入法预设
        todo!("实现五笔输入法按键预设")
    }

    /// 获取英文输入法预设
    pub fn english_preset() -> Vec<KeyMapping> {
        // TODO: 实现英文输入法预设
        todo!("实现英文输入法按键预设")
    }
}

/// 按键统计
#[derive(Debug, Clone, Default)]
pub struct KeyStats {
    /// 总按键次数
    pub total_keys: usize,
    /// 各按键次数
    pub key_counts: std::collections::HashMap<u32, usize>,
    /// 平均按键间隔（毫秒）
    pub avg_interval_ms: f64,
    /// 最频繁按键
    pub most_frequent_key: Option<u32>,
}

/// 按键统计收集器
pub struct KeyStatsCollector {
    /// 统计信息
    stats: KeyStats,
    /// 上次按键时间
    last_key_time: Option<u64>,
}

impl KeyStatsCollector {
    /// 创建新的统计收集器
    pub fn new() -> Self {
        Self {
            stats: KeyStats::default(),
            last_key_time: None,
        }
    }

    /// 记录按键
    pub fn record_key(&mut self, key: &KeyEvent) {
        self.stats.total_keys += 1;
        *self.stats.key_counts.entry(key.code).or_insert(0) += 1;
        
        // 计算间隔
        if let Some(last_time) = self.last_key_time {
            let interval = key.timestamp - last_time;
            let total_intervals = self.stats.total_keys as f64 - 1.0;
            self.stats.avg_interval_ms = (self.stats.avg_interval_ms * (total_intervals - 1.0) + interval as f64) / total_intervals;
        }
        
        self.last_key_time = Some(key.timestamp);
        
        // 更新最频繁按键
        if let Some((key_code, count)) = self.stats.key_counts.iter().max_by_key(|(_, count)| *count) {
            self.stats.most_frequent_key = Some(*key_code);
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> &KeyStats {
        &self.stats
    }
}

impl Default for KeyStatsCollector {
    fn default() -> Self {
        Self::new()
    }
}