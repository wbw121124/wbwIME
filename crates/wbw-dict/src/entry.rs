//! 词条数据结构定义

use serde::{Deserialize, Serialize};
use std::fmt;
use wbw_types::WordEntry;

/// 码表条目（.cin 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CinEntry {
    /// 编码（如拼音）
    pub code: String,
    /// 词条列表
    pub words: Vec<WordEntry>,
}

impl CinEntry {
    /// 创建新的码表条目
    pub fn new(code: String) -> Self {
        Self {
            code,
            words: Vec::new(),
        }
    }

    /// 添加词条
    pub fn add_word(&mut self, word: WordEntry) {
        self.words.push(word);
    }

    /// 获取词条数量
    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

/// 词典条目（用于 FST 词典）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictEntry {
    /// 编码
    pub code: String,
    /// 词文本
    pub word: String,
    /// 词频
    pub freq: u32,
    /// 来源
    pub source: DictSource,
}

/// 词典来源
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DictSource {
    /// 基础词典
    Base,
    /// 用户词典
    User,
    /// 动态词典
    Dynamic,
}

impl fmt::Display for DictSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictSource::Base => write!(f, "基础"),
            DictSource::User => write!(f, "用户"),
            DictSource::Dynamic => write!(f, "动态"),
        }
    }
}

/// 词典统计信息
#[derive(Debug, Clone, Default)]
pub struct DictStats {
    /// 总词条数
    pub total_entries: usize,
    /// 总编码数
    pub total_codes: usize,
    /// 平均每编码词条数
    pub avg_words_per_code: f64,
    /// 最高频词
    pub top_words: Vec<(String, u32)>,
}

/// 词典查询结果
#[derive(Debug, Clone)]
pub struct DictQueryResult {
    /// 匹配的词条
    pub entries: Vec<DictEntry>,
    /// 查询耗时（毫秒）
    pub elapsed_ms: f64,
    /// 是否精确匹配
    pub exact_match: bool,
}

/// 词典构建配置
#[derive(Debug, Clone)]
pub struct DictBuilderConfig {
    /// 是否排序词条
    pub sort_entries: bool,
    /// 是否去重
    pub deduplicate: bool,
    /// 最小词频
    pub min_freq: u32,
    /// 最大词条长度
    pub max_word_len: usize,
}

impl Default for DictBuilderConfig {
    fn default() -> Self {
        Self {
            sort_entries: true,
            deduplicate: true,
            min_freq: 0,
            max_word_len: 32,
        }
    }
}