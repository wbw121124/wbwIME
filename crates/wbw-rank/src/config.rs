//! 排序配置模块

use std::path::Path;
use thiserror::Error;
use wbw_types::{ImeResult, RankConfig};

/// 配置错误类型
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("配置文件读取失败: {0}")]
    FileError(String),
    
    #[error("配置解析失败: {0}")]
    ParseError(String),
    
    #[error("配置验证失败: {0}")]
    ValidationError(String),
    
    #[error("配置保存失败: {0}")]
    SaveError(String),
}

/// 排序配置管理器
pub struct RankConfigManager {
    /// 配置
    config: RankConfig,
    /// 配置文件路径
    config_path: Option<String>,
}

impl RankConfigManager {
    /// 创建新的配置管理器
    pub fn new() -> Self {
        Self {
            config: RankConfig::default(),
            config_path: None,
        }
    }

    /// 从文件加载配置
    pub fn from_file(path: &Path) -> ImeResult<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| wbw_types::ImeError::IoError(format!("配置文件读取失败: {} ({})", path.display(), e)))?;
        let config: RankConfig = serde_json::from_str(&contents)
            .map_err(|e| wbw_types::ImeError::ConfigError(format!("配置解析失败: {}", e)))?;
        Ok(Self {
            config,
            config_path: Some(path.to_string_lossy().to_string()),
        })
    }

    /// 从内存加载配置
    pub fn from_memory(config: RankConfig) -> Self {
        Self {
            config,
            config_path: None,
        }
    }

    /// 获取配置
    pub fn config(&self) -> &RankConfig {
        &self.config
    }

    /// 获取可变配置
    pub fn config_mut(&mut self) -> &mut RankConfig {
        &mut self.config
    }

    /// 设置配置
    pub fn set_config(&mut self, config: RankConfig) {
        self.config = config;
    }

    /// 保存配置到文件
    pub fn save(&self) -> ImeResult<()> {
        if let Some(path) = &self.config_path {
            self.save_to(Path::new(path))
        } else {
            Err(wbw_types::ImeError::ConfigError("未设置配置文件路径".to_string()))
        }
    }

    /// 保存配置到指定路径
    pub fn save_to(&self, path: &Path) -> ImeResult<()> {
        let json = serde_json::to_string_pretty(&self.config)
            .map_err(|e| wbw_types::ImeError::ConfigError(format!("配置序列化失败: {}", e)))?;
        std::fs::write(path, json)
            .map_err(|e| wbw_types::ImeError::IoError(format!("配置保存失败: {} ({})", path.display(), e)))
    }

    /// 验证配置
    pub fn validate(&self) -> ImeResult<()> {
        ConfigValidator::validate(&self.config)
    }

    /// 重置为默认配置
    pub fn reset_to_default(&mut self) {
        self.config = RankConfig::default();
    }

    /// 获取配置文件路径
    pub fn config_path(&self) -> Option<&str> {
        self.config_path.as_deref()
    }

    /// 设置配置文件路径
    pub fn set_config_path(&mut self, path: String) {
        self.config_path = Some(path);
    }
}

impl Default for RankConfigManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 配置预设
pub struct ConfigPresets;

impl ConfigPresets {
    /// 获取默认配置
    pub fn default_config() -> RankConfig {
        RankConfig::default()
    }

    /// 获取高性能配置
    pub fn high_performance() -> RankConfig {
        RankConfig {
            pin_weight: 100.0,
            user_weight: 15.0,
            freq_weight: 2.0,
            ngram_weight: 1.0,
            max_candidates: 5,
        }
    }

    /// 获取高精度配置
    pub fn high_accuracy() -> RankConfig {
        RankConfig {
            pin_weight: 80.0,
            user_weight: 20.0,
            freq_weight: 1.5,
            ngram_weight: 2.0,
            max_candidates: 15,
        }
    }

    /// 获取平衡配置
    pub fn balanced() -> RankConfig {
        RankConfig {
            pin_weight: 90.0,
            user_weight: 12.0,
            freq_weight: 1.2,
            ngram_weight: 0.8,
            max_candidates: 10,
        }
    }

