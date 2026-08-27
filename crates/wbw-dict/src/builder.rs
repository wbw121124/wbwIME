//! 词典构建工具

use std::path::Path;
use thiserror::Error;
use wbw_types::ImeResult;

use crate::cin_parser::CinParser;
use crate::entry::{DictBuilderConfig, DictEntry, DictSource};
use crate::fst_dict::{FstDict, FstDictBuilder};

/// 构建错误类型
#[derive(Error, Debug)]
pub enum BuilderError {
    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("数据错误: {0}")]
    DataError(String),

    #[error("构建失败: {0}")]
    BuildError(String),
}

/// 词典构建器
pub struct DictBuilder {
    /// 构建配置
    config: DictBuilderConfig,
    /// 词条存储
    entries: Vec<DictEntry>,
}

impl DictBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: DictBuilderConfig::default(),
            entries: Vec::new(),
        }
    }

    /// 使用配置创建构建器
    pub fn with_config(config: DictBuilderConfig) -> Self {
        Self {
            config,
            entries: Vec::new(),
        }
    }

    /// 添加词条
    pub fn add_entry(&mut self, entry: DictEntry) {
        if entry.freq >= self.config.min_freq {
            if entry.word.len() <= self.config.max_word_len {
                self.entries.push(entry);
            }
        }
    }

    /// 批量添加词条
    pub fn add_entries(&mut self, entries: Vec<DictEntry>) {
        for entry in entries {
            self.add_entry(entry);
        }
    }

    /// 从 .cin 文件加载
    pub fn load_cin(&mut self, path: &Path) -> ImeResult<()> {
        let path_str = path.to_str().ok_or_else(|| {
            wbw_types::ImeError::ParseError("路径包含无效 Unicode".to_string())
        })?;
        let parser = CinParser::new(path_str);
        let cin_entries = parser.parse()?;

        for cin_entry in cin_entries {
            for word_entry in &cin_entry.words {
                self.add_entry(DictEntry {
                    code: cin_entry.code.clone(),
                    word: word_entry.word.clone(),
                    freq: word_entry.freq,
                    source: DictSource::Base,
                });
            }
        }
        Ok(())
    }

    /// 从字符串加载 .cin 内容
    pub fn load_cin_str(&mut self, content: &str) -> ImeResult<()> {
        let parser = CinParser::new("_");
        let cin_entries = parser.parse_str(content)?;

        for cin_entry in cin_entries {
            for word_entry in &cin_entry.words {
                self.add_entry(DictEntry {
                    code: cin_entry.code.clone(),
                    word: word_entry.word.clone(),
                    freq: word_entry.freq,
                    source: DictSource::Base,
                });
            }
        }
        Ok(())
    }

    /// 清理重复词条
    pub fn deduplicate(&mut self) {
        if self.config.deduplicate {
            use std::collections::HashSet;
            let mut seen = HashSet::new();
            self.entries
                .retain(|e| seen.insert((e.code.clone(), e.word.clone())));
        }
    }

    /// 排序词条（先按编码，再按词频降序）
    pub fn sort(&mut self) {
        if self.config.sort_entries {
            self.entries.sort_by(|a, b| {
                a.code
                    .cmp(&b.code)
                    .then_with(|| b.freq.cmp(&a.freq))
            });
        }
    }

    /// 构建 FST 词典
    pub fn build_fst(self) -> FstDict {
        let mut builder = FstDictBuilder::new();
        builder.add_entries(self.entries);
        builder.build(DictSource::Base)
    }

    /// 获取词条数量
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 清空词条
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 获取配置
    pub fn config(&self) -> &DictBuilderConfig {
        &self.config
    }
}

impl Default for DictBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// 词典验证工具
pub struct DictValidator;

impl DictValidator {
    /// 验证 .cin 文件格式
    pub fn validate_cin(path: &Path) -> ImeResult<()> {
        let path_str = path.to_str().ok_or_else(|| {
            wbw_types::ImeError::ParseError("路径包含无效 Unicode".to_string())
        })?;
        let parser = CinParser::new(path_str);
        parser.validate()
    }

    /// 验证词条有效性
    pub fn validate_entry(entry: &DictEntry) -> bool {
        !entry.code.is_empty() && !entry.word.is_empty()
    }

    /// 批量验证词条
    pub fn validate_entries(entries: &[DictEntry]) -> Vec<(usize, String)> {
        let mut errors = Vec::new();

        for (i, entry) in entries.iter().enumerate() {
            if !Self::validate_entry(entry) {
                errors.push((
                    i,
                    format!("无效词条: code={}, word={}", entry.code, entry.word),
                ));
            }
        }

        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dict_builder_load_cin_str() {
        let mut builder = DictBuilder::new();
        builder
            .load_cin_str("wo 我\nai 爱\nni 你\n")
            .unwrap();
        assert_eq!(builder.entry_count(), 3);
    }

    #[test]
    fn test_dict_builder_deduplicate() {
        let mut builder = DictBuilder::new();
        builder
            .load_cin_str("wo 我\nwo 我\nwo 喔\n")
            .unwrap();
        assert_eq!(builder.entry_count(), 3); // 去重前
        builder.deduplicate();
        assert_eq!(builder.entry_count(), 2); // 去重后：我、喔
    }

    #[test]
    fn test_dict_builder_build_fst() {
        let mut builder = DictBuilder::new();
        builder
            .load_cin_str("wo 我\nai 爱\n")
            .unwrap();
        let dict = builder.build_fst();
        assert_eq!(dict.entry_count(), 2);
        assert_eq!(dict.lookup("wo").len(), 1);
    }

    #[test]
    fn test_dict_validator_entry() {
        let valid = DictEntry {
            code: "wo".to_string(),
            word: "我".to_string(),
            freq: 100,
            source: DictSource::Base,
        };
        assert!(DictValidator::validate_entry(&valid));

        let invalid = DictEntry {
            code: "".to_string(),
            word: "我".to_string(),
            freq: 100,
            source: DictSource::Base,
        };
        assert!(!DictValidator::validate_entry(&invalid));
    }
}
