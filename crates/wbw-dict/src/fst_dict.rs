//! FST 词典实现
//!
//! 基于 `fst::Map` 的压缩前缀词典，支持精确查询、前缀查询和模糊查询（Levenshtein automaton）。
//! 支持序列化为二进制快照（`.fst` 文件）和 mmap 只读加载。

use memmap2::Mmap;
use std::cmp::Reverse;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use thiserror::Error;
use wbw_types::ImeResult;

use crate::entry::{DictEntry, DictSource};
use fst::{IntoStreamer, Streamer};

/// FST key 中 code 与 word 的分隔符（U+0001，控制字符，不会出现在正常编码或词条中）
pub const KEY_SEP: char = '\u{0001}';

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
/// 内部使用 `fst::Map` 存储压缩前缀数据。
/// key 格式：`code + '\u{0001}' + word`，value：`freq`（u64）。
/// 同一 code 下多个词条自然展开为多个 key。
pub struct FstDict {
    /// FST 映射（内部持有 `Vec<u8>` 字节数据）
    map: fst::Map<Vec<u8>>,
    /// 词条总数
    entry_count: usize,
    /// 编码数量（不同 code 的个数）
    code_count: usize,
    /// 词典来源
    source: DictSource,
}

impl FstDict {
    /// 从二进制字节构建词典
    pub fn from_bytes(bytes: Vec<u8>) -> ImeResult<Self> {
        let map = fst::Map::new(bytes)
            .map_err(|e| wbw_types::ImeError::ParseError(format!("FST 加载失败: {}", e)))?;
        let entry_count = map.len();
        let code_count = Self::count_codes(&map);
        Ok(Self {
            map,
            entry_count,
            code_count,
            source: DictSource::Base,
        })
    }

    /// 从文件 mmap 加载词典（只读映射，适合大词典）
    pub fn from_file(path: &Path) -> ImeResult<Self> {
        let file = File::open(path).map_err(|e| {
            wbw_types::ImeError::IoError(format!("打开文件失败 {}: {}", path.display(), e))
        })?;
        let mmap = unsafe {
            Mmap::map(&file)
                .map_err(|e| wbw_types::ImeError::IoError(format!("mmap 映射失败: {}", e)))?
        };
        let bytes = mmap.to_vec();
        let map = fst::Map::new(bytes)
            .map_err(|e| wbw_types::ImeError::ParseError(format!("FST 加载失败: {}", e)))?;
        let entry_count = map.len();
        let code_count = Self::count_codes(&map);
        Ok(Self {
            map,
            entry_count,
            code_count,
            source: DictSource::Base,
        })
    }

    /// 从词条列表构建词典
    pub fn from_entries(entries: Vec<DictEntry>, source: DictSource) -> Self {
        let mut builder = fst::MapBuilder::memory();

        // 按 code 排序以确保 FST 有序性
        let mut sorted: Vec<DictEntry> = entries;
        sorted.sort_by(|a, b| a.code.cmp(&b.code).then_with(|| a.word.cmp(&b.word)));

        let mut entry_count = 0usize;
        let mut code_set = HashSet::new();

        for entry in &sorted {
            let key = format!("{}{}{}", entry.code, KEY_SEP, entry.word);
            builder
                .insert(key.as_bytes(), entry.freq as u64)
                .expect("FST builder insert failed");
            entry_count += 1;
            code_set.insert(entry.code.clone());
        }

        let bytes = builder.into_inner().expect("FST builder finalize failed");
        let map = fst::Map::new(bytes).expect("FST map construction failed");

        Self {
            map,
            entry_count,
            code_count: code_set.len(),
            source,
        }
    }

    /// 序列化为二进制字节（可写入 .fst 文件）
    pub fn to_bytes(&self) -> Vec<u8> {
        self.map.as_ref().as_bytes().to_vec()
    }

    /// 写入 .fst 文件
    pub fn write_to_file(&self, path: &Path) -> ImeResult<()> {
        let file = File::create(path).map_err(|e| {
            wbw_types::ImeError::IoError(format!("创建文件失败 {}: {}", path.display(), e))
        })?;
        let mut writer = BufWriter::new(file);
        writer
            .write_all(self.map.as_ref().as_bytes())
            .map_err(|e| wbw_types::ImeError::IoError(format!("写入文件失败: {}", e)))?;
        Ok(())
    }

