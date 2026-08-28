//! 匹配器主体模块
//!
//! 组合词典查询、模糊匹配、分词，提供统一的输入匹配接口。

use std::num::NonZeroUsize;
use std::time::Instant;
use wbw_dict::entry::{DictEntry, DictSource};
use wbw_dict::{CinParser, FstDict, FstDictBuilder};
use wbw_types::{Candidate, CandidateSource, InputContext};
use crate::fuzzy::{FuzzyConfig, FuzzyMatcher, FuzzyRule};

/// 匹配器配置
#[derive(Debug, Clone)]
pub struct MatcherConfig {
    /// 是否启用模糊匹配
    pub fuzzy_enabled: bool,
    /// 模糊匹配配置
    pub fuzzy_config: FuzzyConfig,
    /// 最大候选词数量
    pub max_candidates: usize,
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
            max_candidates: 10,
            cache_enabled: true,
            cache_size: 1000,
        }
    }
}

/// 匹配器
///
/// 核心匹配引擎，组合词典查询、模糊匹配，提供统一的输入匹配接口。
pub struct Matcher {
    config: MatcherConfig,
    dict: Option<FstDict>,
    fuzzy_matcher: FuzzyMatcher,
    cache: Option<lru::LruCache<String, Vec<Candidate>>>,
}

impl Matcher {
    /// 创建空匹配器（未加载词典）
    pub fn new(config: MatcherConfig) -> Self {
        let cache = if config.cache_enabled {
            Self::new_cache(config.cache_size)
        } else {
            None
        };
        let fuzzy_matcher = FuzzyMatcher::new(config.fuzzy_config.clone());
        Self {
            config,
            dict: None,
            fuzzy_matcher,
            cache,
        }
    }

    /// 创建带词典的匹配器
    pub fn with_dict(config: MatcherConfig, dict: FstDict) -> Self {
        let cache = if config.cache_enabled {
            Self::new_cache(config.cache_size)
        } else {
            None
        };
        let fuzzy_matcher = FuzzyMatcher::new(config.fuzzy_config.clone());
        Self {
            config,
            dict: Some(dict),
            fuzzy_matcher,
            cache,
        }
    }

    /// 构造 LRU 缓存，容量下限为 1
    fn new_cache(size: usize) -> Option<lru::LruCache<String, Vec<Candidate>>> {
        Some(lru::LruCache::new(NonZeroUsize::new(size.max(1))?))
    }

    /// 从 .cin 文件加载词典
    pub fn load_cin(&mut self, path: &str) {
        let parser = CinParser::new(path);
        if let Ok(cin_entries) = parser.parse() {
            let mut builder = FstDictBuilder::new();
            for cin_entry in &cin_entries {
                for word_entry in &cin_entry.words {
                    builder.add_entry(DictEntry {
                        code: cin_entry.code.clone(),
                        word: word_entry.word.clone(),
                        freq: word_entry.freq,
                        source: DictSource::Base,
                    });
                }
            }
            self.dict = Some(builder.build(DictSource::Base));
            self.clear_cache();
        }
    }

    /// 加载词典
    pub fn load_dict(&mut self, dict: FstDict) {
        self.dict = Some(dict);
        self.clear_cache();
    }

    /// 匹配输入上下文
    ///
    /// 根据输入缓冲区返回候选词列表。
    pub fn match_input(&mut self, context: &InputContext) -> Vec<Candidate> {
        let code = &context.buffer;
        if code.is_empty() {
            return Vec::new();
        }

        // 检查缓存
        if let Some(cache) = &mut self.cache {
            if let Some(cached) = cache.get(code) {
                return cached.clone();
            }
        }

        let start = Instant::now();
        let mut candidates = self.do_match(code);

        // 截断到最大候选数
        candidates.truncate(self.config.max_candidates);

        // 缓存结果
        if let Some(cache) = &mut self.cache {
            cache.put(code.clone(), candidates.clone());
        }

        let _elapsed = start.elapsed().as_millis();
        candidates
    }

