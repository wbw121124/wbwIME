pub mod fuzzy;
pub mod matcher;
pub mod pinyin;
pub mod segmenter;

// 重新导出常用类型
pub use fuzzy::{FuzzyConfig, FuzzyMatchResult, FuzzyMatcher, FuzzyRule, FuzzyRulePresets};
pub use matcher::{MatchOptions, MatchStrategy, Matcher, MatcherBuilder, MatcherConfig};
pub use pinyin::{PinyinString, PinyinSyllable, PinyinValidator, ToneMarker};
pub use segmenter::{Segment, SegmentStats, Segmenter};
