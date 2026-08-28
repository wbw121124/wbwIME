//! L0 动态学习模块

use std::path::Path;
use thiserror::Error;
use serde::{Deserialize, Serialize};
use wbw_types::{ImeResult, L0Config, Candidate, CandidateSource};

/// L0 学习错误类型
#[derive(Error, Debug)]
pub enum L0Error {
    #[error("快照加载失败: {0}")]
    SnapshotLoadError(String),
    
    #[error("快照保存失败: {0}")]
    SnapshotSaveError(String),
    
    #[error("学习数据不足: {0}")]
    InsufficientData(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// L0 学习器
pub struct L0Learner {
    /// 配置
    config: L0Config,
    /// 学习数据
    data: Vec<LearningEntry>,
    /// 计数器
    counters: std::collections::HashMap<String, u32>,
}

/// L0 快照数据结构（用于持久化）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L0Snapshot {
    /// 学习数据
    data: Vec<LearningEntry>,
    /// 计数器
    counters: std::collections::HashMap<String, u32>,
}

impl L0Learner {
    /// 创建新的学习器
    pub fn new(config: L0Config) -> Self {
        Self {
            config,
            data: Vec::new(),
            counters: std::collections::HashMap::new(),
        }
    }

    /// 从快照加载
    pub fn from_snapshot(config: L0Config, path: &Path) -> ImeResult<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("快照读取失败: {} ({})", path.display(), e)))?;
        let snapshot: L0Snapshot = serde_json::from_str(&contents)
            .map_err(|e| wbw_types::ImeError::ConfigError(format!("快照解析失败: {}", e)))?;
        Ok(Self {
            config,
            data: snapshot.data,
            counters: snapshot.counters,
        })
    }

    /// 记录用户选择
    pub fn record_selection(&mut self, code: &str, word: &str) {
        let key = format!("{}:{}", code, word);
        *self.counters.entry(key).or_insert(0) += 1;
        
        let entry = LearningEntry {
            code: code.to_string(),
            word: word.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        
        self.data.push(entry);
    }

    /// 检查是否达到学习阈值
    pub fn should_learn(&self, code: &str, word: &str) -> bool {
        let key = format!("{}:{}", code, word);
        self.counters.get(&key).map_or(false, |&count| count >= self.config.threshold)
    }

    /// 获取学习建议
    pub fn get_suggestions(&self) -> Vec<LearningSuggestion> {
        self.counters
            .iter()
            .filter(|(_, &count)| count >= self.config.threshold)
            .map(|(key, &count)| {
                let parts: Vec<&str> = key.split(':').collect();
                let code = parts.get(0).unwrap_or(&"").to_string();
                let word = parts.get(1).unwrap_or(&"").to_string();
                let confidence = (count as f64 / (self.config.threshold as f64 * 2.0)).min(1.0);
                LearningSuggestion {
                    code,
                    word,
                    confidence,
                    selection_count: count,
                    suggestion_type: if count >= self.config.threshold * 2 {
                        SuggestionType::FreqBoost
                    } else {
                        SuggestionType::Reorder
                    },
                }
            })
            .collect()
    }

    /// 获取高频建议（code, word, count）
    pub fn get_top_suggestions(&self, limit: usize) -> Vec<(String, String, u32)> {
        let mut entries: Vec<(&String, &u32)> = self.counters.iter().collect();
        entries.sort_by(|a, b| b.1.cmp(a.1));
        entries
            .into_iter()
            .take(limit)
            .filter_map(|(key, &count)| {
                if count >= self.config.threshold {
                    let parts: Vec<&str> = key.split(':').collect();
                    let code = parts.get(0)?.to_string();
                    let word = parts.get(1)?.to_string();
                    Some((code, word, count))
                } else {
                    None
                }
            })
            .collect()
    }

    /// 应用学习结果
    pub fn apply_learnings(&self) -> ImeResult<Vec<Candidate>> {
        Ok(self
            .get_suggestions()
            .into_iter()
            .map(|s| Candidate {
                text: s.word,
                code: s.code,
                score: s.confidence,
                source: CandidateSource::Dynamic,
                ngram_score: None,
                user_weight: Some(s.confidence),
            })
            .collect())
    }

    /// 保存快照
    pub fn save_snapshot(&self) -> ImeResult<()> {
        let snapshot = L0Snapshot {
            data: self.data.clone(),
            counters: self.counters.clone(),
        };
        let json = serde_json::to_string_pretty(&snapshot)
            .map_err(|e| wbw_types::ImeError::ConfigError(format!("快照序列化失败: {}", e)))?;
        std::fs::write(&self.config.snapshot_path, json)
            .map_err(|e| wbw_types::ImeError::IoError(format!("快照保存失败: {} ({})", self.config.snapshot_path, e)))
    }

    /// 加载快照
    pub fn load_snapshot(&mut self) -> ImeResult<()> {
        let contents = std::fs::read_to_string(&self.config.snapshot_path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("快照读取失败: {} ({})", self.config.snapshot_path, e)))?;
        let snapshot: L0Snapshot = serde_json::from_str(&contents)
            .map_err(|e| wbw_types::ImeError::ConfigError(format!("快照解析失败: {}", e)))?;
        self.data = snapshot.data;
        self.counters = snapshot.counters;
        Ok(())
    }

    /// 清空学习数据
    pub fn clear(&mut self) {
        self.data.clear();
        self.counters.clear();
    }

    /// 获取学习数据数量
    pub fn data_count(&self) -> usize {
        self.data.len()
    }

    /// 获取配置
    pub fn config(&self) -> &L0Config {
        &self.config
    }
}

