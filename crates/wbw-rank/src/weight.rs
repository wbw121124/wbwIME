//! 权重计算模块

use thiserror::Error;
use wbw_types::{Candidate, CandidateSource, ImeResult, RankConfig};

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
    /// 评估某一组权重在给定期望顺序下的准确率
    ///
    /// 用 WeightCalculator 计算每个候选词的权重，按权重降序排序后，
    /// 取与前若干名（与期望顺序长度一致）对比，统计排名匹配的比率。
    fn evaluate(candidates: &[Candidate], expected_order: &[usize], config: &RankConfig) -> f64 {
        if candidates.is_empty() || expected_order.is_empty() {
            return 0.0;
        }

        let calculator = WeightCalculator::new(config.clone());
        let mut indexed: Vec<(usize, f64)> = candidates
            .iter()
            .enumerate()
            .map(|(idx, c)| (idx, calculator.calculate_weight(c)))
            .collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 期望顺序中的前 N 个（N 为期望顺序长度）
        let expected: Vec<usize> = expected_order.iter().cloned().collect();
        let limit = expected.len().min(indexed.len());
        let actual: Vec<usize> = indexed.iter().take(limit).map(|(idx, _)| *idx).collect();

        if limit == 0 {
            return 0.0;
        }

        let matched = actual
            .iter()
            .zip(expected.iter())
            .filter(|(a, e)| a == e)
            .count();
        matched as f64 / limit as f64
    }

    /// 网格搜索最优权重
    pub fn grid_search(
        candidates: &[Candidate],
        expected_order: &[usize],
        param_ranges: &WeightRanges,
    ) -> ImeResult<(RankConfig, f64)> {
        const STEPS: usize = 5;

        let step = |(min, max): (f64, f64)| {
            (0..STEPS)
                .map(|i| {
                    let t = i as f64 / (STEPS - 1) as f64;
                    min + (max - min) * t
                })
                .collect::<Vec<f64>>()
        };

        let pins = step(param_ranges.pin_weight);
        let users = step(param_ranges.user_weight);
        let freqs = step(param_ranges.freq_weight);
        let ngrams = step(param_ranges.ngram_weight);

        let mut best_config = RankConfig::default();
        let mut best_accuracy = -1.0_f64;

        for &pin in &pins {
            for &user in &users {
                for &freq in &freqs {
                    for &ngram in &ngrams {
                        let config = RankConfig {
                            pin_weight: pin,
                            user_weight: user,
                            freq_weight: freq,
                            ngram_weight: ngram,
                            max_candidates: candidates.len().max(1),
                        };
                        let accuracy = Self::evaluate(candidates, expected_order, &config);
                        if accuracy > best_accuracy {
                            best_accuracy = accuracy;
                            best_config = config;
                        }
                    }
                }
            }
        }

        Ok((best_config, best_accuracy.max(0.0)))
    }

    /// 随机搜索
    pub fn random_search(
        candidates: &[Candidate],
        expected_order: &[usize],
        iterations: usize,
    ) -> ImeResult<(RankConfig, f64)> {
        let ranges = WeightRanges::default();
        let mut seed: u64 = 0x9E3779B97F4A7C15;

        // 生成 [0, 1) 的确定性伪随机数
        let next_random = move || -> f64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fffffff;
            (seed & 0x7fffffff) as f64 / (0x7fffffff as f64)
        };

        let sample = |(min, max): (f64, f64), rng: &mut dyn FnMut() -> f64| {
            min + (max - min) * rng()
        };

        let mut best_config = RankConfig::default();
        let mut best_accuracy = -1.0_f64;
        let mut rng = next_random;

        for _ in 0..iterations.max(1) {
            let config = RankConfig {
                pin_weight: sample(ranges.pin_weight, &mut rng),
                user_weight: sample(ranges.user_weight, &mut rng),
                freq_weight: sample(ranges.freq_weight, &mut rng),
                ngram_weight: sample(ranges.ngram_weight, &mut rng),
                max_candidates: candidates.len().max(1),
            };
            let accuracy = Self::evaluate(candidates, expected_order, &config);
            if accuracy > best_accuracy {
                best_accuracy = accuracy;
                best_config = config;
            }
        }

        Ok((best_config, best_accuracy.max(0.0)))
    }

    /// 模拟退火
    pub fn simulated_annealing(
        candidates: &[Candidate],
        expected_order: &[usize],
        initial_temp: f64,
        cooling_rate: f64,
        iterations: usize,
    ) -> ImeResult<(RankConfig, f64)> {
        let ranges = WeightRanges::default();
        let mut seed: u64 = 0x243F6A8885A308D3;
        let next_random = move || -> f64 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fffffff;
            (seed & 0x7fffffff) as f64 / (0x7fffffff as f64)
        };
        let mut rng = next_random;

        let clamp = |v: f64, (min, max): (f64, f64)| v.clamp(min, max);
        let random_from = |(min, max): (f64, f64), rng: &mut dyn FnMut() -> f64| {
            min + (max - min) * rng()
        };

        let mut current = RankConfig {
            pin_weight: random_from(ranges.pin_weight, &mut rng),
            user_weight: random_from(ranges.user_weight, &mut rng),
            freq_weight: random_from(ranges.freq_weight, &mut rng),
            ngram_weight: random_from(ranges.ngram_weight, &mut rng),
            max_candidates: candidates.len().max(1),
        };
        let mut current_accuracy = Self::evaluate(candidates, expected_order, &current);

        let mut best = current.clone();
        let mut best_accuracy = current_accuracy;
        let mut temperature = initial_temp.max(0.0);

        for _ in 0..iterations.max(1) {
            // 温度衰减
            temperature *= cooling_rate;
            if temperature <= 1e-10 {
                break;
            }

            // 随机扰动当前权重
            let perturb = |v: f64, (min, max): (f64, f64), rng: &mut dyn FnMut() -> f64| {
                let amount = (rng() - 0.5) * temperature;
                clamp(v + amount, (min, max))
            };

            let mut neighbor = current.clone();
            neighbor.pin_weight = perturb(current.pin_weight, ranges.pin_weight, &mut rng);
            neighbor.user_weight = perturb(current.user_weight, ranges.user_weight, &mut rng);
            neighbor.freq_weight = perturb(current.freq_weight, ranges.freq_weight, &mut rng);
            neighbor.ngram_weight = perturb(current.ngram_weight, ranges.ngram_weight, &mut rng);

            let neighbor_accuracy = Self::evaluate(candidates, expected_order, &neighbor);

            let accept = if neighbor_accuracy >= current_accuracy {
                true
            } else {
                let delta = neighbor_accuracy - current_accuracy;
                let p = (delta / temperature).exp();
                rng() < p
            };

            if accept {
                current = neighbor;
                current_accuracy = neighbor_accuracy;
            }

            if current_accuracy > best_accuracy {
                best = current.clone();
                best_accuracy = current_accuracy;
            }
        }

        Ok((best, best_accuracy.max(0.0)))
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
                ngram_score: Some(0.8),
                user_weight: Some(0.9),
            },
            Candidate {
                text: "终于".into(),
                code: "zhongyu".into(),
                score: 50.0,
                source: CandidateSource::System,
                ngram_score: Some(0.6),
                user_weight: Some(0.4),
            },
            Candidate {
                text: "中".into(),
                code: "zhong".into(),
                score: 20.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
        ]
    }

    #[test]
    fn test_grid_search() {
        let candidates = test_candidates();
        let expected = vec![0, 1, 2];
        let (config, accuracy) =
            WeightTuner::grid_search(&candidates, &expected, &WeightRanges::default()).unwrap();
        assert!(accuracy >= 0.0 && accuracy <= 1.0);
        assert!(config.max_candidates >= 1);
    }

    #[test]
    fn test_random_search() {
        let candidates = test_candidates();
        let expected = vec![0, 1, 2];
        let (_, accuracy) = WeightTuner::random_search(&candidates, &expected, 100).unwrap();
        assert!(accuracy >= 0.0 && accuracy <= 1.0);
    }

    #[test]
    fn test_simulated_annealing() {
        let candidates = test_candidates();
        let expected = vec![0, 1, 2];
        let (_, accuracy) =
            WeightTuner::simulated_annealing(&candidates, &expected, 100.0, 0.99, 200).unwrap();
        assert!(accuracy >= 0.0 && accuracy <= 1.0);
    }
}