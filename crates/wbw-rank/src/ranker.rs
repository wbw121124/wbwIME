//! 排序器主体模块
//!
//! 提供候选词排序功能，支持加权排序、L0 动态学习。

use crate::config::RankConfigManager;
use crate::l0_learn::L0Learner;
use crate::weight::WeightCalculator;
use std::time::Instant;
use wbw_types::{Candidate, L0Config, RankConfig};

/// 排序器
pub struct Ranker {
    config_manager: RankConfigManager,
    weight_calculator: WeightCalculator,
    l0_learner: L0Learner,
}

impl Ranker {
    /// 创建新的排序器
    pub fn new(config: RankConfig) -> Self {
        let weight_calculator = WeightCalculator::new(config.clone());
        let l0_config = L0Config::default();
        let l0_learner = L0Learner::new(l0_config);

        Self {
            config_manager: RankConfigManager::from_memory(config),
            weight_calculator,
            l0_learner,
        }
    }

    /// 从配置管理器创建
    pub fn from_config_manager(config_manager: RankConfigManager) -> Self {
        let config = config_manager.config().clone();
        let weight_calculator = WeightCalculator::new(config.clone());
        let l0_config = L0Config::default();
        let l0_learner = L0Learner::new(l0_config);

        Self {
            config_manager,
            weight_calculator,
            l0_learner,
        }
    }

    /// 排序候选词
    ///
    /// 按权重降序排列候选词。
    pub fn rank(&self, candidates: &[Candidate]) -> Vec<Candidate> {
        let start = Instant::now();

        // 计算权重
        let mut weighted: Vec<(Candidate, f64)> = candidates
            .iter()
            .cloned()
            .map(|c| {
                let weight = self.weight_calculator.calculate_weight(&c);
                (c, weight)
            })
            .collect();

        // 按权重降序排序
        weighted.sort_by(|a, b| b.1.total_cmp(&a.1));

        // 提取排序后的候选词
        let result: Vec<Candidate> = weighted.into_iter().map(|(c, _)| c).collect();

        let _elapsed = start.elapsed().as_millis();
        result
    }

    /// 带上下文的排序
    ///
    /// 考虑上下文调整候选词权重。
    pub fn rank_with_context(&self, candidates: &[Candidate], context: &str) -> Vec<Candidate> {
        let mut ranked = self.rank(candidates);

        // 如果有上下文，根据上下文调整权重
        // 简单实现：检查候选词是否与上下文相关
        if !context.is_empty() {
            ranked.sort_by(|a, b| {
                let a_relevance = self.context_relevance(&a.text, context);
                let b_relevance = self.context_relevance(&b.text, context);
                b_relevance.total_cmp(&a_relevance)
            });
        }

        ranked
    }

    /// 计算候选词与上下文的相关性
    fn context_relevance(&self, word: &str, context: &str) -> f64 {
        // 简单实现：如果候选词出现在上下文中，相关性为 1.0
        if context.contains(word) {
            1.0
        } else {
            0.0
        }
    }

    /// 记录用户选择（用于 L0 学习）
    pub fn record_selection(&mut self, code: &str, word: &str) {
        self.l0_learner.record_selection(code, word);
    }

    /// 获取 L0 学习建议
    pub fn get_l0_suggestions(&self) -> Vec<(String, String, u32)> {
        self.l0_learner.get_top_suggestions(10)
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
}

/// 排序策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankStrategy {
    Weighted,
    LearningBased,
    Hybrid,
}

/// 排序结果
#[derive(Debug, Clone)]
pub struct RankResult {
    pub candidates: Vec<Candidate>,
    pub elapsed_ms: f64,
    pub strategy: RankStrategy,
}

/// 排序器构建器
pub struct RankerBuilder {
    config: RankConfig,
}

impl RankerBuilder {
    pub fn new() -> Self {
        Self {
            config: RankConfig::default(),
        }
    }

    pub fn with_config(mut self, config: RankConfig) -> Self {
        self.config = config;
        self
    }

    pub fn build(self) -> Ranker {
        Ranker::new(self.config)
    }
}

impl Default for RankerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wbw_types::CandidateSource;

    fn test_candidates() -> Vec<Candidate> {
        vec![
            Candidate {
                text: "中国".into(),
                code: "zhongguo".into(),
                score: 100.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
            Candidate {
                text: "终于".into(),
                code: "zhongyu".into(),
                score: 50.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
        ]
    }

    #[test]
    fn test_rank() {
        let ranker = Ranker::new(RankConfig::default());
        let candidates = test_candidates();
        let ranked = ranker.rank(&candidates);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn test_rank_with_context() {
        let ranker = Ranker::new(RankConfig::default());
        let candidates = test_candidates();
        let ranked = ranker.rank_with_context(&candidates, "中国");
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn test_record_selection() {
        let mut ranker = Ranker::new(RankConfig::default());
        ranker.record_selection("zhongguo", "中国");
        assert_eq!(ranker.l0_learner().data_count(), 1);
    }

    #[test]
    fn test_builder() {
        let ranker = RankerBuilder::new().build();
        assert_eq!(ranker.config_manager().config().max_candidates, 10);
    }
}
