//! 匹配器主体模块

use std::num::NonZeroUsize;
use std::fmt;
use thiserror::Error;
use wbw_types::{Candidate, ImeError, ImeResult, MatchResult, InputContext};
use crate::fuzzy::{FuzzyConfig, FuzzyMatcher, FuzzyRule};
use crate::pinyin::PinyinString;
use crate::segmenter::Segmenter;

/// 匹配器错误类型
#[derive(Error, Debug)]
pub enum MatcherError {
    #[error("词典查询失败: {0}")]
    DictError(String),
    
    #[error("拼音解析失败: {0}")]
    PinyinError(String),
    
    #[error("分词失败: {0}")]
    SegmentError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// 匹配器配置
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 模糊匹配配置
    pub fuzzy_config: FuzzyConfig,
    /// 是否启用分词
    pub segment_enabled: bool,
    /// 最大候选词数量
    pub max_candidates: usize,
    /// 最小匹配分数
    pub min_score: f64,
    /// 是否启用缓存
    pub cache_enabled: bool,
    /// 缓存大小
    pub cache_size: usize,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self {
            fuzzy_enabled: true,
            fuzzy_config: FuzzyConfig::default(),
            segment_enabled: true,
            max_candidates: 10,
            min_score: 0.0,
            cache_enabled: true,
            cache_size: 1000,
        }
    }
}

/// 匹配器
pub struct Matcher {
    /// 配置
    config: MatcherConfig,
    /// 模糊匹配器
    fuzzy_matcher: FuzzyMatcher,
    /// 分词器
    segmenter: Segmenter,
    /// 匹配缓存
    cache: Option<lru::LruCache<String, Vec<Candidate>>>,
}

impl Matcher {
    /// 创建新的匹配器
    pub fn new(config: MatcherConfig) -> Self {
        let fuzzy_matcher = FuzzyMatcher::new(config.fuzzy_config.clone());
        let segmenter = Segmenter::new();
        
        let cache = if config.cache_enabled {
            Some(lru::LruCache::new(NonZeroUsize::new(config.cache_size).unwrap()))
        } else {
            None
        };
        
        Self {
            config,
            fuzzy_matcher,
            segmenter,
            cache,
        }
    }

    /// 匹配输入
    pub fn match_input(&mut self, context: &InputContext) -> ImeResult<MatchResult> {
        // TODO: 实现匹配逻辑
        todo!("实现输入匹配")
    }

    /// 精确匹配
    pub fn exact_match(&self, code: &str) -> ImeResult<Vec<Candidate>> {
        // TODO: 实现精确匹配逻辑
        todo!("实现精确匹配")
    }

    /// 前缀匹配
    pub fn prefix_match(&self, code: &str) -> ImeResult<Vec<Candidate>> {
        // TODO: 实现前缀匹配逻辑
        todo!("实现前缀匹配")
    }

    /// 模糊匹配
    pub fn fuzzy_match(&self, code: &str) -> ImeResult<Vec<Candidate>> {
        // TODO: 实现模糊匹配逻辑
        todo!("实现模糊匹配")
    }

    /// 清除缓存
    pub fn clear_cache(&mut self) {
        if let Some(cache) = &mut self.cache {
            cache.clear();
        }
    }

    /// 获取配置
    pub fn config(&self) -> &MatcherConfig {
        &self.config
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        // TODO: 实现缓存统计
        todo!("实现缓存命中率统计")
    }
}

/// 匹配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    /// 精确匹配
    Exact,
    /// 前缀匹配
    Prefix,
    /// 后缀匹配
    Suffix,
    /// 包含匹配
    Contains,
    /// 模糊匹配
    Fuzzy,
    /// 正则匹配
    Regex,
}

/// 匹配选项
#[derive(Debug, Clone)]
pub struct MatchOptions {
    /// 匹配策略
    pub strategy: MatchStrategy,
    /// 是否区分大小写
    pub case_sensitive: bool,
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 最大编辑距离
    pub max_edit_distance: usize,
    /// 最大结果数量
    pub max_results: usize,
    /// 最小匹配分数
    pub min_score: f64,
}

impl Default for MatchOptions {
    fn default() -> Self {
        Self {
            strategy: MatchStrategy::Prefix,
            case_sensitive: false,
            fuzzy_enabled: true,
            max_edit_distance: 1,
            max_results: 10,
            min_score: 0.0,
        }
    }
}

/// 匹配结果统计
#[derive(Debug, Clone, Default)]
pub struct MatchStats {
    /// 总匹配次数
    pub total_matches: usize,
    /// 平均匹配耗时（毫秒）
    pub avg_time_ms: f64,
    /// 最大匹配耗时（毫秒）
    pub max_time_ms: f64,
    /// 缓存命中次数
    pub cache_hits: usize,
    /// 缓存未命中次数
    pub cache_misses: usize,
}

/// 匹配器构建器
pub struct MatcherBuilder {
    config: MatcherConfig,
}

impl MatcherBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: MatcherConfig::default(),
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: MatcherConfig) -> Self {
        self.config = config;
        self
    }

    /// 启用模糊匹配
    pub fn with_fuzzy(mut self, enabled: bool) -> Self {
        self.config.fuzzy_enabled = enabled;
        self
    }

    /// 设置模糊规则
    pub fn with_fuzzy_rules(mut self, rules: Vec<FuzzyRule>) -> Self {
        self.config.fuzzy_config.rules = rules;
        self
    }

    /// 设置最大候选词数量
    pub fn with_max_candidates(mut self, max: usize) -> Self {
        self.config.max_candidates = max;
        self
    }

    /// 启用缓存
    pub fn with_cache(mut self, enabled: bool, size: usize) -> Self {
        self.config.cache_enabled = enabled;
        self.config.cache_size = size;
        self
    }

    /// 构建匹配器
    pub fn build(self) -> Matcher {
        Matcher::new(self.config)
    }
}

impl Default for MatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}