//! N-gram 评分器模块

use std::path::Path;
use thiserror::Error;
use wbw_types::{ImeError, ImeResult, NgramConfig};
use crate::table::NgramTable;
use crate::smooth::{SmoothConfig, Smoother};

/// 评分器错误类型
#[derive(Error, Debug)]
pub enum ScorerError {
    #[error("评分器初始化失败: {0}")]
    InitError(String),
    
    #[error("评分计算失败: {0}")]
    ComputationError(String),
    
    #[error("数据不足: {0}")]
    InsufficientData(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// 评分器配置
#[derive(Debug, Clone)]
pub struct ScorerConfig {
    /// N-gram 配置
    pub ngram: NgramConfig,
    /// 平滑配置
    pub smooth: SmoothConfig,
    /// 是否启用长度归一化
    pub normalize_length: bool,
    /// 是否启用对数概率
    pub use_log_prob: bool,
    /// 最小概率阈值
    pub min_prob: f64,
    /// 最大概率阈值
    pub max_prob: f64,
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self {
            ngram: NgramConfig::default(),
            smooth: SmoothConfig::default(),
            normalize_length: true,
            use_log_prob: true,
            min_prob: 1e-10,
            max_prob: 1.0,
        }
    }
}

/// N-gram 评分器
pub struct NgramScorer {
    /// 配置
    config: ScorerConfig,
    /// 概率表
    table: Option<NgramTable>,
    /// 平滑处理器
    smoother: Smoother,
}

impl NgramScorer {
    /// 创建新的评分器
    pub fn new(config: ScorerConfig) -> Self {
        let smoother = Smoother::new(config.smooth.clone());
        Self {
            config,
            table: None,
            smoother,
        }
    }

    /// 从文件加载
    pub fn from_file(config: ScorerConfig, path: &Path) -> ImeResult<Self> {
        // TODO: 实现文件加载逻辑
        todo!("实现评分器文件加载")
    }

    /// 设置概率表
    pub fn with_table(mut self, table: NgramTable) -> Self {
        self.table = Some(table);
        self
    }

    /// 评分单个词
    pub fn score_word(&self, context: &[&str], word: &str) -> ImeResult<f64> {
        // TODO: 实现单个词评分
        todo!("实现单个词评分")
    }

    /// 评分序列
    pub fn score_sequence(&self, words: &[&str]) -> ImeResult<f64> {
        // TODO: 实现序列评分
        todo!("实现序列评分")
    }

    /// 计算条件概率
    pub fn conditional_probability(&self, context: &[&str], word: &str) -> ImeResult<f64> {
        // TODO: 实现条件概率计算
        todo!("实现条件概率计算")
    }

    /// 计算困惑度
    pub fn perplexity(&self, test_data: &[&str]) -> ImeResult<f64> {
        // TODO: 实现困惑度计算
        todo!("实现困惑度计算")
    }

    /// 获取配置
    pub fn config(&self) -> &ScorerConfig {
        &self.config
    }

    /// 检查是否已加载表
    pub fn has_table(&self) -> bool {
        self.table.is_some()
    }

    /// 获取表引用
    pub fn table(&self) -> Option<&NgramTable> {
        self.table.as_ref()
    }
}

/// 批量评分结果
#[derive(Debug, Clone)]
pub struct BatchScoreResult {
    /// 评分结果
    pub scores: Vec<(String, f64)>,
    /// 总耗时（毫秒）
    pub elapsed_ms: f64,
    /// 平均耗时（毫秒）
    pub avg_time_ms: f64,
}

/// 评分器构建器
pub struct ScorerBuilder {
    config: ScorerConfig,
}

impl ScorerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: ScorerConfig::default(),
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: ScorerConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置 N-gram 阶数
    pub fn with_order(mut self, order: usize) -> Self {
        self.config.ngram.order = order;
        self
    }

    /// 设置平滑方法
    pub fn with_smooth_method(mut self, method: crate::smooth::SmoothMethod) -> Self {
        self.config.smooth.method = method;
        self
    }

    /// 设置平滑参数
    pub fn with_smooth_parameter(mut self, param: f64) -> Self {
        self.config.smooth.parameter = param;
        self
    }

    /// 启用长度归一化
    pub fn with_normalize_length(mut self, enable: bool) -> Self {
        self.config.normalize_length = enable;
        self
    }

    /// 构建评分器
    pub fn build(self) -> NgramScorer {
        NgramScorer::new(self.config)
    }
}

impl Default for ScorerBuilder {
    fn default() -> Self {
        Self::new()
    }
}