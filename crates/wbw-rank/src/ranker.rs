//! 排序器主体模块

use std::num::NonZeroUsize;
use thiserror::Error;
use wbw_types::{Candidate, ImeResult, RankConfig};
use crate::config::RankConfigManager;
use crate::weight::{WeightCalculator, WeightNormalizer};
use crate::l0_learn::L0Learner;

/// 排序错误类型
#[derive(Error, Debug)]
pub enum RankerError {
    #[error("排序计算失败: {0}")]
    CalculationError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("数据不足: {0}")]
    InsufficientData(String),
    
    #[error("学习错误: {0}")]
    LearningError(String),
}

/// 排序器
pub struct Ranker {
    /// 配置管理器
    config_manager: RankConfigManager,
    /// 权重计算器
    weight_calculator: WeightCalculator,
    /// L0 学习器
    l0_learner: L0Learner,
    /// 排序缓存
    cache: Option<lru::LruCache<String, Vec<Candidate>>>,
}

impl Ranker {
    /// 创建新的排序器
    pub fn new(config: RankConfig) -> Self {
        let weight_calculator = WeightCalculator::new(config.clone());
        let l0_config = wbw_types::L0Config::default();
        let l0_learner = L0Learner::new(l0_config);
        
        Self {
            config_manager: RankConfigManager::from_memory(config),
            weight_calculator,
            l0_learner,
            cache: Some(lru::LruCache::new(NonZeroUsize::new(1000).unwrap())),
        }
    }

    /// 从配置管理器创建
    pub fn from_config_manager(config_manager: RankConfigManager) -> Self {
        let config = config_manager.config().clone();
        let weight_calculator = WeightCalculator::new(config.clone());
        let l0_config = wbw_types::L0Config::default();
        let l0_learner = L0Learner::new(l0_config);
        
        Self {
            config_manager,
            weight_calculator,
            l0_learner,
            cache: Some(lru::LruCache::new(NonZeroUsize::new(1000).unwrap())),
        }
    }

    /// 排序候选词
    pub fn rank(&self, candidates: Vec<Candidate>) -> ImeResult<Vec<Candidate>> {
        // TODO: 实现排序逻辑
        todo!("实现候选词排序")
    }

    /// 带上下文的排序
    pub fn rank_with_context(
        &self,
        candidates: Vec<Candidate>,
        context: &str,
    ) -> ImeResult<Vec<Candidate>> {
        // TODO: 实现带上下文的排序
        todo!("实现带上下文的排序")
    }

    /// 记录用户选择（用于 L0 学习）
    pub fn record_selection(&mut self, code: &str, word: &str) {
        self.l0_learner.record_selection(code, word);
    }

    /// 获取排序建议
    pub fn get_suggestions(&self) -> Vec<wbw_types::Candidate> {
        // TODO: 实现排序建议获取
        todo!("获取排序建议")
    }

    /// 获取配置
    pub fn config(&self) -> &RankConfig {
        self.config_manager.config()
    }

    /// 获取配置管理器
    pub fn config_manager(&self) -> &RankConfigManager {
        &self.config_manager
    }

    /// 获取可变配置管理器
    pub fn config_manager_mut(&mut self) -> &mut RankConfigManager {
        &mut self.config_manager
    }

    /// 获取 L0 学习器
    pub fn l0_learner(&self) -> &L0Learner {
        &self.l0_learner
    }

    /// 获取可变 L0 学习器
    pub fn l0_learner_mut(&mut self) -> &mut L0Learner {
        &mut self.l0_learner
    }

    /// 清除缓存
    pub fn clear_cache(&mut self) {
        if let Some(cache) = &mut self.cache {
            cache.clear();
        }
    }

    /// 获取缓存命中率
    pub fn cache_hit_rate(&self) -> f64 {
        // TODO: 实现缓存统计
        todo!("获取缓存命中率")
    }
}

/// 排序策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankStrategy {
    /// 加权排序
    Weighted,
    /// 基于学习的排序
    LearningBased,
    /// 混合排序
    Hybrid,
    /// 个性化排序
    Personalized,
}

/// 排序结果
#[derive(Debug, Clone)]
pub struct RankResult {
    /// 排序后的候选词
    pub candidates: Vec<Candidate>,
    /// 排序耗时（毫秒）
    pub elapsed_ms: f64,
    /// 使用的策略
    pub strategy: RankStrategy,
    /// 排序统计
    pub stats: RankStats,
}

/// 排序统计
#[derive(Debug, Clone, Default)]
pub struct RankStats {
    /// 总候选词数
    pub total_candidates: usize,
    /// 排序后候选词数
    pub ranked_candidates: usize,
    /// 平均权重
    pub avg_weight: f64,
    /// 最大权重
    pub max_weight: f64,
    /// 最小权重
    pub min_weight: f64,
    /// 缓存命中次数
    pub cache_hits: usize,
    /// 缓存未命中次数
    pub cache_misses: usize,
}

/// 排序器构建器
pub struct RankerBuilder {
    config: RankConfig,
    cache_size: Option<usize>,
    l0_config: Option<wbw_types::L0Config>,
}

impl RankerBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            config: RankConfig::default(),
            cache_size: Some(1000),
            l0_config: None,
        }
    }

    /// 设置配置
    pub fn with_config(mut self, config: RankConfig) -> Self {
        self.config = config;
        self
    }

    /// 设置缓存大小
    pub fn with_cache_size(mut self, size: usize) -> Self {
        self.cache_size = Some(size);
        self
    }

    /// 设置 L0 配置
    pub fn with_l0_config(mut self, config: wbw_types::L0Config) -> Self {
        self.l0_config = Some(config);
        self
    }

    /// 构建排序器
    pub fn build(self) -> Ranker {
        let mut ranker = Ranker::new(self.config);
        
        if let Some(cache_size) = self.cache_size {
            ranker.cache = Some(lru::LruCache::new(NonZeroUsize::new(cache_size).unwrap()));
        }
        
        if let Some(l0_config) = self.l0_config {
            ranker.l0_learner = L0Learner::new(l0_config);
        }
        
        ranker
    }
}

impl Default for RankerBuilder {
    fn default() -> Self {
        Self::new()
    }
}