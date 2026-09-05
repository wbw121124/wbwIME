//! 模糊匹配模块
//!
//! 提供基于规则的拼音模糊匹配，支持声母替换、韵母混淆等。

use std::collections::HashMap;

/// 模糊匹配规则
#[derive(Debug, Clone)]
pub struct FuzzyRule {
    /// 规则名称
    pub name: String,
    /// 源字符/音素
    pub from: String,
    /// 目标字符/音素
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
}

impl std::fmt::Display for FuzzyRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
}

impl Default for FuzzyConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            rules: Self::default_pinyin_rules(),
            max_edit_distance: 1,
        }
    }
}

impl FuzzyConfig {
    /// 默认拼音模糊规则
    fn default_pinyin_rules() -> Vec<FuzzyRule> {
        FuzzyRulePresets::all_rules()
    }
}

/// 模糊匹配器
pub struct FuzzyMatcher {
    config: FuzzyConfig,
    /// from → [rules] 映射，用于快速查找
    rule_map: HashMap<String, Vec<FuzzyRule>>,
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

    /// 使用默认拼音规则创建
    pub fn pinyin_default() -> Self {
        Self::new(FuzzyConfig::default())
    }

    /// 构建规则映射
    fn build_rule_map(rules: &[FuzzyRule]) -> HashMap<String, Vec<FuzzyRule>> {
        let mut map = HashMap::new();
        for rule in rules {
            if rule.enabled {
                map.entry(rule.from.clone())
                    .or_insert_with(Vec::new)
                    .push(rule.clone());
            }
        }
        map
    }

    const MAX_VARIANTS: usize = 512;

    /// 生成输入的所有模糊变体
    ///
    /// 对输入中的每个音素，根据规则生成所有可能的替换，
    /// 然后组合所有可能的变体。
    ///
    /// 基于字符序列处理，避免多字节切片越界，且替换长度变化
    /// （如 z→zh）不会造成后续偏移错位。
    ///
    /// 变体总数上限为 [`MAX_VARIANTS`](Self::MAX_VARIANTS)，
    /// 超出后截断以防止组合爆炸。
    pub fn generate_variants(&self, input: &str) -> Vec<String> {
        if !self.config.enabled {
            return vec![input.to_string()];
        }

        let mut variants: Vec<String> = vec![input.to_string()];

        for (from, rules) in &self.rule_map {
            let from_chars: Vec<char> = from.chars().collect();
            if from_chars.is_empty() {
                continue;
            }
            let targets: Vec<&str> = rules.iter().map(|r| r.to.as_str()).collect();
            let mut new_variants: Vec<String> = Vec::new();

            for variant in &variants {
                if variants.len() + new_variants.len() >= Self::MAX_VARIANTS {
                    break;
                }
                let chars: Vec<char> = variant.chars().collect();
                // 在每个不重叠的字符位置尝试替换
                let mut i = 0;
                while i + from_chars.len() <= chars.len() {
                    if chars[i..i + from_chars.len()] == from_chars[..] {
                        for target in &targets {
                            if variants.len() + new_variants.len() >= Self::MAX_VARIANTS {
                                break;
                            }
                            let mut out: String = chars[..i].iter().collect();
                            out.push_str(target);
                            out.extend(chars[i + from_chars.len()..].iter());
                            new_variants.push(out);
                        }
                        i += from_chars.len();
                    } else {
                        i += 1;
                    }
                }
            }

            variants.extend(new_variants);
        }

        // 去重
        variants.sort();
        variants.dedup();
        variants.truncate(Self::MAX_VARIANTS);
        variants
    }

    /// 计算编辑距离（Levenshtein 距离）
    ///
    /// 复用 `wbw-dict` 中的同一实现，避免重复。
    pub fn edit_distance(s1: &str, s2: &str) -> usize {
        wbw_dict::edit_distance(s1, s2)
    }

    /// 检查是否模糊匹配
    ///
    /// 判断 input 是否与 target 在编辑距离限制内匹配，
    /// 或者 input 的某个变体与 target 相同。
    pub fn is_match(&self, input: &str, target: &str) -> bool {
        if !self.config.enabled {
            return input == target;
        }

        // 精确匹配
        if input == target {
            return true;
        }

        // 编辑距离匹配
        let dist = Self::edit_distance(input, target);
        if dist <= self.config.max_edit_distance {
            return true;
        }

        // 规则变体匹配
        let variants = self.generate_variants(input);
        variants.iter().any(|v| v == target)
    }

