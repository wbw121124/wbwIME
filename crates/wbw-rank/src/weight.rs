//! 权重计算模块

use thiserror::Error;
use wbw_types::{Candidate, CandidateSource, ImeError, ImeResult, RankConfig};

/// 权重错误类型
#[derive(Error, Debug)]
pub enum WeightError {
    #[error("权重计算失败: {0}")]
    CalculationError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("数据不足: {0}")]
    InsufficientData(String),
}

/// 权重计算器
pub struct WeightCalculator {
    /// 配置
    config: RankConfig,
}

impl WeightCalculator {
    /// 创建新的权重计算器
    pub fn new(config: RankConfig) -> Self {
        Self { config }
    }

    /// 计算候选词权重
    pub fn calculate_weight(&self, candidate: &Candidate) -> f64 {
        let mut weight = 0.0;
        
        // 基础权重（拼音匹配）
        weight += self.config.pin_weight;
        
        // 用户权重
        if let Some(user_weight) = candidate.user_weight {
            weight += user_weight * self.config.user_weight;
        }
        
        // 词频权重
        weight += candidate.score * self.config.freq_weight;
        
        // N-gram 权重
        if let Some(ngram_score) = candidate.ngram_score {
            weight += ngram_score * self.config.ngram_weight;
        }
        
        // 来源加成
        weight *= self.source_multiplier(&candidate.source);
        
        weight
    }

    /// 获取来源乘数
    fn source_multiplier(&self, source: &CandidateSource) -> f64 {
        match source {
            CandidateSource::System => 1.0,
            CandidateSource::User => 1.2,
            CandidateSource::Dynamic => 1.1,
            CandidateSource::Phrase => 1.3,
        }
    }

    /// 批量计算权重
    pub fn calculate_weights(&self, candidates: &[Candidate]) -> Vec<(Candidate, f64)> {
        candidates
            .iter()
            .map(|c| {
                let weight = self.calculate_weight(c);
                (c.clone(), weight)
            })
            .collect()
    }

    /// 获取配置
    pub fn config(&self) -> &RankConfig {
        &self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: RankConfig) {
        self.config = config;
    }
}

/// 权重归一化器
pub struct WeightNormalizer;

impl WeightNormalizer {
    /// 最小-最大归一化
    pub fn min_max_normalize(weights: &mut [f64]) {
        if weights.is_empty() {
            return;
        }
        
        let min = weights.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = weights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let range = max - min;
        
        if range > 0.0 {
            for weight in weights.iter_mut() {
                *weight = (*weight - min) / range;
            }
        }
    }

    /// Z-score 归一化
    pub fn z_score_normalize(weights: &mut [f64]) {
        if weights.is_empty() {
            return;
        }
        
        let mean = weights.iter().sum::<f64>() / weights.len() as f64;
        let variance = weights.iter().map(|w| (w - mean).powi(2)).sum::<f64>() / weights.len() as f64;
        let std_dev = variance.sqrt();
        
        if std_dev > 0.0 {
            for weight in weights.iter_mut() {
                *weight = (*weight - mean) / std_dev;
            }
        }
    }

    /// L2 归一化
    pub fn l2_normalize(weights: &mut [f64]) {
        let norm: f64 = weights.iter().map(|w| w.powi(2)).sum::<f64>().sqrt();
        
        if norm > 0.0 {
            for weight in weights.iter_mut() {
                *weight /= norm;
            }
        }
    }

    /// softmax 归一化
    pub fn softmax_normalize(weights: &mut [f64]) {
        if weights.is_empty() {
            return;
        }
        
        let max_weight = weights.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_sum: f64 = weights.iter().map(|w| (w - max_weight).exp()).sum();
        
        for weight in weights.iter_mut() {
            *weight = (*weight - max_weight).exp() / exp_sum;
        }
    }
}

/// 权重调优器
pub struct WeightTuner;

impl WeightTuner {
    /// 网格搜索最优权重
    pub fn grid_search(
        candidates: &[Candidate],
        expected_order: &[usize],
        param_ranges: &WeightRanges,
    ) -> ImeResult<(RankConfig, f64)> {
        // TODO: 实现网格搜索
        todo!("实现权重网格搜索")
    }

    /// 随机搜索
    pub fn random_search(
        candidates: &[Candidate],
        expected_order: &[usize],
        iterations: usize,
    ) -> ImeResult<(RankConfig, f64)> {
        // TODO: 实现随机搜索
        todo!("实现权重随机搜索")
    }

    /// 模拟退火
    pub fn simulated_annealing(
        candidates: &[Candidate],
        expected_order: &[usize],
        initial_temp: f64,
        cooling_rate: f64,
        iterations: usize,
    ) -> ImeResult<(RankConfig, f64)> {
        // TODO: 实现模拟退火
        todo!("实现模拟退火优化")
    }
}

/// 权重搜索范围
#[derive(Debug, Clone)]
pub struct WeightRanges {
    /// 拼音权重范围
    pub pin_weight: (f64, f64),
    /// 用户权重范围
    pub user_weight: (f64, f64),
    /// 词频权重范围
    pub freq_weight: (f64, f64),
    /// N-gram 权重范围
    pub ngram_weight: (f64, f64),
}

impl Default for WeightRanges {
    fn default() -> Self {
        Self {
            pin_weight: (50.0, 150.0),
            user_weight: (5.0, 30.0),
            freq_weight: (0.5, 5.0),
            ngram_weight: (0.1, 3.0),
        }
    }
}

/// 权重评估结果
#[derive(Debug, Clone)]
pub struct WeightEvaluationResult {
    /// 配置
    pub config: RankConfig,
    /// 准确率
    pub accuracy: f64,
    /// 平均倒数排名（MRR）
    pub mrr: f64,
    /// 计算时间（毫秒）
    pub elapsed_ms: f64,
}