    /// 精确查询编码（返回该编码下所有词条，按词频降序）
    pub fn lookup(&self, code: &str) -> Vec<DictEntry> {
        let prefix = format!("{}{}", code, KEY_SEP);
        let mut results = Vec::new();
        let mut stream = self.map.range().ge(prefix.as_bytes()).into_stream();
        while let Some((key, freq)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(rest) = key_str.strip_prefix(&prefix) {
                results.push(DictEntry {
                    code: code.to_string(),
                    word: rest.to_string(),
                    freq: freq as u32,
                    source: self.source,
                });
            } else {
                break;
            }
        }
        results.sort_by_key(|e| Reverse(e.freq));
        results
    }

    /// 前缀查询：返回所有编码以 prefix 开头的词条
    pub fn prefix_lookup(&self, prefix: &str) -> Vec<DictEntry> {
        let mut results = Vec::new();
        // 用 prefix 本身（不含 SEP）作为 range 下界，匹配所有以 prefix 开头的 code
        let mut stream = self.map.range().ge(prefix.as_bytes()).into_stream();
        while let Some((key, freq)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(code_end) = key_str.find(KEY_SEP) {
                let code = &key_str[..code_end];
                if code.starts_with(prefix) {
                    let word = &key_str[code_end + 1..];
                    results.push(DictEntry {
                        code: code.to_string(),
                        word: word.to_string(),
                        freq: freq as u32,
                        source: self.source,
                    });
                } else if code > prefix {
                    // 已超过 prefix 的字典序范围，可以提前终止
                    break;
                }
            }
        }
        results.sort_by_key(|e| Reverse(e.freq));
        results
    }

    /// 模糊查询：查找编辑距离在 max_edit_distance 以内的编码
    ///
    /// 策略：先用 FST 前缀匹配缩小候选集（所有可能的 1-edit 变体前缀），
    /// 再对候选词条的 code 部分做精确编辑距离过滤。
    /// 对于小词典直接全表扫描也很快。
    pub fn fuzzy_lookup(&self, code: &str, max_edit_distance: usize) -> Vec<(DictEntry, usize)> {
        // 全表扫描 + 编辑距离过滤（FST map 不适合做分离式 code 模糊搜索）
        let mut results = Vec::new();
        let mut stream = self.map.stream();
        while let Some((key, freq)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(code_end) = key_str.find(KEY_SEP) {
                let matched_code = &key_str[..code_end];
                let word = &key_str[code_end + 1..];
                let dist = crate::edit_distance(code, matched_code);
                if dist <= max_edit_distance {
                    results.push((
                        DictEntry {
                            code: matched_code.to_string(),
                            word: word.to_string(),
                            freq: freq as u32,
                            source: self.source,
                        },
                        dist,
                    ));
                }
            }
        }

        results.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.freq.cmp(&a.0.freq)));
        results
    }

    /// 获取词条总数
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// 获取编码数量（不同编码的个数）
    pub fn code_count(&self) -> usize {
        self.code_count
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
        let total_codes = self.code_count;
        let avg_words_per_code = if total_codes > 0 {
            total_entries as f64 / total_codes as f64
        } else {
            0.0
        };

        // 收集 top 10 高频词
        let mut all_words: Vec<(String, u32)> = Vec::new();
        let mut stream = self.map.stream();
        while let Some((key, freq)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(word) = key_str.split(KEY_SEP).nth(1) {
                all_words.push((word.to_string(), freq as u32));
            }
        }
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
        let mut all_entries = self.iter_entries();
        let other_entries = other.iter_entries();

        let mut seen: HashSet<(String, String)> = HashSet::new();
        all_entries.retain(|e| seen.insert((e.code.clone(), e.word.clone())));

        for entry in other_entries {
            if seen.insert((entry.code.clone(), entry.word.clone())) {
                all_entries.push(entry);
            }
        }

        *self = Self::from_entries(all_entries, self.source);
    }

    /// 内部：遍历所有词条（返回 owned 值）
    fn iter_entries(&self) -> Vec<DictEntry> {
        self.collect_entries()
    }

    /// 内部：收集所有词条
    fn collect_entries(&self) -> Vec<DictEntry> {
        let mut entries = Vec::new();
        let mut stream = self.map.stream();
        while let Some((key, freq)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(code_end) = key_str.find(KEY_SEP) {
                entries.push(DictEntry {
                    code: key_str[..code_end].to_string(),
                    word: key_str[code_end + 1..].to_string(),
                    freq: freq as u32,
                    source: self.source,
                });
            }
        }
        entries
    }

    /// 内部：统计不同 code 的数量
    fn count_codes(map: &fst::Map<Vec<u8>>) -> usize {
        let mut count = 0usize;
        let mut last_code = String::new();
        let mut stream = map.stream();
        while let Some((key, _)) = stream.next() {
            let key_str = String::from_utf8_lossy(key);
            if let Some(code_end) = key_str.find(KEY_SEP) {
                let code = &key_str[..code_end];
                if code != last_code {
                    count += 1;
                    last_code = code.to_string();
                }
            }
        }
        count
    }
}

