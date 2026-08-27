//! 拼音处理模块
//!
//! 提供拼音音节解析、验证、声调标记等功能。

use std::collections::HashMap;
use std::fmt;

/// 拼音声母表（按长度降序排列，优先匹配长声母）
static INITIALS: &[&str] = &[
    "zh", "ch", "sh", "b", "p", "m", "f", "d", "t", "n", "l", "g", "k", "h", "j", "q", "x", "r",
    "z", "c", "s", "y", "w",
];

/// 拼音韵母表
static FINALS: &[&str] = &[
    "ang", "eng", "ing", "ong", "ian", "iang", "iong", "uan", "uang", "ueng", "iao", "iou",
    "uei", "uen", "üan", "ün", "ai", "ei", "ao", "ou", "an", "en", "ia", "ie", "ua", "uo",
    "uai", "üe", "a", "o", "e", "i", "u", "ü", "er",
];

/// 有效拼音音节表（用于快速验证）
static VALID_SYLLABLES: &[&str] = &[
    "ba", "bo", "bai", "bei", "bao", "ban", "ben", "bang", "beng", "bi", "bie", "biao", "bian",
    "bin", "bing", "bu",
    "pa", "po", "pai", "pei", "pao", "pou", "pan", "pen", "pang", "peng", "pi", "pie", "piao",
    "pian", "pin", "ping", "pu",
    "ma", "mo", "me", "mai", "mei", "mao", "mou", "man", "men", "mang", "meng", "mi", "mie",
    "miao", "miu", "mian", "min", "ming", "mu",
    "fa", "fo", "fei", "fou", "fan", "fen", "fang", "feng", "fu",
    "da", "de", "dai", "dei", "dao", "dou", "dan", "den", "dang", "deng", "di", "die", "diao",
    "diu", "dian", "ding", "dong", "du", "duo", "duan", "dun",
    "ta", "te", "tai", "tei", "tao", "tou", "tan", "tang", "teng", "ti", "tie", "tiao", "tian",
    "ting", "tong", "tu", "tuo", "tuan", "tun",
    "na", "ne", "nai", "nei", "nao", "nou", "nan", "nen", "nang", "neng", "ni", "nie", "niao",
    "niu", "nian", "nin", "niang", "ning", "nong", "nu", "nuo", "nuan", "nün", "nüe",
    "la", "le", "lai", "lei", "lao", "lou", "lan", "len", "lang", "leng", "li", "lia", "lie",
    "liao", "liu", "lian", "lin", "liang", "ling", "long", "lu", "luo", "luan", "lun", "lü",
    "lüe",
    "ga", "ge", "gai", "gei", "gao", "gou", "gan", "gen", "gang", "geng", "gong", "gu", "gua",
    "guai", "gui", "guan", "gun", "guang", "guo",
    "ka", "ke", "kai", "kei", "kao", "kou", "kan", "ken", "kang", "keng", "kong", "ku", "kua",
    "kuai", "kui", "kuan", "kun", "kuang",
    "ha", "he", "hai", "hei", "hao", "hou", "han", "hen", "hang", "heng", "hong", "hu", "hua",
    "huai", "hui", "huan", "hun", "huang",
    "ji", "jia", "jie", "jiao", "jiu", "jian", "jin", "jiang", "jing", "jiong", "ju", "jue",
    "jun", "juan",
    "qi", "qia", "qie", "qiao", "qiu", "qian", "qin", "qiang", "qing", "qiong", "qu", "que",
    "qun", "quan",
    "xi", "xia", "xie", "xiao", "xiu", "xian", "xin", "xiang", "xing", "xiong", "xu", "xue",
    "xun", "xuan",
    "zhi", "zha", "zhe", "zhai", "zhei", "zhao", "zhou", "zhan", "zhen", "zhang", "zheng",
    "zhong", "zhu", "zhua", "zhuai", "zhui", "zhuan", "zhun", "zhuang",
    "chi", "cha", "che", "chai", "chao", "chou", "chan", "chen", "chang", "cheng", "chong",
    "chu", "chua", "chuai", "chui", "chuan", "chun", "chuang",
    "shi", "sha", "she", "shai", "shei", "shao", "shou", "shan", "shen", "shang", "sheng",
    "shu", "shua", "shuai", "shui", "shuan", "shun", "shuang",
    "ri", "re", "rao", "rou", "ran", "ren", "rang", "reng", "rong", "ru", "rua", "rui",
    "ruan", "run",
    "za", "ze", "zai", "zei", "zao", "zou", "zan", "zen", "zang", "zeng", "zong", "zu",
    "zuo", "zuan", "zun",
    "ca", "ce", "cai", "cao", "cou", "can", "cen", "cang", "ceng", "cong", "cu", "cuo",
    "cuan", "cun",
    "sa", "se", "sai", "sao", "sou", "san", "sen", "sang", "seng", "song", "su", "suo",
    "suan", "sun",
    "ya", "yo", "ye", "yao", "you", "yan", "yin", "yang", "ying", "yong", "yu", "yue", "yun",
    "yuan",
    "wa", "wo", "wai", "wei", "wan", "wen", "wang", "weng", "wu",
    "a", "o", "e", "ai", "ei", "ao", "ou", "an", "en", "ang", "eng", "er",
    "yi", "wu", "yu",
];