    /// 执行匹配（内部逻辑）
    fn do_match(&self, code: &str) -> Vec<Candidate> {
        let mut candidates = Vec::new();

        // 1. 精确匹配
        let exact = self.exact_lookup(code);
        candidates.extend(exact);

        // 2. 前缀匹配
        let prefix = self.prefix_lookup(code);
        candidates.extend(prefix);

        // 3. 模糊匹配（如果启用）
        if self.config.fuzzy_enabled {
            let fuzzy = self.fuzzy_lookup(code);
            candidates.extend(fuzzy);
        }

        // 按分数降序排序
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        // 全局去重（保留首个即最高分），不受相邻性限制
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.text.clone()));
        candidates
    }

    /// 精确匹配：查找与输入完全相同的编码
    pub fn exact_lookup(&self, code: &str) -> Vec<Candidate> {
        let dict = match &self.dict {
            Some(d) => d,
            None => return Vec::new(),
        };

        dict.lookup(code)
            .into_iter()
            .map(|entry| Candidate {
                text: entry.word.clone(),
                code: code.to_string(),
                score: 100.0 + entry.freq as f64,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            })
            .collect()
    }

    /// 前缀匹配：查找以输入为前缀的编码
    pub fn prefix_lookup(&self, code: &str) -> Vec<Candidate> {
        let dict = match &self.dict {
            Some(d) => d,
            None => return Vec::new(),
        };

        dict.prefix_lookup(code)
            .into_iter()
            .map(|entry| {
                let score = if entry.code == code {
                    90.0
                } else {
                    70.0 - (entry.code.len() as f64 - code.len() as f64) * 5.0
                };
                Candidate {
                    text: entry.word.clone(),
                    code: entry.code.clone(),
                    score,
                    source: CandidateSource::System,
                    ngram_score: None,
                    user_weight: None,
                }
            })
            .collect()
    }

    /// 模糊匹配：编辑距离（Levenshtein）查找 + 拼音规则变体查找
    pub fn fuzzy_lookup(&self, code: &str) -> Vec<Candidate> {
        if !self.config.fuzzy_enabled {
            return Vec::new();
        }
        let dict = match &self.dict {
            Some(d) => d,
            None => return Vec::new(),
        };

        let max_edit = self.config.fuzzy_config.max_edit_distance;
        let mut candidates: Vec<Candidate> = dict
            .fuzzy_lookup(code, max_edit)
            .into_iter()
            .map(|(entry, dist)| {
                // 分数随编辑距离增大而降低
                let score = if dist == 0 {
                    100.0
                } else {
                    50.0 - (dist as f64) * 10.0
                };
                Candidate {
                    text: entry.word.clone(),
                    code: entry.code.clone(),
                    score,
                    source: CandidateSource::System,
                    ngram_score: None,
                    user_weight: None,
                }
            })
            .collect();

        // 规则变体（如 z→zh、ei→ie 对调等编辑距离引擎覆盖不到的错法）
        for variant in self.fuzzy_matcher.generate_variants(code) {
            if variant == code {
                continue;
            }
            for entry in dict.lookup(&variant) {
                candidates.push(Candidate {
                    text: entry.word.clone(),
                    code: entry.code.clone(),
                    score: 80.0,
                    source: CandidateSource::System,
                    ngram_score: None,
                    user_weight: None,
                });
            }
        }

        // 按 (text, code) 去重并保留最高分
        candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert((c.text.clone(), c.code.clone())));
        candidates
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

    /// 获取缓存大小
    pub fn cache_len(&self) -> usize {
        self.cache.as_ref().map_or(0, |c| c.len())
    }
}

/// 匹配策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchStrategy {
    Exact,
    Prefix,
    Fuzzy,
}

/// 匹配选项
#[derive(Debug, Clone)]
pub struct MatchOptions {
    pub strategy: MatchStrategy,
    pub max_results: usize,
}

impl Default for MatchOptions {
    fn default() -> Self {
        Self {
            strategy: MatchStrategy::Prefix,
            max_results: 10,
        }
    }
}

/// 匹配器构建器
pub struct MatcherBuilder {
    config: MatcherConfig,
    dict: Option<FstDict>,
}

impl MatcherBuilder {
    pub fn new() -> Self {
        Self {
            config: MatcherConfig::default(),
            dict: None,
        }
    }

    pub fn with_config(mut self, config: MatcherConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_dict(mut self, dict: FstDict) -> Self {
        self.dict = Some(dict);
        self
    }

    pub fn with_fuzzy(mut self, enabled: bool) -> Self {
        self.config.fuzzy_enabled = enabled;
        self
    }

    pub fn with_fuzzy_rules(mut self, rules: Vec<FuzzyRule>) -> Self {
        self.config.fuzzy_config.rules = rules;
        self
    }

    pub fn with_max_candidates(mut self, max: usize) -> Self {
        self.config.max_candidates = max;
        self
    }

    pub fn with_cache(mut self, enabled: bool, size: usize) -> Self {
        self.config.cache_enabled = enabled;
        self.config.cache_size = size;
        self
    }

    pub fn build(self) -> Matcher {
        match self.dict {
            Some(dict) => Matcher::with_dict(self.config, dict),
            None => Matcher::new(self.config),
        }
    }
}

impl Default for MatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_dict() -> FstDict {
        let mut builder = FstDictBuilder::new();
        let entries = vec![
            DictEntry { code: "wo".into(), word: "我".into(), freq: 100, source: DictSource::Base },
            DictEntry { code: "wo".into(), word: "喔".into(), freq: 50, source: DictSource::Base },
            DictEntry { code: "ai".into(), word: "爱".into(), freq: 200, source: DictSource::Base },
            DictEntry { code: "ni".into(), word: "你".into(), freq: 150, source: DictSource::Base },
            DictEntry { code: "zhongguo".into(), word: "中国".into(), freq: 300, source: DictSource::Base },
            DictEntry { code: "zhongyu".into(), word: "中雨".into(), freq: 80, source: DictSource::Base },
        ];
        builder.add_entries(entries);
        builder.build(DictSource::Base)
    }