/// 词典统计信息
#[derive(Debug, Clone, Default)]
pub struct DictStats {
    pub total_entries: usize,
    pub total_codes: usize,
    pub avg_words_per_code: f64,
    pub top_words: Vec<(String, u32)>,
}

/// 词典构建器
pub struct FstDictBuilder {
    entries: Vec<DictEntry>,
}

impl FstDictBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add_entry(&mut self, entry: DictEntry) {
        self.entries.push(entry);
    }

    pub fn add_entries(&mut self, entries: Vec<DictEntry>) {
        self.entries.extend(entries);
    }

    pub fn build(self, source: DictSource) -> FstDict {
        FstDict::from_entries(self.entries, source)
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

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
    let mut entries: Vec<DictEntry> = dict1.collect_entries();
    entries.extend(dict2.collect_entries());
    FstDict::from_entries(entries, dict1.source)
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
        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_fuzzy_lookup() {
        let entries = vec![
            make_entry("zhongguo", "中国", 1000),
            make_entry("zhongguo", "忠国", 10),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        let results = dict.fuzzy_lookup("zongguo", 1);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].1, 1);
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
        let entries2 = vec![make_entry("wo", "我", 100), make_entry("wo", "喔", 50)];
        let mut dict1 = FstDict::from_entries(entries1, DictSource::Base);
        let dict2 = FstDict::from_entries(entries2, DictSource::User);

        dict1.merge(&dict2);
        assert_eq!(dict1.entry_count(), 2);
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

    #[test]
    fn test_roundtrip_bytes() {
        let entries = vec![
            make_entry("wo", "我", 100),
            make_entry("ai", "爱", 200),
            make_entry("shijie", "世界", 300),
        ];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        let bytes = dict.to_bytes();
        assert!(!bytes.is_empty());

        let dict2 = FstDict::from_bytes(bytes).unwrap();
        assert_eq!(dict2.entry_count(), 3);
        assert_eq!(dict2.code_count(), 3);

        let r1 = dict.lookup("wo");
        let r2 = dict2.lookup("wo");
        assert_eq!(r1.len(), r2.len());
        assert_eq!(r1[0].word, r2[0].word);
    }

    #[test]
    fn test_roundtrip_file() {
        let entries = vec![make_entry("wo", "我", 100), make_entry("ai", "爱", 200)];
        let dict = FstDict::from_entries(entries, DictSource::Base);

        let dir = std::env::temp_dir().join("wbwime_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.fst");

        dict.write_to_file(&path).unwrap();
        let dict2 = FstDict::from_file(&path).unwrap();

        assert_eq!(dict2.entry_count(), 2);
        let results = dict2.lookup("wo");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].word, "我");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn test_cs_oi_cin_fst_consistency() {
        let cin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("resources")
            .join("dicts")
            .join("cs-oi.cin");
        if !cin_path.exists() {
            return;
        }

        let parser = crate::cin_parser::CinParser::new(cin_path.to_str().unwrap());
        let entries = parser.parse().unwrap();

        let dict_entries: Vec<DictEntry> = entries
            .iter()
            .flat_map(|cin_entry| {
                cin_entry.words.iter().map(move |w| DictEntry {
                    code: cin_entry.code.clone(),
                    word: w.word.clone(),
                    freq: w.freq,
                    source: DictSource::Base,
                })
            })
            .collect();

        let dict = FstDict::from_entries(dict_entries, DictSource::Base);

        let bytes = dict.to_bytes();
        let dict2 = FstDict::from_bytes(bytes).unwrap();

        assert_eq!(dict.entry_count(), dict2.entry_count());
        assert_eq!(dict.code_count(), dict2.code_count());

        let r1 = dict.lookup("zdl");
        let r2 = dict2.lookup("zdl");
        assert_eq!(r1.len(), r2.len());
    }
}
