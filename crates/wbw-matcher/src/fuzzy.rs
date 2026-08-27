//! 模糊匹配模块

use std::fmt;
use thiserror::Error;
use wbw_types::{ImeError, ImeResult};

/// 模糊匹配错误类型
#[derive(Error, Debug)]
pub enum FuzzyError {
    #[error("模糊规则无效: {0}")]
    InvalidRule(String),
    
    #[error("匹配失败: {0}")]
    MatchError(String),
    
    #[error("配置错误: {0}")]
    ConfigError(String),
}

/// 模糊匹配规则
#[derive(Debug, Clone)]
pub struct FuzzyRule {
    /// 规则名称
    pub name: String,
    /// 源字符
    pub from: String,
    /// 目标字符
    pub to: String,
    /// 是否启用
    pub enabled: bool,
    /// 优先级（数字越大优先级越高）
    pub priority: u32,
}

impl FuzzyRule {
    /// 创建新的模糊规则
    pub fn new(name: String, from: String, to: String) -> Self {
        Self {
            name,
            from,
            to,
            enabled: true,
            priority: 0,
        }
    }

    /// 设置优先级
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// 禁用规则
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// 启用规则
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// 检查是否匹配
    pub fn matches(&self, input: &str) -> bool {
        // TODO: 实现匹配逻辑
        todo!("实现模糊规则匹配")
    }

    /// 应用规则
    pub fn apply(&self, input: &str) -> ImeResult<String> {
        // TODO: 实现规则应用逻辑
        todo!("实现模糊规则应用")
    }
}

impl fmt::Display for FuzzyRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} -> {}", self.name, self.from, self.to)
    }
}

/// 模糊匹配配置
#[derive(Debug, Clone)]
pub struct FuzzyConfig {
    /// 是否启用模糊匹配
    pub enabled: bool,
    /// 模糊规则列表
    pub rules: Vec<FuzzyRule>,
    /// 最大编辑距离
    pub max_edit_distance: usize,
    /// 是否区分大小写
    pub case_sensitive: bool,
    /// 是否启用声调匹配
    pub tone_matching: bool,
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Vec::new(),
            max_edit_distance: 1,
            case_sensitive: false,
            tone_matching: false,
        }
    }
}

/// 模糊匹配器
pub struct FuzzyMatcher {
    /// 配置
    config: FuzzyConfig,
    /// 预处理的规则映射
    rule_map: std::collections::HashMap<String, Vec<FuzzyRule>>,
}

impl FuzzyMatcher {
    /// 创建新的模糊匹配器
    pub fn new(config: FuzzyConfig) -> Self {
        let rule_map = Self::build_rule_map(&config.rules);
        Self { config, rule_map }
    }

    /// 从规则列表创建
    pub fn from_rules(rules: Vec<FuzzyRule>) -> Self {
        let config = FuzzyConfig {
            rules: rules.clone(),
            ..Default::default()
        };
        let rule_map = Self::build_rule_map(&rules);
        Self { config, rule_map }
    }

    /// 构建规则映射
    fn build_rule_map(rules: &[FuzzyRule]) -> std::collections::HashMap<String, Vec<FuzzyRule>> {
        let mut map = std::collections::HashMap::new();
        for rule in rules {
            if rule.enabled {
                map.entry(rule.from.clone())
                    .or_insert_with(Vec::new)
                    .push(rule.clone());
            }
        }
        map
    }

    /// 生成所有可能的变体
    pub fn generate_variants(&self, input: &str) -> Vec<String> {
        // TODO: 实现变体生成逻辑
        todo!("实现模糊变体生成")
    }

    /// 计算编辑距离
    pub fn edit_distance(&self, s1: &str, s2: &str) -> usize {
        // TODO: 实现编辑距离计算
        todo!("实现编辑距离计算")
    }

    /// 检查是否匹配
    pub fn is_match(&self, input: &str, target: &str) -> bool {
        // TODO: 实现匹配检查
        todo!("实现模糊匹配检查")
    }

    /// 获取配置
    pub fn config(&self) -> &FuzzyConfig {
        &self.config
    }

    /// 获取规则数量
    pub fn rule_count(&self) -> usize {
        self.config.rules.len()
    }
}

/// 模糊匹配结果
#[derive(Debug, Clone)]
pub struct FuzzyMatchResult {
    /// 原始输入
    pub input: String,
    /// 匹配结果
    pub matches: Vec<FuzzyMatch>,
    /// 匹配耗时（毫秒）
    pub elapsed_ms: f64,
}

/// 单个模糊匹配
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// 匹配的文本
    pub text: String,
    /// 匹配分数
    pub score: f64,
    /// 使用的规则
    pub rule: Option<FuzzyRule>,
    /// 编辑距离
    pub edit_distance: usize,
}

/// 预定义的模糊规则集合
pub struct FuzzyRulePresets;

impl FuzzyRulePresets {
    /// 获取拼音模糊规则
    pub fn pinyin_rules() -> Vec<FuzzyRule> {
        vec![
            FuzzyRule::new("z-zh".to_string(), "z".to_string(), "zh".to_string()),
            FuzzyRule::new("c-ch".to_string(), "c".to_string(), "ch".to_string()),
            FuzzyRule::new("s-sh".to_string(), "s".to_string(), "sh".to_string()),
            FuzzyRule::new("n-l".to_string(), "n".to_string(), "l".to_string()),
            FuzzyRule::new("l-n".to_string(), "l".to_string(), "n".to_string()),
            FuzzyRule::new("r-l".to_string(), "r".to_string(), "l".to_string()),
            FuzzyRule::new("an-ang".to_string(), "an".to_string(), "ang".to_string()),
            FuzzyRule::new("en-eng".to_string(), "en".to_string(), "eng".to_string()),
            FuzzyRule::new("in-ing".to_string(), "in".to_string(), "ing".to_string()),
        ]
    }

    /// 获取常见拼写错误规则
    pub fn typo_rules() -> Vec<FuzzyRule> {
        vec![
            FuzzyRule::new("ei-ie".to_string(), "ei".to_string(), "ie".to_string()),
            FuzzyRule::new("ui-iu".to_string(), "ui".to_string(), "iu".to_string()),
            FuzzyRule::new("un-ün".to_string(), "un".to_string(), "ün".to_string()),
        ]
    }

    /// 获取所有预定义规则
    pub fn all_rules() -> Vec<FuzzyRule> {
        let mut rules = Self::pinyin_rules();
        rules.extend(Self::typo_rules());
        rules
    }
}