    #[test]
    fn test_exact_lookup() {
        let dict = build_test_dict();
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.exact_lookup("wo");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "我");
    }

    #[test]
    fn test_exact_lookup_miss() {
        let dict = build_test_dict();
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.exact_lookup("xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_prefix_lookup() {
        let dict = build_test_dict();
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.prefix_lookup("zhong");
        assert!(!results.is_empty());
        let codes: Vec<&str> = results.iter().map(|c| c.code.as_str()).collect();
        assert!(codes.contains(&"zhongguo"));
        assert!(codes.contains(&"zhongyu"));
    }

    #[test]
    fn test_fuzzy_lookup() {
        let dict = build_test_dict();
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.fuzzy_lookup("zongguo");
        // z→zh 规则应生成 "zhongguo" 变体
        assert!(!results.is_empty());
    }

    #[test]
    fn test_fuzzy_lookup_edit_distance() {
        // 编辑距离类模糊：输入缺字符（zdlu）能匹配到 zdliu
        let mut builder = FstDictBuilder::new();
        builder.add_entries(vec![DictEntry {
            code: "zdliu".into(),
            word: "最大流".into(),
            freq: 100,
            source: DictSource::Base,
        }]);
        let dict = builder.build(DictSource::Base);
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.fuzzy_lookup("zdlu");
        assert!(!results.is_empty(), "缺字符编码应通过编辑距离模糊匹配到");
        assert_eq!(results[0].text, "最大流");
        assert_eq!(results[0].code, "zdliu");
    }

    #[test]
    fn test_fuzzy_lookup_transposition_rule() {
        // 对调规则 ei→ie 编辑距离为 2，编辑距离引擎(默认 max=1)覆盖不到，
        // 只能由规则变体引擎命中，验证接线生效。
        let mut builder = FstDictBuilder::new();
        builder.add_entries(vec![DictEntry {
            code: "qie".into(),
            word: "切".into(),
            freq: 100,
            source: DictSource::Base,
        }]);
        let dict = builder.build(DictSource::Base);
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.fuzzy_lookup("qei");
        assert!(!results.is_empty(), "对调规则 ei→ie 应通过规则引擎命中");
        assert_eq!(results[0].text, "切");
    }

    #[test]
    fn test_fuzzy_lookup_too_far_ignored() {
        // 超出最大编辑距离的编码不应被模糊匹配到
        let mut builder = FstDictBuilder::new();
        builder.add_entries(vec![DictEntry {
            code: "wo".into(),
            word: "我".into(),
            freq: 100,
            source: DictSource::Base,
        }]);
        let dict = builder.build(DictSource::Base);
        let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let results = matcher.fuzzy_lookup("zhongguo");
        // "wo" 到 "zhongguo" 编辑距离很大，应无结果
        assert!(results.is_empty());
    }

    #[test]
    fn test_do_match_dedup_by_text() {
        // 不同编码命中同一词（zdl / zdliu → 最大流）时，
        // do_match 应按词去重，只保留最高分的一个候选。
        let mut builder = FstDictBuilder::new();
        builder.add_entries(vec![
            DictEntry { code: "zdl".into(), word: "最大流".into(), freq: 100, source: DictSource::Base },
            DictEntry { code: "zdliu".into(), word: "最大流".into(), freq: 50, source: DictSource::Base },
        ]);
        let dict = builder.build(DictSource::Base);
        let mut matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let context = InputContext {
            buffer: "zdl".to_string(),
            cursor: 0,
            mode: wbw_types::InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let results = matcher.match_input(&context);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text, "最大流");
    }

    #[test]
    fn test_match_input() {
        let dict = build_test_dict();
        let mut matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let context = InputContext {
            buffer: "wo".to_string(),
            cursor: 0,
            mode: wbw_types::InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let results = matcher.match_input(&context);
        assert!(!results.is_empty());
        assert_eq!(results[0].text, "我");
    }

    #[test]
    fn test_match_input_empty() {
        let dict = build_test_dict();
        let mut matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let context = InputContext {
            buffer: String::new(),
            cursor: 0,
            mode: wbw_types::InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let results = matcher.match_input(&context);
        assert!(results.is_empty());
    }

    #[test]
    fn test_cache() {
        let dict = build_test_dict();
        let mut matcher = Matcher::with_dict(MatcherConfig::default(), dict);
        let context = InputContext {
            buffer: "wo".to_string(),
            cursor: 0,
            mode: wbw_types::InputMode::Pinyin,
            selected: Vec::new(),
            session_id: 0,
        };
        let _ = matcher.match_input(&context);
        assert_eq!(matcher.cache_len(), 1);
        let _ = matcher.match_input(&context);
        assert_eq!(matcher.cache_len(), 1);
    }

    #[test]
    fn test_builder() {
        let dict = build_test_dict();
        let matcher = MatcherBuilder::new()
            .with_dict(dict)
            .with_max_candidates(5)
            .with_cache(true, 100)
            .build();
        assert_eq!(matcher.config().max_candidates, 5);
    }
}
