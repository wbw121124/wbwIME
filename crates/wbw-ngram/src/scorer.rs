//! N-gram 评分器模块
//!
//! 提供基于 N-gram 的候选词评分功能。

use crate::smooth::SmoothConfig;
use crate::table::NgramTable;

/// 评分器配置
#[derive(Debug, Clone)]
pub struct ScorerConfig {
    /// 平滑配置
    pub smooth: SmoothConfig,
    /// 是否启用对数概率
    pub use_log_prob: bool,
    /// 最小概率阈值
    pub min_prob: f64,
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self {
            smooth: SmoothConfig::default(),
            use_log_prob: true,
            min_prob: 1e-10,
        }
    }
}

/// N-gram 评分器
pub struct NgramScorer {
    config: ScorerConfig,
    table: Option<NgramTable>,
}

impl NgramScorer {
    /// 创建新的评分器
    pub fn new(config: ScorerConfig) -> Self {
        Self {
            config,
            table: None,
        }
    }

    /// 设置概率表
    pub fn with_table(mut self, table: NgramTable) -> Self {
        self.table = Some(table);
        self
    }

    /// 评分单个词 P(word | context)
    ///
    /// 返回概率值（或对数概率）。
    pub fn score_word(&self, context: &[&str], word: &str) -> f64 {
        let table = match &self.table {
            Some(t) => t,
            None => return 0.0,
        };

        let prob = table.conditional_probability(context, word);
        let prob = prob.max(self.config.min_prob);

        if self.config.use_log_prob {
            prob.ln()
        } else {
            prob
        }
    }

    /// 评分序列：P(w1, w2, ..., wn) = Π P(wi | w1..wi-1)
    ///
    /// 返回总对数概率。
    pub fn score_sequence(&self, words: &[&str]) -> f64 {
        let table = match &self.table {
            Some(t) => t,
            None => return 0.0,
        };

        if words.is_empty() {
            return 0.0;
        }

        let order = table.order();
        let mut total_log_prob = 0.0;

        for i in 0..words.len() {
            let start = if i >= order - 1 { i - (order - 1) } else { 0 };
            let context = &words[start..i];
            let prob = table.conditional_probability(context, words[i]);
            let prob = prob.max(self.config.min_prob);
            total_log_prob += prob.ln();
        }

        if self.config.use_log_prob {
            total_log_prob
        } else {
            total_log_prob.exp()
        }
    }

    /// 计算困惑度
    pub fn perplexity(&self, words: &[&str]) -> f64 {
        if words.is_empty() {
            return 0.0;
        }

        let log_prob = self.score_sequence(words);
        let n = words.len() as f64;
        (-log_prob / n).exp()
    }

    /// 批量评分
    pub fn score_batch(&self, context: &[&str], words: &[&str]) -> Vec<(String, f64)> {
        words
            .iter()
            .map(|w| (w.to_string(), self.score_word(context, w)))
            .collect()
    }

    /// 检查是否已加载表
    pub fn has_table(&self) -> bool {
        self.table.is_some()
    }
}

/// 评分器构建器
pub struct ScorerBuilder {
    config: ScorerConfig,
}

impl ScorerBuilder {
    pub fn new() -> Self {
        Self {
            config: ScorerConfig::default(),
        }
    }

    pub fn with_config(mut self, config: ScorerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn with_smooth_parameter(mut self, param: f64) -> Self {
        self.config.smooth.parameter = param;
        self
    }

    pub fn with_log_prob(mut self, enable: bool) -> Self {
        self.config.use_log_prob = enable;
        self
    }

    pub fn build(self) -> NgramScorer {
        NgramScorer::new(self.config)
    }
}

impl Default for ScorerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::NgramTableBuilder;

    fn build_test_scorer() -> NgramScorer {
        let mut builder = NgramTableBuilder::new(2);
        builder.add_count(vec!["我".into()], "爱".into(), 10);
        builder.add_count(vec!["爱".into()], "中国".into(), 8);
        builder.add_count(vec!["我".into()], "是".into(), 5);
        let table = builder.build();

        NgramScorer::new(ScorerConfig::default()).with_table(table)
    }

    #[test]
    fn test_score_word() {
        let scorer = build_test_scorer();
        let score = scorer.score_word(&["我"], "爱");
        assert!(score < 0.0); // 对数概率为负
    }

    #[test]
    fn test_score_sequence() {
        let scorer = build_test_scorer();
        let score = scorer.score_sequence(&["我", "爱", "中国"]);
        assert!(score < 0.0);
    }

    #[test]
    fn test_perplexity() {
        let scorer = build_test_scorer();
        let pp = scorer.perplexity(&["我", "爱", "中国"]);
        assert!(pp > 0.0);
    }

    #[test]
    fn test_score_batch() {
        let scorer = build_test_scorer();
        let results = scorer.score_batch(&["我"], &["爱", "是", "不"]);
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_no_table() {
        let scorer = NgramScorer::new(ScorerConfig::default());
        assert_eq!(scorer.score_word(&["我"], "爱"), 0.0);
    }
}
