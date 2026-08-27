//! N-gram 平滑处理模块
//!
//! 提供多种平滑方法的实现。

/// 平滑方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmoothMethod {
    /// 加一平滑（拉普拉斯平滑）
    Laplace,
    /// 加k平滑
    AddK,
    /// Good-Turing 平滑
    GoodTuring,
    /// 插值平滑
    Interpolation,
    /// 回退平滑
    Backoff,
}

impl std::fmt::Display for SmoothMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmoothMethod::Laplace => write!(f, "拉普拉斯平滑"),
            SmoothMethod::AddK => write!(f, "加k平滑"),
            SmoothMethod::GoodTuring => write!(f, "Good-Turing平滑"),
            SmoothMethod::Interpolation => write!(f, "插值平滑"),
            SmoothMethod::Backoff => write!(f, "回退平滑"),
        }
    }
}

/// 平滑配置
#[derive(Debug, Clone)]
pub struct SmoothConfig {
    pub method: SmoothMethod,
    pub parameter: f64,
    pub enable_backoff: bool,
    pub backoff_threshold: f64,
}

impl Default for SmoothConfig {
    fn default() -> Self {
        Self {
            method: SmoothMethod::Laplace,
            parameter: 1.0,
            enable_backoff: true,
            backoff_threshold: 0.01,
        }
    }
}

/// 平滑处理器
pub struct Smoother {
    config: SmoothConfig,
}

impl Smoother {
    pub fn new(config: SmoothConfig) -> Self {
        Self { config }
    }

    /// 拉普拉斯平滑：(count + alpha) / (total + alpha * vocab_size)
    pub fn laplace(count: f64, total: f64, alpha: f64) -> f64 {
        if total + alpha <= 0.0 {
            return 0.0;
        }
        (count + alpha) / (total + alpha)
    }

    /// 加k平滑
    pub fn add_k(count: f64, total: f64, k: f64) -> f64 {
        if total + k <= 0.0 {
            return 0.0;
        }
        (count + k) / (total + k)
    }

    /// 插值平滑：lambda * high + (1-lambda) * low
    pub fn interpolation(high_order_prob: f64, low_order_prob: f64, lambda: f64) -> f64 {
        lambda * high_order_prob + (1.0 - lambda) * low_order_prob
    }

    /// 回退平滑：如果高阶概率低于阈值，使用低阶概率
    pub fn backoff(high_order_prob: f64, low_order_prob: f64, threshold: f64) -> f64 {
        if high_order_prob >= threshold {
            high_order_prob
        } else {
            low_order_prob
        }
    }

    /// 应用平滑方法
    pub fn apply(&self, count: f64, total: f64) -> f64 {
        match self.config.method {
            SmoothMethod::Laplace => Self::laplace(count, total, self.config.parameter),
            SmoothMethod::AddK => Self::add_k(count, total, self.config.parameter),
            SmoothMethod::Interpolation => Self::interpolation(count, total, self.config.parameter),
            SmoothMethod::Backoff => Self::backoff(count, total, self.config.backoff_threshold),
            SmoothMethod::GoodTuring => Self::laplace(count, total, 0.5), // 简化处理
        }
    }
}

/// 平滑效果评估
pub struct SmoothEvaluator;

impl SmoothEvaluator {
    /// 计算困惑度（Perplexity）
    pub fn perplexity(log_probs: &[f64]) -> f64 {
        if log_probs.is_empty() {
            return 0.0;
        }
        let sum: f64 = log_probs.iter().sum();
        let avg_log_prob = sum / log_probs.len() as f64;
        (-avg_log_prob).exp()
    }

    /// 计算交叉熵
    pub fn cross_entropy(probs: &[f64]) -> f64 {
        if probs.is_empty() {
            return 0.0;
        }
        let sum: f64 = probs.iter().map(|p| if *p > 0.0 { -p.ln() } else { 0.0 }).sum();
        sum / probs.len() as f64
    }

    /// 计算困惑度改进率
    pub fn perplexity_improvement(baseline: f64, improved: f64) -> f64 {
        if baseline == 0.0 {
            return 0.0;
        }
        (baseline - improved) / baseline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_laplace() {
        let prob = Smoother::laplace(5.0, 100.0, 1.0);
        assert!((prob - 6.0 / 101.0).abs() < 1e-10);
    }

    #[test]
    fn test_add_k() {
        let prob = Smoother::add_k(3.0, 50.0, 0.5);
        assert!((prob - 3.5 / 50.5).abs() < 1e-10);
    }

    #[test]
    fn test_interpolation() {
        let prob = Smoother::interpolation(0.8, 0.2, 0.7);
        assert!((prob - 0.62).abs() < 1e-10);
    }

    #[test]
    fn test_backoff() {
        assert_eq!(Smoother::backoff(0.5, 0.1, 0.01), 0.5);
        assert_eq!(Smoother::backoff(0.005, 0.1, 0.01), 0.1);
    }

    #[test]
    fn test_perplexity() {
        let log_probs = vec![-1.0, -2.0, -0.5];
        let pp = SmoothEvaluator::perplexity(&log_probs);
        assert!(pp > 0.0);
    }

    #[test]
    fn test_cross_entropy() {
        let probs = vec![0.5, 0.3, 0.2];
        let ce = SmoothEvaluator::cross_entropy(&probs);
        assert!(ce > 0.0);
    }
}