    /// 获取配置列表
    pub fn all_presets() -> Vec<(&'static str, RankConfig)> {
        vec![
            ("默认", Self::default_config()),
            ("高性能", Self::high_performance()),
            ("高精度", Self::high_accuracy()),
            ("平衡", Self::balanced()),
        ]
    }
}

/// 配置验证器
pub struct ConfigValidator;

impl ConfigValidator {
    /// 验证配置有效性
    pub fn validate(config: &RankConfig) -> ImeResult<()> {
        let errors = Self::validation_errors(config);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(wbw_types::ImeError::ConfigError(errors.join("；")))
        }
    }

    /// 验证权重范围
    pub fn validate_weights(config: &RankConfig) -> bool {
        config.pin_weight >= 0.0
            && config.user_weight >= 0.0
            && config.freq_weight >= 0.0
            && config.ngram_weight >= 0.0
    }

    /// 验证最大候选词数量
    pub fn validate_max_candidates(config: &RankConfig) -> bool {
        config.max_candidates > 0 && config.max_candidates <= 100
    }

    /// 获取验证错误消息
    pub fn validation_errors(config: &RankConfig) -> Vec<String> {
        let mut errors = Vec::new();
        
        if !Self::validate_weights(config) {
            errors.push("权重值不能为负数".to_string());
        }
        
        if !Self::validate_max_candidates(config) {
            errors.push("最大候选词数量必须在 1-100 之间".to_string());
        }
        
        errors
    }
}

/// 配置差异比较
#[derive(Debug, Clone)]
pub struct ConfigDiff {
    /// 权重差异
    pub weight_diff: WeightDiff,
    /// 最大候选词数量差异
    pub max_candidates_diff: Option<i32>,
}

/// 权重差异
#[derive(Debug, Clone)]
pub struct WeightDiff {
    /// 拼音权重差异
    pub pin_weight: f64,
    /// 用户权重差异
    pub user_weight: f64,
    /// 词频权重差异
    pub freq_weight: f64,
    /// N-gram 权重差异
    pub ngram_weight: f64,
}

impl ConfigDiff {
    /// 计算配置差异
    pub fn diff(old: &RankConfig, new: &RankConfig) -> Self {
        Self {
            weight_diff: WeightDiff {
                pin_weight: new.pin_weight - old.pin_weight,
                user_weight: new.user_weight - old.user_weight,
                freq_weight: new.freq_weight - old.freq_weight,
                ngram_weight: new.ngram_weight - old.ngram_weight,
            },
            max_candidates_diff: if old.max_candidates != new.max_candidates {
                Some(new.max_candidates as i32 - old.max_candidates as i32)
            } else {
                None
            },
        }
    }

    /// 检查是否有差异
    pub fn has_changes(&self) -> bool {
        self.weight_diff.pin_weight != 0.0
            || self.weight_diff.user_weight != 0.0
            || self.weight_diff.freq_weight != 0.0
            || self.weight_diff.ngram_weight != 0.0
            || self.max_candidates_diff.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_config() -> RankConfig {
        RankConfig {
            pin_weight: 90.0,
            user_weight: 12.0,
            freq_weight: 1.2,
            ngram_weight: 0.8,
            max_candidates: 10,
        }
    }

    #[test]
    fn test_from_file_and_save_to() {
        let dir = std::env::temp_dir().join(format!("wbw_rank_config_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path: PathBuf = dir.join("rank.json");

        let manager = RankConfigManager::from_memory(sample_config());
        manager.save_to(&path).unwrap();

        let loaded = RankConfigManager::from_file(&path).unwrap();
        assert_eq!(loaded.config().pin_weight, 90.0);
        assert_eq!(loaded.config().max_candidates, 10);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_save_without_path() {
        let manager = RankConfigManager::from_memory(sample_config());
        assert!(manager.save().is_err());
    }

    #[test]
    fn test_validate() {
        let valid = ConfigValidator::validate(&sample_config());
        assert!(valid.is_ok());

        let invalid = RankConfig {
            max_candidates: 0,
            ..sample_config()
        };
        assert!(ConfigValidator::validate(&invalid).is_err());
    }
}