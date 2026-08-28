//! .cin 码表解析器

use std::collections::HashMap;
use std::fs;
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

/// .cin 解析结果
pub struct CinParseResult {
    /// 码表条目
    pub entries: Vec<CinEntry>,
    /// 模糊规则
    pub fuzzy_rules: Vec<CinFuzzyRule>,
}

/// 从 .cin 文件解析的模糊规则
#[derive(Debug, Clone)]
pub struct CinFuzzyRule {
    /// 规则名称
    pub name: String,
    /// 源字符/音素
    pub from: String,
    /// 目标字符/音素
    pub to: String,
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

    /// 解析码表文件（仅返回条目，忽略模糊规则）
    pub fn parse(&self) -> ImeResult<Vec<CinEntry>> {
        let result = self.parse_full()?;
        Ok(result.entries)
    }

    /// 解析码表文件（返回完整结果，包含模糊规则）
    pub fn parse_full(&self) -> ImeResult<CinParseResult> {
        let content = fs::read_to_string(&self.path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("读取文件失败 {}: {}", self.path, e)))?;
        self.parse_str_full(&content)
    }

    /// 从字符串解析（仅返回条目，忽略模糊规则）
    pub fn parse_str(&self, content: &str) -> ImeResult<Vec<CinEntry>> {
        let result = self.parse_str_full(content)?;
        Ok(result.entries)
    }

    /// 从字符串解析（返回完整结果，包含模糊规则）
    pub fn parse_str_full(&self, content: &str) -> ImeResult<CinParseResult> {
        let mut map: HashMap<String, CinEntry> = HashMap::new();
        let mut fuzzy_rules = Vec::new();
        let mut in_keyname_section = false;
        let mut in_fuzzy_section = false;

        for (line_num, line) in content.lines().enumerate() {
            let line_num = line_num + 1; // 行号从 1 开始
            let trimmed = line.trim();

            // 跳过空行
            if self.skip_empty && trimmed.is_empty() {
                continue;
            }

            // 处理特殊段落标记
            if trimmed.starts_with("%keyname begin") {
                in_keyname_section = true;
                continue;
            }
            if trimmed.starts_with("%keyname end") {
                in_keyname_section = false;
                continue;
            }

            // 处理模糊规则段落
            if trimmed.starts_with("%fuzzy begin") {
                in_fuzzy_section = true;
                continue;
            }
            if trimmed.starts_with("%fuzzy end") {
                in_fuzzy_section = false;
                continue;
            }

            // 跳过 keyname 段落中的条目
            if in_keyname_section {
                continue;
            }

            // 在 fuzzy 段落中解析模糊规则
            if in_fuzzy_section {
                if self.skip_comments && trimmed.starts_with(&self.comment_prefix) {
                    continue;
                }
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() == 3 {
                    fuzzy_rules.push(CinFuzzyRule {
                        name: parts[0].to_string(),
                        from: parts[1].to_string(),
                        to: parts[2].to_string(),
                    });
                }
                continue;
            }

            // 跳过注释行（% 开头的其他行）
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

            // 解析词与词频。
            // 词可能包含空格（如英文短语 "Floyd 算法"）。
            // 规则（编码为第 1 个字段，其余为词）：
            //   - 末尾字段为 "_"    => 词频未设置占位，剥离该字段，剩余部分为词
            //   - 末尾字段为纯数字  => 作为词频，剥离该字段，剩余部分为词
            //   - 否则              => 全部字段拼成词
            // 需要注意：只有第 3 个及以后的字段才可能是词频，避免把纯数字词误解为词频。
            let n = parts.len();
            let last = parts[n - 1];
            let (word_text, freq) = if n >= 3 && last == "_" {
                (parts[1..n - 1].join(" "), 0)
            } else if n >= 3 {
                match last.parse::<u32>() {
                    Ok(f) => (parts[1..n - 1].join(" "), f),
                    Err(_) => (parts[1..].join(" "), 0),
                }
            } else {
                (parts[1..].join(" "), 0)
            };

            let entry = CinEntry::new(code.to_string());
            map.entry(code.to_string())
                .or_insert(entry)
                .add_word(WordEntry {
                    word: word_text,
                    freq,
                    pos: None,
                });
        }

        let mut entries: Vec<CinEntry> = map.into_values().collect();
        // 按编码排序
        entries.sort_by(|a, b| a.code.cmp(&b.code));

        Ok(CinParseResult {
            entries,
            fuzzy_rules,
        })
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

    #[test]
    fn test_parse_cin_with_keyname_section() {
        // cs-oi.cin 格式：包含 %keyname begin/end 和 %chardef begin/end
        let content = "%gen_inp\n%ename Test\n%keyname begin\na a\nb b\n%keyname end\n%chardef begin\nmn 模拟\nmh 枚举\nbl 暴力\n%chardef end\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].code, "bl");
        assert_eq!(entries[0].words[0].word, "暴力");
        assert_eq!(entries[1].code, "mh");
        assert_eq!(entries[1].words[0].word, "枚举");
        assert_eq!(entries[2].code, "mn");
        assert_eq!(entries[2].words[0].word, "模拟");
    }

    #[test]
    fn test_parse_cin_multi_word_entries() {
        // 同一编码对应多个候选词
        let content = "bcj 并查集\nbcj Disjoint Set Union\nbcj DSU\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, "bcj");
        assert_eq!(entries[0].words.len(), 3);
    }

    #[test]
    fn test_parse_word_with_spaces() {
        // 词包含空格（英文短语），且最后一个字段不是数字时应整体保留
        let content = "zdl Floyd 算法\nzdl Dijkstra 算法\nzdl SPFA\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].code, "zdl");
        assert_eq!(entries[0].words.len(), 3);
        // 含空格词应完整保留，不能只取第二个字段
        let floyd = entries[0].words.iter().find(|w| w.word == "Floyd 算法");
        assert!(floyd.is_some(), "应保留完整词 'Floyd 算法'");
        let spfa = entries[0].words.iter().find(|w| w.word == "SPFA");
        assert!(spfa.is_some(), "应保留单词 'SPFA'");
    }

    #[test]
    fn test_parse_word_with_spaces_and_freq() {
        // 词含空格且带词频字段（末尾数字），应正确切分
        let content = "zdl Floyd 算法 5\nwo 我 1000\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        let zdl = entries.iter().find(|e| e.code == "zdl").unwrap();
        assert_eq!(zdl.words[0].word, "Floyd 算法");
        assert_eq!(zdl.words[0].freq, 5);
        let wo = entries.iter().find(|e| e.code == "wo").unwrap();
        assert_eq!(wo.words[0].word, "我");
        assert_eq!(wo.words[0].freq, 1000);
    }

    #[test]
    fn test_parse_freq_placeholder_rules() {
        // code top 10         => 词 = "top",      词频 = 10
        // code top 10 _       => 词 = "top 10",   词频 = 未设置(0)
        // code top 10 _ _     => 词 = "top 10 _", 词频 = 未设置(0)
        let content = "co top 10\nco top 10 _\nco top 10 _ _\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 1);
        let e = &entries[0];
        assert_eq!(e.code, "co");
        assert_eq!(e.words.len(), 3);
        assert_eq!(e.words[0].word, "top");
        assert_eq!(e.words[0].freq, 10);
        assert_eq!(e.words[1].word, "top 10");
        assert_eq!(e.words[1].freq, 0);
        assert_eq!(e.words[2].word, "top 10 _");
        assert_eq!(e.words[2].freq, 0);
    }

    #[test]
    fn test_parse_cin_abbreviation_codes() {
        // 缩写编码（非拼音）
        let content = "kspx 快速排序\nmbpx 冒泡排序\ncrpx 插入排序\n";
        let parser = CinParser::new("_");
        let entries = parser.parse_str(content).unwrap();
        assert_eq!(entries.len(), 3);
        let codes: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
        assert!(codes.contains(&"kspx"));
        assert!(codes.contains(&"mbpx"));
    }

    #[test]
    fn test_parse_cs_oi_cin_file() {
        // 测试真实 cs-oi.cin 文件解析
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("resources")
            .join("dicts")
            .join("cs-oi.cin");
        if path.exists() {
            let parser = CinParser::new(path.to_str().unwrap());
            let entries = parser.parse().unwrap();
            assert!(entries.len() > 100, "cs-oi.cin 应包含大量条目");
            // 验证特定条目存在
            let mn_entry = entries.iter().find(|e| e.code == "mn");
            assert!(mn_entry.is_some(), "应包含 'mn 模拟' 条目");
            assert_eq!(mn_entry.unwrap().words[0].word, "模拟");
        }
    }

    #[test]
    fn test_parse_fuzzy_section() {
        let content = "%fuzzy begin\nz-zh z zh\nc-ch c ch\ns-sh s sh\nn-l n l\n%fuzzy end\nwo 我\n";
        let parser = CinParser::new("_");
        let result = parser.parse_str_full(content).unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.fuzzy_rules.len(), 4);
        assert_eq!(result.fuzzy_rules[0].name, "z-zh");
        assert_eq!(result.fuzzy_rules[0].from, "z");
        assert_eq!(result.fuzzy_rules[0].to, "zh");
    }

    #[test]
    fn test_parse_fuzzy_section_with_comments() {
        let content = "%fuzzy begin\n% 这是注释\nz-zh z zh\n% 另一行注释\n%fuzzy end\n";
        let parser = CinParser::new("_");
        let result = parser.parse_str_full(content).unwrap();
        assert_eq!(result.entries.len(), 0);
        assert_eq!(result.fuzzy_rules.len(), 1);
    }

    #[test]
    fn test_parse_empty_fuzzy_section() {
        let content = "%fuzzy begin\n%fuzzy end\nwo 我\n";
        let parser = CinParser::new("_");
        let result = parser.parse_str_full(content).unwrap();
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.fuzzy_rules.len(), 0);
    }
}
