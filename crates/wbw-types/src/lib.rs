//! 共享类型定义
//!
//! 包含所有 crate 共享的类型定义，用于打破循环依赖

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// 输入法错误类型
#[derive(Error, Debug, Clone)]
pub enum ImeError {
    #[error("词典加载失败: {0}")]
    DictLoadError(String),

    #[error("码表解析错误: {0}")]
    ParseError(String),

    #[error("匹配失败: {0}")]
    MatchError(String),

    #[error("排序失败: {0}")]
    RankError(String),

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("IO 错误: {0}")]
    IoError(String),

    #[error("N-gram 模型错误: {0}")]
    NgramError(String),

    #[error("未知错误")]
    Unknown,
}

/// 输入法结果类型
pub type ImeResult<T> = Result<T, ImeError>;

/// 候选词数据结构
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candidate {
    /// 词文本
    pub text: String,
    /// 编码（如拼音）
    pub code: String,
    /// 词频分数
    pub score: f64,
    /// 来源标识（如用户词库、系统词库）
    pub source: CandidateSource,
    /// N-gram 评分（可选）
    pub ngram_score: Option<f64>,
    /// 用户权重（可选）
    pub user_weight: Option<f64>,
}

/// 候选词来源
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CandidateSource {
    /// 系统词典
    System,
    /// 用户词典
    User,
    /// 动态学习词典
    Dynamic,
    /// 短语/固定词组
    Phrase,
}

impl fmt::Display for CandidateSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CandidateSource::System => write!(f, "系统"),
            CandidateSource::User => write!(f, "用户"),
            CandidateSource::Dynamic => write!(f, "动态"),
            CandidateSource::Phrase => write!(f, "短语"),
        }
    }
}

/// 输入上下文
#[derive(Debug, Clone)]
pub struct InputContext {
    /// 当前输入缓冲区
    pub buffer: String,
    /// 光标位置
    pub cursor: usize,
    /// 输入模式
    pub mode: InputMode,
    /// 已选择的候选词
    pub selected: Vec<String>,
    /// 会话 ID
    pub session_id: u64,
}

/// 输入模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputMode {
    /// 拼音输入
    Pinyin,
    /// 五笔输入
    Wubi,
    /// 英文输入
    English,
    /// 符号输入
    Symbol,
}

/// 会话状态
#[derive(Debug, Clone)]
pub struct Session {
    /// 会话 ID
    pub id: u64,
    /// 当前输入上下文
    pub context: InputContext,
    /// 候选词列表
    pub candidates: Vec<Candidate>,
    /// 配置
    pub config: SessionConfig,
}

/// 会话配置
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 最大候选词数量
    pub max_candidates: usize,
    /// 是否启用 N-gram 评分
    pub ngram_enabled: bool,
    /// 用户词库路径
    pub user_dict_path: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            fuzzy_enabled: true,
            max_candidates: 10,
            ngram_enabled: true,
            user_dict_path: None,
        }
    }
}

/// 编码到词条的映射
#[derive(Debug, Clone)]
pub struct CodeEntry {
    /// 编码（如拼音）
    pub code: String,
    /// 词条列表
    pub entries: Vec<WordEntry>,
}

/// 词条数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordEntry {
    /// 词文本
    pub word: String,
    /// 词频
    pub freq: u32,
    /// 词性（可选）
    pub pos: Option<String>,
}

/// 匹配结果
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// 匹配的候选词
    pub candidates: Vec<Candidate>,
    /// 匹配耗时（毫秒）
    pub elapsed_ms: f64,
    /// 是否完全匹配
    pub exact_match: bool,
}

/// 排序配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankConfig {
    /// 拼音匹配权重
    pub pin_weight: f64,
    /// 用户词库权重
    pub user_weight: f64,
    /// 词频权重
    pub freq_weight: f64,
    /// N-gram 权重
    pub ngram_weight: f64,
    /// 最大候选词数量
    pub max_candidates: usize,
}

impl Default for RankConfig {
    fn default() -> Self {
        Self {
            pin_weight: 100.0,
            user_weight: 10.0,
            freq_weight: 1.0,
            ngram_weight: 0.5,
            max_candidates: 10,
        }
    }
}

/// N-gram 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NgramConfig {
    /// N-gram 阶数（如 2 表示 bigram）
    pub order: usize,
    /// 平滑参数
    pub smooth: f64,
    /// 模型文件路径
    pub model_path: Option<String>,
}

impl Default for NgramConfig {
    fn default() -> Self {
        Self {
            order: 2,
            smooth: 0.1,
            model_path: None,
        }
    }
}

/// 词典配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictConfig {
    /// 基础词典路径
    pub base_path: String,
    /// N-gram 模型路径
    pub ngram_path: Option<String>,
    /// 用户词典路径
    pub user_dict_path: Option<String>,
}

/// 匹配器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatcherConfig {
    /// 是否启用模糊匹配
    pub fuzzy: bool,
    /// 模糊匹配规则
    pub fuzzy_rules: Vec<String>,
}

/// L0 动态学习配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L0Config {
    /// 触发阈值
    pub threshold: u32,
    /// 快照路径
    pub snapshot_path: String,
}

impl Default for L0Config {
    fn default() -> Self {
        Self {
            threshold: 3,
            snapshot_path: "wbw_l0.json".to_string(),
        }
    }
}

/// 全局配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// 词典配置
    pub dict: DictConfig,
    /// 匹配器配置
    pub matcher: MatcherConfig,
    /// 排序配置
    pub rank: RankConfig,
    /// L0 学习配置
    pub l0: L0Config,
    /// N-gram 配置
    pub ngram: NgramConfig,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            dict: DictConfig {
                base_path: "resources/dicts/base.cin".to_string(),
                ngram_path: Some("resources/dicts/ngram.bin".to_string()),
                user_dict_path: None,
            },
            matcher: MatcherConfig {
                fuzzy: true,
                fuzzy_rules: vec![
                    "z->zh".to_string(),
                    "c->ch".to_string(),
                    "s->sh".to_string(),
                    "n->l".to_string(),
                    "l->n".to_string(),
                ],
            },
            rank: RankConfig::default(),
            l0: L0Config {
                threshold: 3,
                snapshot_path: "wbw_l0.json".to_string(),
            },
            ngram: NgramConfig::default(),
        }
    }
}