/// 学习条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningEntry {
    /// 编码
    pub code: String,
    /// 词文本
    pub word: String,
    /// 时间戳
    pub timestamp: u64,
}

/// 学习建议
#[derive(Debug, Clone)]
pub struct LearningSuggestion {
    /// 编码
    pub code: String,
    /// 建议词
    pub word: String,
    /// 置信度
    pub confidence: f64,
    /// 基于的选择次数
    pub selection_count: u32,
    /// 建议类型
    pub suggestion_type: SuggestionType,
}

/// 建议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionType {
    /// 新词学习
    NewWord,
    /// 词频提升
    FreqBoost,
    /// 词序调整
    Reorder,
    /// 词性推断
    PosInference,
}

/// L0 学习统计
#[derive(Debug, Clone, Default)]
pub struct L0Stats {
    /// 总学习条目数
    pub total_entries: usize,
    /// 达到阈值的条目数
    pub threshold_reached: usize,
    /// 平均选择次数
    pub avg_selections: f64,
    /// 最高选择次数
    pub max_selections: u32,
    /// 学习准确率
    pub accuracy: f64,
}

/// L0 统计收集器
pub struct L0StatsCollector {
    /// 统计信息
    stats: L0Stats,
    /// 选择记录
    selections: Vec<(String, String, u32)>,
}

impl L0StatsCollector {
    /// 创建新的统计收集器
    pub fn new() -> Self {
        Self {
            stats: L0Stats::default(),
            selections: Vec::new(),
        }
    }

    /// 记录学习事件
    pub fn record_learning(&mut self, code: &str, word: &str, count: u32) {
        self.selections.push((code.to_string(), word.to_string(), count));
        
        // 更新统计
        self.stats.total_entries += 1;
        if count >= 3 { // 假设阈值为3
            self.stats.threshold_reached += 1;
        }
        
        // 更新最大选择次数
        if count > self.stats.max_selections {
            self.stats.max_selections = count;
        }
        
        // 更新平均选择次数
        let total: u32 = self.selections.iter().map(|(_, _, c)| c).sum();
        self.stats.avg_selections = total as f64 / self.selections.len() as f64;
    }

    /// 获取统计信息
    pub fn stats(&self) -> &L0Stats {
        &self.stats
    }

    /// 获取学习效率
    pub fn efficiency(&self) -> f64 {
        if self.stats.total_entries == 0 {
            return 0.0;
        }
        self.stats.threshold_reached as f64 / self.stats.total_entries as f64
    }
}

impl Default for L0StatsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// L0 学习策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum L0Strategy {
    /// 基于阈值的学习
    Threshold,
    /// 基于时间衰减的学习
    TimeDecay,
    /// 基于频率的学习
    Frequency,
    /// 混合策略
    Hybrid,
}

/// L0 学习配置
#[derive(Debug, Clone)]
pub struct L0LearnConfig {
    /// 学习策略
    pub strategy: L0Strategy,
    /// 阈值
    pub threshold: u32,
    /// 时间衰减因子
    pub decay_factor: f64,
    /// 最大学习条目数
    pub max_entries: usize,
    /// 是否启用自动学习
    pub auto_learn: bool,
}

impl Default for L0LearnConfig {
    fn default() -> Self {
        Self {
            strategy: L0Strategy::Threshold,
            threshold: 3,
            decay_factor: 0.9,
            max_entries: 10000,
            auto_learn: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(name: &str) -> L0Config {
        L0Config {
            threshold: 2,
            snapshot_path: std::env::temp_dir()
                .join(format!("wbw_l0_snapshot_{}_{}.json", std::process::id(), name))
                .to_string_lossy()
                .to_string(),
        }
    }

    #[test]
    fn test_save_and_load_snapshot() {
        let config = test_config("save_load");
        let mut learner = L0Learner::new(config.clone());
        learner.record_selection("zhongguo", "中国");
        learner.record_selection("zhongguo", "中国");
        learner.save_snapshot().unwrap();

        let loaded = L0Learner::from_snapshot(config, Path::new(&learner.config().snapshot_path)).unwrap();
        assert_eq!(loaded.data_count(), 2);
        assert!(loaded.should_learn("zhongguo", "中国"));

        std::fs::remove_file(&learner.config().snapshot_path).ok();
    }

    #[test]
    fn test_load_snapshot_into_existing() {
        let config = test_config("load_existing");
        let mut learner = L0Learner::new(config.clone());
        learner.record_selection("a", "啊");
        learner.record_selection("a", "啊");
        learner.save_snapshot().unwrap();

        let mut other = L0Learner::new(config.clone());
        other.load_snapshot().unwrap();
        assert!(other.should_learn("a", "啊"));

        std::fs::remove_file(&config.snapshot_path).ok();
    }

    #[test]
    fn test_apply_learnings() {
        let config = test_config("apply");
        let mut learner = L0Learner::new(config);
        learner.record_selection("zhongguo", "中国");
        learner.record_selection("zhongguo", "中国");

        let candidates = learner.apply_learnings().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].text, "中国");
        assert_eq!(candidates[0].code, "zhongguo");
        assert_eq!(candidates[0].source, CandidateSource::Dynamic);
        assert!(candidates[0].user_weight.is_some());
    }
}