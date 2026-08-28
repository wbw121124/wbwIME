//! 错误类型定义

use std::fmt;
use thiserror::Error;
use wbw_types::ImeError;

/// 核心错误类型
#[derive(Error, Debug, Clone)]
pub enum CoreError {
    #[error("会话错误: {0}")]
    SessionError(String),
    
    #[error("候选词错误: {0}")]
    CandidateError(String),
    
    #[error("上下文错误: {0}")]
    ContextError(String),
    
    #[error("词典错误: {0}")]
    DictError(String),
    
    #[error("匹配错误: {0}")]
    MatchError(String),
    
    #[error("排序错误: {0}")]
    RankError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("IO 错误: {0}")]
    IoError(String),
    
    #[error("未知错误")]
    Unknown,
}

impl From<ImeError> for CoreError {
    fn from(err: ImeError) -> Self {
        match err {
            ImeError::DictLoadError(msg) => CoreError::DictError(msg),
            ImeError::ParseError(msg) => CoreError::DictError(msg),
            ImeError::MatchError(msg) => CoreError::MatchError(msg),
            ImeError::RankError(msg) => CoreError::RankError(msg),
            ImeError::ConfigError(msg) => CoreError::ConfigError(msg),
            ImeError::IoError(msg) => CoreError::IoError(msg),
            ImeError::NgramError(msg) => CoreError::DictError(msg),
            ImeError::Unknown => CoreError::Unknown,
        }
    }
}

/// 核心结果类型
pub type CoreResult<T> = Result<T, CoreError>;

/// 错误上下文
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// 错误消息
    pub message: String,
    /// 错误来源
    pub source: String,
    /// 错误代码
    pub code: Option<u32>,
    /// 建议操作
    pub suggestion: Option<String>,
}

impl ErrorContext {
    /// 创建新的错误上下文
    pub fn new(message: String, source: String) -> Self {
        Self {
            message,
            source,
            code: None,
            suggestion: None,
        }
    }

    /// 设置错误代码
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    /// 设置建议操作
    pub fn with_suggestion(mut self, suggestion: String) -> Self {
        self.suggestion = Some(suggestion);
        self
    }
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.source, self.message)?;
        if let Some(code) = self.code {
            write!(f, " (代码: {})", code)?;
        }
        if let Some(suggestion) = &self.suggestion {
            write!(f, " 建议: {}", suggestion)?;
        }
        Ok(())
    }
}

/// 错误恢复策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// 重试操作
    Retry,
    /// 回退到默认值
    Fallback,
    /// 忽略错误继续
    Ignore,
    /// 终止操作
    Abort,
}

/// 错误恢复器
pub struct ErrorRecovery;

impl ErrorRecovery {
    /// 尝试恢复错误
    pub fn try_recover<T, F>(error: &CoreError, strategy: RecoveryStrategy, fallback: F) -> CoreResult<T>
    where
        F: FnOnce() -> CoreResult<T>,
    {
        match strategy {
            RecoveryStrategy::Retry => {
                // 重试一次：调用 fallback() 进行重试（F 为 FnOnce，只能调用一次）
                fallback().or_else(|_| Err(error.clone()))
            }
            RecoveryStrategy::Fallback => fallback(),
            RecoveryStrategy::Ignore => {
                // 忽略错误并返回 fallback 默认值（泛型 T 无法显式构造，故调用 fallback() 作为折中）
                fallback()
            }
            RecoveryStrategy::Abort => Err(error.clone()),
        }
    }

    /// 记录错误日志
    pub fn log_error(error: &CoreError, context: Option<&ErrorContext>) {
        match context {
            Some(ctx) => eprintln!("[错误日志] {} 上下文: {}", error, ctx),
            None => eprintln!("[错误日志] {}", error),
        }
    }

    /// 发送错误报告
    pub fn report_error(error: &CoreError, context: Option<&ErrorContext>) {
        match context {
            Some(ctx) => eprintln!("错误报告: {}\n  上下文: {}", error, ctx),
            None => eprintln!("错误报告: {}", error),
        }
    }
}