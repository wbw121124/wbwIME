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
        vec![
            KeyMapping {
                from: KeyEvent::new(13, None),
                to: KeyAction::Confirm,
                enabled: true,
                description: "回车确认".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(8, None),
                to: KeyAction::DeleteChar,
                enabled: true,
                description: "退格删除".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(27, None),
                to: KeyAction::Cancel,
                enabled: true,
                description: "Esc 取消".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(38, None),
                to: KeyAction::SelectUp,
                enabled: true,
                description: "上方向键选择上一个".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(40, None),
                to: KeyAction::SelectDown,
                enabled: true,
                description: "下方向键选择下一个".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(33, None),
                to: KeyAction::PageUp,
                enabled: true,
                description: "PageUp 上一页".to_string(),
            },
            KeyMapping {
                from: KeyEvent::new(34, None),
                to: KeyAction::PageDown,
                enabled: true,
                description: "PageDown 下一页".to_string(),
            },
        ]
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
        for line in config.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // 简单 KV 形式：键名=动作，例如 "13=Confirm"
            let Some((key_name, action_name)) = line.split_once('=') else {
                return Err(ImeError::ConfigError(format!("无效的按键配置行: {}", line)));
            };
            let key_name = key_name.trim();
            let action_name = action_name.trim();

            let action = match action_name {
                "Confirm" => KeyAction::Confirm,
                "Cancel" => KeyAction::Cancel,
                "DeleteChar" => KeyAction::DeleteChar,
                "PageUp" => KeyAction::PageUp,
                "PageDown" => KeyAction::PageDown,
                "SelectUp" => KeyAction::SelectUp,
                "SelectDown" => KeyAction::SelectDown,
                "SwitchMode" => KeyAction::SwitchMode,
                "TriggerFuzzy" => KeyAction::TriggerFuzzy,
                other => {
                    // 剩余动作按 InputChar(字符) 处理
                    if other.len() == 1 {
                        KeyAction::InputChar(other.chars().next().unwrap())
                    } else {
                        KeyAction::Other(other.to_string())
                    }
                }
            };

            let code = parse_key_code(key_name)?;
            let mapping = KeyMapping {
                from: KeyEvent::new(code, None),
                to: action,
                enabled: true,
                description: format!("{}={}", key_name, action_name),
            };
            self.mappings.push(mapping);
        }
        Ok(())
    }

    /// 保存映射配置
    pub fn save_config(&self) -> ImeResult<String> {
        let mut lines = Vec::new();
        for mapping in &self.mappings {
            let action_name = match &mapping.to {
                KeyAction::InputChar(ch) => ch.to_string(),
                KeyAction::DeleteChar => "DeleteChar".to_string(),
                KeyAction::Confirm => "Confirm".to_string(),
                KeyAction::Cancel => "Cancel".to_string(),
                KeyAction::PageUp => "PageUp".to_string(),
                KeyAction::PageDown => "PageDown".to_string(),
                KeyAction::SelectUp => "SelectUp".to_string(),
                KeyAction::SelectDown => "SelectDown".to_string(),
                KeyAction::SwitchMode => "SwitchMode".to_string(),
                KeyAction::TriggerFuzzy => "TriggerFuzzy".to_string(),
                KeyAction::Other(other) => other.clone(),
            };
            lines.push(format!("{}={}", mapping.from.code, action_name));
        }
        Ok(lines.join("\n"))
    }
}

impl Default for KeyMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// 解析按键名字为虚拟键码
fn parse_key_code(name: &str) -> ImeResult<u32> {
    // 支持数字键码
    if let Ok(code) = name.parse::<u32>() {
        return Ok(code);
    }
    // 支持字母键（ASCII 码）
    let chars: Vec<char> = name.chars().collect();
    if chars.len() == 1 && chars[0].is_ascii_alphanumeric() {
        return Ok(chars[0] as u32);
    }
    // 支持常见名称
    match name {
        "Enter" | "RETURN" => Ok(13),
        "Backspace" | "BACK" => Ok(8),
        "Esc" | "ESCAPE" => Ok(27),
        "Up" | "UP" => Ok(38),
        "Down" | "DOWN" => Ok(40),
        "PageUp" => Ok(33),
        "PageDown" => Ok(34),
        _ => Err(ImeError::ConfigError(format!("无法识别的按键: {}", name))),
    }
}

/// 按键预设
pub struct KeyPresets;

impl KeyPresets {
    /// 获取拼音输入法预设
    pub fn pinyin_preset() -> Vec<KeyMapping> {
        ('a'..='z').map(|ch| KeyMapping {
            from: KeyEvent::new(ch as u32, Some(ch)),
            to: KeyAction::InputChar(ch),
            enabled: true,
            description: format!("拼音字母 {}", ch),
        }).collect()
    }

    /// 获取五笔输入法预设
    pub fn wubi_preset() -> Vec<KeyMapping> {
        ('a'..='z').map(|ch| KeyMapping {
            from: KeyEvent::new(ch as u32, Some(ch)),
            to: KeyAction::InputChar(ch),
            enabled: true,
            description: format!("五笔字母 {}", ch),
        }).collect()
    }

    /// 获取英文输入法预设
    pub fn english_preset() -> Vec<KeyMapping> {
        let mut mappings: Vec<KeyMapping> = ('a'..='z').map(|ch| KeyMapping {
            from: KeyEvent::new(ch as u32, Some(ch)),
            to: KeyAction::InputChar(ch),
            enabled: true,
            description: format!("英文字母 {}", ch),
        }).collect();
        // 空格键（VK_SPACE=32）作为输入空格
        mappings.push(KeyMapping {
            from: KeyEvent::new(32, Some(' ')),
            to: KeyAction::InputChar(' '),
            enabled: true,
            description: "空格".to_string(),
        });
        mappings
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
            let _ = *count;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mappings_contains_confirm() {
        let mapper = KeyMapper::new();
        let mappings = mapper.default_mappings_ref();
        assert!(mappings.iter().any(|m| matches!(m.to, KeyAction::Confirm)));
        assert!(mappings.iter().any(|m| matches!(m.to, KeyAction::DeleteChar)));
        assert!(mappings.iter().any(|m| matches!(m.to, KeyAction::Cancel)));
    }

    #[test]
    fn test_process_key_maps_enter() {
        let mapper = KeyMapper::new();
        let enter = KeyEvent::new(13, None);
        match mapper.process_key(&enter) {
            Some(KeyAction::Confirm) => {}
            _ => panic!("回车应映射为 Confirm"),
        }
    }

    #[test]
    fn test_load_and_save_config() {
        let mut mapper = KeyMapper::new();
        mapper.load_config("13=Confirm\n8=DeleteChar\na=A\n").unwrap();
        let saved = mapper.save_config().unwrap();
        assert!(saved.contains("13=Confirm"));
        assert!(saved.contains("8=DeleteChar"));
    }

    #[test]
    fn test_load_config_invalid_line() {
        let mut mapper = KeyMapper::new();
        assert!(mapper.load_config("not-a-valid-line").is_err());
    }

    #[test]
    fn test_pinyin_preset() {
        let preset = KeyPresets::pinyin_preset();
        assert_eq!(preset.len(), 26);
        assert!(preset[0].from.code == 'a' as u32);
        assert!(matches!(preset[0].to, KeyAction::InputChar('a')));
    }
}