//! .cin 码表解析器

use std::path::Path;
use thiserror::Error;
use wbw_types::{ImeError, ImeResult};

use crate::entry::CinEntry;

/// 解析错误类型
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("文件读取失败: {0}")]
    FileError(String),
    
    #[error("编码格式错误: {0}")]
    EncodingError(String),
    
    #[error("码表格式错误: {0}")]
    FormatError(String),
    
    #[error("词条解析失败: {0}")]
    EntryError(String),
}

/// .cin 解析器
pub struct CinParser {
    /// 文件路径
    path: String,
    /// 编码格式
    encoding: String,
    /// 是否跳过注释行
    skip_comments: bool,
    /// 注释前缀
    comment_prefix: String,
}

impl CinParser {
    /// 创建新的解析器
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            encoding: "utf-8".to_string(),
            skip_comments: true,
            comment_prefix: "%".to_string(),
        }
    }

    /// 设置编码格式
    pub fn with_encoding(mut self, encoding: &str) -> Self {
        self.encoding = encoding.to_string();
        self
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

    /// 解析码表文件
    pub fn parse(&self) -> ImeResult<Vec<CinEntry>> {
        // TODO: 实现解析逻辑
        todo!("实现 .cin 码表解析逻辑")
    }

    /// 从字符串解析
    pub fn parse_str(&self, content: &str) -> ImeResult<Vec<CinEntry>> {
        // TODO: 实现字符串解析逻辑
        todo!("实现字符串解析逻辑")
    }

    /// 验证码表格式
    pub fn validate(&self) -> ImeResult<()> {
        // TODO: 实现验证逻辑
        todo!("实现码表格式验证逻辑")
    }

    /// 获取文件路径
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 获取编码格式
    pub fn encoding(&self) -> &str {
        &self.encoding
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
    use std::collections::HashMap;
    
    let mut map: HashMap<String, CinEntry> = HashMap::new();
    
    for entry in entries {
        let code = entry.code.clone();
        map.entry(code)
            .and_modify(|e| e.words.extend(entry.words.clone()))
            .or_insert(entry);
    }
    
    map.into_values().collect()
}