/// 拼音音节
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinyinSyllable {
    /// 声母
    pub initial: Option<String>,
    /// 韵母
    pub final_: String,
    /// 声调（1-4，0 表示无声调标记）
    pub tone: u8,
    /// 完整拼音（无声调）
    pub full: String,
}

impl PinyinSyllable {
    /// 创建新的拼音音节
    pub fn new(initial: Option<String>, final_: String, tone: u8) -> Self {
        let full = match &initial {
            Some(i) => format!("{}{}", i, final_),
            None => final_.clone(),
        };

        Self {
            initial,
            final_,
            tone,
            full,
        }
    }

    /// 从字符串解析（仅接受小写字母）
    pub fn from_str(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();
        if s.is_empty() {
            return None;
        }

        // 尝试匹配声母（优先匹配长声母）
        for initial in INITIALS {
            if s.starts_with(initial) {
                let rest = &s[initial.len()..];
                if !rest.is_empty() && FINALS.contains(&rest) {
                    return Some(Self::new(
                        Some(initial.to_string()),
                        rest.to_string(),
                        0,
                    ));
                }
            }
        }

        // 无声母（如 a, o, e, ai, ei 等）
        if VALID_SYLLABLES.contains(&s.as_str()) {
            return Some(Self::new(None, s, 0));
        }

        None
    }

    /// 获取不带声调的拼音
    pub fn without_tone(&self) -> &str {
        &self.full
    }

    /// 检查是否是有效拼音
    pub fn is_valid(&self) -> bool {
        VALID_SYLLABLES.contains(&self.full.as_str())
    }
}

impl fmt::Display for PinyinSyllable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full)
    }
}

/// 拼音字符串（多个音节）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinyinString {
    /// 音节列表
    pub syllables: Vec<PinyinSyllable>,
    /// 原始输入
    pub raw_input: String,
}

impl PinyinString {
    /// 创建新的拼音字符串
    pub fn new(raw_input: String) -> Self {
        Self {
            syllables: Vec::new(),
            raw_input,
        }
    }

    /// 解析输入字符串（贪心匹配：优先匹配长音节）
    pub fn parse(&mut self) -> bool {
        let input = self.raw_input.to_lowercase();
        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();
        let mut pos = 0;
        let mut syllables = Vec::new();

        while pos < len {
            let mut matched = false;
            // 尝试不同长度的子串（从长到短）
            let max_len = std::cmp::min(6, len - pos);
            for seg_len in (1..=max_len).rev() {
                let segment: String = chars[pos..pos + seg_len].iter().collect();
                if let Some(syllable) = PinyinSyllable::from_str(&segment) {
                    syllables.push(syllable);
                    pos += seg_len;
                    matched = true;
                    break;
                }
            }
            if !matched {
                // 无法解析，停止
                break;
            }
        }

        self.syllables = syllables;
        pos == len // 返回是否完全解析
    }

    /// 获取音节数量
    pub fn syllable_count(&self) -> usize {
        self.syllables.len()
    }

    /// 获取第一个音节
    pub fn first_syllable(&self) -> Option<&PinyinSyllable> {
        self.syllables.first()
    }

