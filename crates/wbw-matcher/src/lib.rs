pub mod fuzzy;
pub mod segmenter;
pub mod pinyin;
pub mod matcher;

// 重新导出常用类型
pub use fuzzy::{FuzzyRule, FuzzyConfig, FuzzyMatcher, FuzzyMatchResult, FuzzyRulePresets};
pub use segmenter::{Segment, Segmenter, SegmentStats};
pub use pinyin::{PinyinSyllable, PinyinString, ToneMarker, PinyinValidator};
pub use matcher::{Matcher, MatcherConfig, MatcherBuilder, MatchStrategy, MatchOptions};