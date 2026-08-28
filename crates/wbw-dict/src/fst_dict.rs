//! FST 词典实现
//!
//! 提供基于哈希表的内存词典，支持精确查询和前缀查询。
//! 后续可替换为 FST（有限状态转换器）实现以获得更优的内存和查询性能。

use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;
use wbw_types::ImeResult;

use crate::entry::{DictEntry, DictSource};

/// FST 词典错误
#[derive(Error, Debug)]
pub enum FstDictError {
    #[error("词典加载失败: {0}")]
    LoadError(String),

    #[error("词典构建失败: {0}")]
    BuildError(String),
}

/// FST 词典
///
/// 内部使用 HashMap 存储编码到词条列表的映射，
/// 支持精确查询、前缀查询和模糊查询。
pub struct FstDict {
    /// 编码 → 词条列表
    entries: HashMap<String, Vec<DictEntry>>,
    /// 词条总数
    entry_count: usize,
    /// 词典来源
    source: DictSource,
}

impl FstDict {
    /// 从文件加载词典（预留接口，当前未使用 FST 文件格式）
    pub fn from_file(_path: &Path) -> ImeResult<Self> {
        Ok(Self {
            entries: HashMap::new(),
            entry_count: 0,
            source: DictSource::Base,
        })
    }

    /// 从内存数据构建词典
    pub fn from_entries(entries: Vec<DictEntry>, source: DictSource) -> Self {
        let mut map: HashMap<String, Vec<DictEntry>> = HashMap::new();
        let mut total = 0;

        for entry in entries {
            total += 1;
            map.entry(entry.code.clone()).or_default().push(entry);
        }

        Self {
            entries: map,
            entry_count: total,
            source,
        }
    }

