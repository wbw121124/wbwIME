//! .cin 码表解析器

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;
use wbw_types::{ImeResult, WordEntry};

use crate::entry::CinEntry;

/// 解析错误类型
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("文件读取失败: {0}")]
    FileError(String),

    #[error("编码格式错误: {0}")]
    EncodingError(String),

    #[error("码表格式错误: 第{line}行: {message}")]
    FormatError { line: usize, message: String },
}

impl From<ParseError> for wbw_types::ImeError {
    fn from(e: ParseError) -> Self {
        wbw_types::ImeError::ParseError(e.to_string())
    }
}

/// .cin 解析器
pub struct CinParser {
    /// 文件路径
    path: String,
    /// 是否跳过注释行
    skip_comments: bool,
    /// 注释前缀
    comment_prefix: String,
    /// 是否跳过空白行
    skip_empty: bool,
    /// 最大编码长度
    max_code_len: usize,
}

impl CinParser {
    /// 创建新的解析器
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            skip_comments: true,
            comment_prefix: "%".to_string(),
            skip_empty: true,
            max_code_len: 32,
        }
    }

    /// 设置是否跳过注释行
    pub fn with_skip_comments(mut self, skip: bool) -> Self {
        self.skip_comments = skip;
        self
    }

    /// 设置注释前缀
    pub fn with_comment_prefix(mut self, prefix: &str) -> Self {
        self.comment_prefix = prefix.to_string();
        self
    }

    /// 设置最大编码长度
    pub fn with_max_code_len(mut self, max: usize) -> Self {
        self.max_code_len = max;
        self
    }

    /// 解析码表文件
    pub fn parse(&self) -> ImeResult<Vec<CinEntry>> {
        let content = fs::read_to_string(&self.path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("读取文件失败 {}: {}", self.path, e)))?;
        self.parse_str(&content)
    }

    /// 从字符串解析
    pub fn parse_str(&self, content: &str) -> ImeResult<Vec<CinEntry>> {
        let mut map: HashMap<String, CinEntry> = HashMap::new();

        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num + 1; // 行号从 1 开始
            let trimmed = line.trim();

            // 跳过空行
            if self.skip_empty && trimmed.is_empty() {
                continue;
            }

            // 跳过注释行
            if self.skip_comments && trimmed.starts_with(&self.comment_prefix) {
                continue;
            }

            // 解析行：格式为 "编码 汉字" 或 "编码 汉字 词频"
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(wbw_types::ImeError::ParseError(format!(
                    "第{}行格式错误: 需要至少'编码 汉字'两个字段",
                    line_num
                )));
            }

            let code = parts[0];

            // 验证编码长度
            if code.len() > self.max_code_len {
                return Err(wbw_types::ImeError::ParseError(format!(
                    "第{}行编码超长: '{}' 长度{}超过限制{}",
                    line_num,
                    code,
                    code.len(),
                    self.max_code_len
                )));
            }

            let word_text = parts[1];

            // 解析词频（可选第三字段）
            let freq = if parts.len() >= 3 {
                parts[2].parse::<u32>().unwrap_or(0)
            } else {
                0
            };

            let entry = CinEntry::new(code.to_string());
            map.entry(code.to_string())
                .or_insert(entry)
                .add_word(WordEntry {
                    word: word_text.to_string(),
                    freq,
                    pos: None,
                });
        }

        let mut result: Vec<CinEntry> = map.into_values().collect();
        // 按编码排序
        result.sort_by(|a, b| a.code.cmp(&b.code));
        Ok(result)
    }

    /// 验证码表格式（不实际解析，只检查格式）
    pub fn validate(&self) -> ImeResult<()> {
        let content = fs::read_to_string(&self.path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("读取文件失败 {}: {}", self.path, e)))?;

        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num + 1;
            let trimmed = line.trim();

            if self.skip_empty && trimmed.is_empty() {
                continue;
            }
            if self.skip_comments && trimmed.starts_with(&self.comment_prefix) {
                continue;
            }

            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(wbw_types::ImeError::ParseError(format!(
                    "第{}行格式错误: 需要至少'编码 汉字'两个字段",
                    line_num
                )));
            }
        }
        Ok(())
    }

    /// 获取文件路径
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// 批量解析多个 .cin 文件
pub fn parse_multiple(paths: &[&str]) -> ImeResult<Vec<CinEntry>> {
    let mut all_entries = Vec::new();

    for path in paths {
        let parser = CinParser::new(path);
        let entries = parser.parse()?;
        all_entries.extend(entries);
    }

    Ok(all_entries)
}

/// 合并多个码表条目（按编码分组）
pub fn merge_entries(entries: Vec<CinEntry>) -> Vec<CinEntry> {
    let mut map: HashMap<String, CinEntry> = HashMap::new();

    for entry in entries {
        let code = entry.code.clone();
        map.entry(code)
            .and_modify(|e| e.words.extend(entry.words.clone()))
            .or_insert(entry);
    }

    let mut result: Vec<CinEntry> = map.into_values().collect();
    result.sort_by(|a, b| a.code.cmp(&b.code));
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let content = "% 注释行\nwo 我\nai 爱\nni 你\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].code, "ai");
        assert_eq!(entries[0].words[0].word, "爱");
    }

    #[test]
    fn test_parse_with_freq() {
        let content = "wo 我 1000\nai 爱 800\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].words[0].freq, 800);
    }

    #[test]
    fn test_merge_entries() {
        let content1 = "wo 我\nwo 喔\n";
        let content2 = "wo 涡\nai 爱\n";
        let parser = CinParser::new("_");
        let mut entries = parser.parse_str(content1).unwrap();
        entries.extend(parser.parse_str(content2).unwrap());
        let merged = merge_entries(entries);
        assert_eq!(merged.len(), 2); // wo 和 ai
        let wo_entry = merged.iter().find(|e| e.code == "wo").unwrap();
        assert_eq!(wo_entry.words.len(), 3); // 我、喔、涡
    }

    #[test]
    fn test_skip_comments() {
        let content = "% 注释\nwo 我\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_invalid_line() {
        let content = "wo\n";
        let parser = CinParser::new("_");
        let result = parser.parse_str(content);
        assert!(result.is_err());
    }
}