    /// 获取最后一个音节
    pub fn last_syllable(&self) -> Option<&PinyinSyllable> {
        self.syllables.last()
    }

    /// 转换为字符串（不带声调）
    pub fn to_plain_string(&self) -> String {
        self.syllables
            .iter()
            .map(|s| s.without_tone())
            .collect::<Vec<_>>()
            .join("")
    }

    /// 检查是否全部有效
    pub fn is_valid(&self) -> bool {
        !self.syllables.is_empty() && self.syllables.iter().all(|s| s.is_valid())
    }
}

/// 声调符号映射
fn tone_marked_vowel(vowel: char, tone: u8) -> Option<char> {
    match (vowel, tone) {
        // 一声（阴平）：ā ē ī ō ū ǖ
        ('a', 1) => Some('ā'),
        ('e', 1) => Some('ē'),
        ('i', 1) => Some('ī'),
        ('o', 1) => Some('ō'),
        ('u', 1) => Some('ū'),
        ('ü', 1) => Some('ǖ'),
        // 二声（阳平）：á é í ó ú ǘ
        ('a', 2) => Some('á'),
        ('e', 2) => Some('é'),
        ('i', 2) => Some('í'),
        ('o', 2) => Some('ó'),
        ('u', 2) => Some('ú'),
        ('ü', 2) => Some('ǘ'),
        // 三声（上声）：ǎ ě ǐ ǒ ǔ ǚ
        ('a', 3) => Some('ǎ'),
        ('e', 3) => Some('ě'),
        ('i', 3) => Some('ǐ'),
        ('o', 3) => Some('ǒ'),
        ('u', 3) => Some('ǔ'),
        ('ü', 3) => Some('ǚ'),
        // 四声（去声）：à è ì ò ù ǜ
        ('a', 4) => Some('à'),
        ('e', 4) => Some('è'),
        ('i', 4) => Some('ì'),
        ('o', 4) => Some('ò'),
        ('u', 4) => Some('ù'),
        ('ü', 4) => Some('ǜ'),
        _ => None,
    }
}

/// 标记声调的主要元音位置
fn find_main_vowel(s: &str) -> Option<(usize, char)> {
    // a, e, o 优先；没有则找 i, u, ü 中最后出现的
    for (i, ch) in s.char_indices() {
        if ch == 'a' || ch == 'e' || ch == 'o' {
            return Some((i, ch));
        }
    }
    let mut last = None;
    for (i, ch) in s.char_indices() {
        if ch == 'i' || ch == 'u' || ch == 'ü' {
            last = Some((i, ch));
        }
    }
    last
}

/// 拼音声调标记工具
pub struct ToneMarker;

impl ToneMarker {
    /// 标记声调（带声调符号），如 "mao" + tone=2 → "máo"
    pub fn mark_tone(pinyin: &str, tone: u8) -> String {
        if tone == 0 || tone > 4 {
            return pinyin.to_string();
        }

        if let Some((pos, vowel)) = find_main_vowel(pinyin) {
            if let Some(marked) = tone_marked_vowel(vowel, tone) {
                let mut result = String::new();
                for (i, ch) in pinyin.char_indices() {
                    if i == pos {
                        result.push(marked);
                    } else {
                        result.push(ch);
                    }
                }
                return result;
            }
        }
        pinyin.to_string()
    }

    /// 移除声调标记
    pub fn remove_tone(pinyin: &str) -> String {
        let tone_map: HashMap<char, char> = [
            ('ā', 'a'), ('á', 'a'), ('ǎ', 'a'), ('à', 'a'),
            ('ē', 'e'), ('é', 'e'), ('ě', 'e'), ('è', 'e'),
            ('ī', 'i'), ('í', 'i'), ('ǐ', 'i'), ('ì', 'i'),
            ('ō', 'o'), ('ó', 'o'), ('ǒ', 'o'), ('ò', 'o'),
            ('ū', 'u'), ('ú', 'u'), ('ǔ', 'u'), ('ù', 'u'),
            ('ǖ', 'ü'), ('ǘ', 'ü'), ('ǚ', 'ü'), ('ǜ', 'ü'),
        ]
        .iter()
        .cloned()
        .collect();

        pinyin
            .chars()
            .map(|ch| tone_map.get(&ch).copied().unwrap_or(ch))
            .collect()
    }
}