    /// 精确查询编码
    pub fn lookup(&self, code: &str) -> Vec<&DictEntry> {
        self.entries
            .get(code)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// 前缀查询：返回所有编码以 prefix 开头的词条
    pub fn prefix_lookup(&self, prefix: &str) -> Vec<&DictEntry> {
        let mut result = Vec::new();
        for (code, entries) in &self.entries {
            if code.starts_with(prefix) {
                result.extend(entries.iter());
            }
        }
        // 按词频降序排序
        result.sort_by_key(|e| Reverse(e.freq));
        result
    }

    /// 模糊查询：查找与目标编码编辑距离在 max_edit_distance 以内的词条
    pub fn fuzzy_lookup(&self, code: &str, max_edit_distance: usize) -> Vec<(&DictEntry, usize)> {
        let mut result = Vec::new();
        for (dict_code, entries) in &self.entries {
            let dist = edit_distance(code, dict_code);
            if dist <= max_edit_distance {
                for entry in entries {
                    result.push((entry, dist));
                }
            }
        }
        // 按编辑距离升序、词频降序排序
        result.sort_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| b.0.freq.cmp(&a.0.freq))
        });
        result
    }

    /// 获取词条总数
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// 获取编码数量（不同编码的个数）
    pub fn code_count(&self) -> usize {
        self.entries.len()
    }

    /// 获取词典来源
    pub fn source(&self) -> DictSource {
        self.source
    }

    /// 检查词典是否为空
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// 获取词典统计信息
    pub fn stats(&self) -> DictStats {
        let total_entries = self.entry_count;
        let total_codes = self.entries.len();

        let avg_words_per_code = if total_codes > 0 {
            total_entries as f64 / total_codes as f64
        } else {
            0.0
        };

        // 收集所有词条并按词频排序
        let mut all_words: Vec<(String, u32)> = self
            .entries
            .values()
            .flatten()
            .map(|e| (e.word.clone(), e.freq))
            .collect();
        all_words.sort_by_key(|w| Reverse(w.1));
        all_words.truncate(10);

        DictStats {
            total_entries,
            total_codes,
            avg_words_per_code,
            top_words: all_words,
        }
    }

    /// 合并另一个词典（去重）
    pub fn merge(&mut self, other: &FstDict) {
        for (code, entries) in &other.entries {
            let target = self.entries.entry(code.clone()).or_default();
            for entry in entries {
                // 检查是否已存在相同词条
                if !target.iter().any(|e| e.word == entry.word) {
                    target.push(entry.clone());
                    self.entry_count += 1;
                }
            }
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

/// 词典构建器
pub struct FstDictBuilder {
    entries: Vec<DictEntry>,
}

impl FstDictBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// 添加词条
    pub fn add_entry(&mut self, entry: DictEntry) {
        self.entries.push(entry);
    }

    /// 批量添加词条
    pub fn add_entries(&mut self, entries: Vec<DictEntry>) {
        self.entries.extend(entries);
    }

    /// 构建词典
    pub fn build(self, source: DictSource) -> FstDict {
        FstDict::from_entries(self.entries, source)
    }

    /// 获取词条数量
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 清空词条
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for FstDictBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 计算两个字符串的编辑距离（Levenshtein 距离）
pub fn edit_distance(s1: &str, s2: &str) -> usize {
    let s1: Vec<char> = s1.chars().collect();
    let s2: Vec<char> = s2.chars().collect();
    let m = s1.len();
    let n = s2.len();

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in dp[0].iter_mut().enumerate() {
        *cell = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if s1[i - 1] == s2[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

/// 合并两个词典
pub fn merge_dicts(dict1: &FstDict, dict2: &FstDict) -> FstDict {
    let mut result = FstDict::from_entries(Vec::new(), dict1.source);
    result.merge(dict1);
    result.merge(dict2);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::DictEntry;

    fn make_entry(code: &str, word: &str, freq: u32) -> DictEntry {
        DictEntry {
            code: code.to_string(),
            word: word.to_string(),
            freq,
            source: DictSource::Base,
        }
    }

    #[test]
    fn test_lookup() {
        let entries = vec![
            make_entry("wo", "我", 100),
            make_entry("wo", "喔", 50),
            make_entry("ai", "爱", 200),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        let results = dict.lookup("wo");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].word, "我");
        assert_eq!(results[1].word, "喔");

        let results = dict.lookup("ai");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "爱");

        let results = dict.lookup("xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_prefix_lookup() {
        let entries = vec![
            make_entry("shi", "是", 100),
            make_entry("shi", "时", 80),
            make_entry("shijie", "世界", 200),
            make_entry("shi", "十", 90),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        let results = dict.prefix_lookup("shi");
        assert_eq!(results.len(), 4); // 是、时、十、世界
    }

    #[test]
    fn test_fuzzy_lookup() {
        let entries = vec![
            make_entry("zhongguo", "中国", 1000),
            make_entry("zhongguo", "忠国", 10),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        // "zongguo" 与 "zhongguo" 编辑距离为 1
        let results = dict.fuzzy_lookup("zongguo", 1);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, 1); // 编辑距离
    }

    #[test]
    fn test_edit_distance() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("abc", "ab"), 1);
        assert_eq!(edit_distance("abc", "ac"), 1);
        assert_eq!(edit_distance("abc", "def"), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_merge() {
        let entries1 = vec![make_entry("wo", "我", 100)];
        let entries2 = vec![
            make_entry("wo", "我", 100), // 重复
            make_entry("wo", "喔", 50),  // 新增
        ];
        let mut dict1 = FstDict::from_entries(entries1, DictSource::Base);
        let dict2 = FstDict::from_entries(entries2, DictSource::User);

        dict1.merge(&dict2);
        assert_eq!(dict1.entry_count(), 2); // 我、喔
    }

    #[test]
    fn test_stats() {
        let entries = vec![
            make_entry("wo", "我", 100),
            make_entry("wo", "喔", 50),
            make_entry("ai", "爱", 200),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);
        let stats = dict.stats();
        assert_eq!(stats.total_entries, 3);
        assert_eq!(stats.total_codes, 2);
        assert!((stats.avg_words_per_code - 1.5).abs() < 0.01);
    }

    #[test]
    fn test_builder() {
        let mut builder = FstDictBuilder::new();
        builder.add_entry(make_entry("wo", "我", 100));
        builder.add_entry(make_entry("ai", "爱", 200));
        let dict = builder.build(DictSource::Base);
        assert_eq!(dict.entry_count(), 2);
    }
}