    /// 对候选列表进行模糊匹配过滤
    pub fn filter_candidates<T>(
        &self,
        input: &str,
        candidates: &[T],
        extract: impl Fn(&T) -> &str,
    ) -> Vec<(T, f64)>
    where
        T: Clone,
    {
        let mut results: Vec<(T, f64)> = Vec::new();

        let variants = self.generate_variants(input);

        for candidate in candidates {
            let target = extract(candidate);

            if input == target {
                // 精确匹配，最高分
                results.push((candidate.clone(), 100.0));
                continue;
            }

            // 规则变体匹配
            if variants.iter().any(|v| v == target) {
                results.push((candidate.clone(), 80.0));
                continue;
            }

            // 编辑距离匹配
            let dist = Self::edit_distance(input, target);
            if dist <= self.config.max_edit_distance {
                let score = 60.0 - (dist as f64) * 20.0;
                results.push((candidate.clone(), score));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results
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
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FuzzyMatchResult {
    /// 原始输入
    pub input: String,
    /// 匹配的变体
    pub variants: Vec<String>,
    /// 匹配耗时（毫秒）
    pub elapsed_ms: f64,
}

/// 预定义的模糊规则集合
pub struct FuzzyRulePresets;

impl FuzzyRulePresets {
    /// 获取拼音声母模糊规则
    pub fn pinyin_rules() -> Vec<FuzzyRule> {
        vec![
            FuzzyRule::new("z-zh".into(), "z".into(), "zh".into()),
            FuzzyRule::new("c-ch".into(), "c".into(), "ch".into()),
            FuzzyRule::new("s-sh".into(), "s".into(), "sh".into()),
            FuzzyRule::new("n-l".into(), "n".into(), "l".into()),
            FuzzyRule::new("l-n".into(), "l".into(), "n".into()),
            FuzzyRule::new("r-l".into(), "r".into(), "l".into()),
            FuzzyRule::new("an-ang".into(), "an".into(), "ang".into()),
            FuzzyRule::new("en-eng".into(), "en".into(), "eng".into()),
            FuzzyRule::new("in-ing".into(), "in".into(), "ing".into()),
        ]
    }

    /// 获取常见拼写错误规则
    pub fn typo_rules() -> Vec<FuzzyRule> {
        vec![
            FuzzyRule::new("ei-ie".into(), "ei".into(), "ie".into()),
            FuzzyRule::new("ui-iu".into(), "ui".into(), "iu".into()),
            FuzzyRule::new("un-ün".into(), "un".into(), "ün".into()),
        ]
    }

    /// 获取所有预定义规则
    pub fn all_rules() -> Vec<FuzzyRule> {
        let mut rules = Self::pinyin_rules();
        rules.extend(Self::typo_rules());
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_distance() {
        assert_eq!(FuzzyMatcher::edit_distance("", ""), 0);
        assert_eq!(FuzzyMatcher::edit_distance("abc", "abc"), 0);
        assert_eq!(FuzzyMatcher::edit_distance("abc", "ab"), 1);
        assert_eq!(FuzzyMatcher::edit_distance("abc", "def"), 3);
        assert_eq!(FuzzyMatcher::edit_distance("kitten", "sitting"), 3);
    }

    #[test]
    fn test_generate_variants_simple() {
        let matcher = FuzzyMatcher::pinyin_default();
        let variants = matcher.generate_variants("zongguo");
        // "z" 应该被替换为 "zh"，所以变体中应该包含 "zhongguo"
        assert!(variants.contains(&"zhongguo".to_string()));
    }

    #[test]
    fn test_generate_variants_disabled() {
        let config = FuzzyConfig {
            enabled: false,
            ..Default::default()
        };
        let matcher = FuzzyMatcher::new(config);
        let variants = matcher.generate_variants("zongguo");
        assert_eq!(variants, vec!["zongguo".to_string()]);
    }

    #[test]
    fn test_generate_variants_no_misalignment() {
        // 长度变化的替换（an→ang）在多个位置共存时不应错位
        let config = FuzzyConfig {
            rules: vec![FuzzyRule::new("an-ang".into(), "an".into(), "ang".into())],
            ..Default::default()
        };
        let matcher = FuzzyMatcher::new(config);
        let variants = matcher.generate_variants("banan");
        // "banan"：两处 an → ang，单步替换应正确生成二者，且不相互错位
        assert!(variants.contains(&"banang".to_string()));
        assert!(variants.contains(&"bangan".to_string()));
        // 原始输入仍在
        assert!(variants.contains(&"banan".to_string()));
    }

    #[test]
    fn test_is_match_exact() {
        let matcher = FuzzyMatcher::pinyin_default();
        assert!(matcher.is_match("wo", "wo"));
    }

    #[test]
    fn test_is_match_fuzzy() {
        let matcher = FuzzyMatcher::pinyin_default();
        // "zongguo" 通过 z→zh 规则变为 "zhongguo"
        assert!(matcher.is_match("zongguo", "zhongguo"));
    }

    #[test]
    fn test_is_match_edit_distance() {
        let config = FuzzyConfig {
            max_edit_distance: 1,
            ..Default::default()
        };
        let matcher = FuzzyMatcher::new(config);
        assert!(matcher.is_match("wo", "wwo")); // 编辑距离 1
        assert!(!matcher.is_match("wo", "www")); // 编辑距离 2
    }

    #[test]
    fn test_filter_candidates() {
        let matcher = FuzzyMatcher::pinyin_default();
        let candidates = vec!["zhongguo", "zhongyu", "zhongyao"];
        let results = matcher.filter_candidates("zongguo", &candidates, |s| s);
        // "zongguo" 通过 z→zh 规则变为 "zhongguo"，应匹配
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "zhongguo");
    }

    #[test]
    fn test_pinyin_rules_count() {
        let rules = FuzzyRulePresets::all_rules();
        assert!(rules.len() >= 9); // 至少有声母规则
    }
}