/// 拼音有效性检查
pub struct PinyinValidator;

impl PinyinValidator {
    /// 检查是否是有效声母
    pub fn is_valid_initial(s: &str) -> bool {
        INITIALS.contains(&s)
    }

    /// 检查是否是有效韵母
    pub fn is_valid_final(s: &str) -> bool {
        FINALS.contains(&s)
    }

    /// 检查是否是有效拼音音节
    pub fn is_valid_syllable(s: &str) -> bool {
        let s = s.to_lowercase();
        VALID_SYLLABLES.contains(&s.as_str())
    }

    /// 检查是否是有效拼音（可能是多个音节拼接）
    pub fn is_valid_pinyin(s: &str) -> bool {
        let s = s.to_lowercase();
        if s.is_empty() {
            return false;
        }
        // 尝试将字符串拆分为有效音节
        Self::can_split_into_syllables(&s)
    }

    /// 递归检查是否可以拆分为有效音节
    fn can_split_into_syllables(s: &str) -> bool {
        if s.is_empty() {
            return true;
        }
        // 尝试不同长度的前缀
        let max_len = std::cmp::min(6, s.len());
        for len in (1..=max_len).rev() {
            if let Some(prefix) = s.get(..len) {
                if VALID_SYLLABLES.contains(&prefix) {
                    if Self::can_split_into_syllables(&s[len..]) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_syllable_from_str() {
        assert!(PinyinSyllable::from_str("ba").is_some());
        assert!(PinyinSyllable::from_str("zhong").is_some());
        assert!(PinyinSyllable::from_str("shi").is_some());
        assert!(PinyinSyllable::from_str("xyz").is_none());
    }

    #[test]
    fn test_pinyin_syllable_components() {
        let s = PinyinSyllable::from_str("zhong").unwrap();
        assert_eq!(s.initial, Some("zh".to_string()));
        assert_eq!(s.final_, "ong");
        assert_eq!(s.full, "zhong");

        let s = PinyinSyllable::from_str("a").unwrap();
        assert_eq!(s.initial, None);
        assert_eq!(s.final_, "a");
    }

    #[test]
    fn test_pinyin_string_parse() {
        let mut ps = PinyinString::new("zhongguo".to_string());
        assert!(ps.parse());
        assert_eq!(ps.syllable_count(), 2);
        assert_eq!(ps.syllables[0].full, "zhong");
        assert_eq!(ps.syllables[1].full, "guo");

        let mut ps = PinyinString::new("woshida".to_string());
        assert!(ps.parse());
        assert_eq!(ps.syllable_count(), 3); // wo, shi, da
    }

    #[test]
    fn test_tone_marker() {
        assert_eq!(ToneMarker::mark_tone("ma", 1), "mā");
        assert_eq!(ToneMarker::mark_tone("ma", 2), "má");
        assert_eq!(ToneMarker::mark_tone("ma", 3), "mǎ");
        assert_eq!(ToneMarker::mark_tone("ma", 4), "mà");
        assert_eq!(ToneMarker::mark_tone("zhong", 1), "zhōng");
    }

    #[test]
    fn test_remove_tone() {
        assert_eq!(ToneMarker::remove_tone("mā"), "ma");
        assert_eq!(ToneMarker::remove_tone("zhōng"), "zhong");
    }

    #[test]
    fn test_validator() {
        assert!(PinyinValidator::is_valid_syllable("ba"));
        assert!(PinyinValidator::is_valid_syllable("zhong"));
        assert!(!PinyinValidator::is_valid_syllable("xyz"));
        assert!(PinyinValidator::is_valid_initial("zh"));
        assert!(!PinyinValidator::is_valid_initial("abc"));
        assert!(PinyinValidator::is_valid_final("ong"));
    }

    #[test]
    fn test_valid_syllables_coverage() {
        // 确保常用音节都在列表中
        let common = ["wo", "ni", "ta", "hao", "shi", "de", "le", "bu", "ai", "da"];
        for s in common {
            assert!(
                PinyinValidator::is_valid_syllable(s),
                "音节 {} 应该是有效的",
                s
            );
        }
    }
}
