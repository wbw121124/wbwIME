//! 分词模块
//!
//! 提供输入串的拼音分词功能，支持正向最大匹配和反向最大匹配。

/// 分词结果片段
#[derive(Debug, Clone)]
pub struct Segment {
    /// 分词文本（拼音音节）
    pub text: String,
    /// 起始位置
    pub start: usize,
    /// 结束位置
    pub end: usize,
    /// 词性（可选）
    pub pos: Option<String>,
    /// 词频（可选）
    pub freq: Option<u32>,
}

impl Segment {
    /// 创建新的分词结果
    pub fn new(text: String, start: usize, end: usize) -> Self {
        Self {
            text,
            start,
            end,
            pos: None,
            freq: None,
        }
    }

    /// 获取分词长度（字符数）
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl std::fmt::Display for Segment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}]", self.text)
    }
}

/// 分词器
///
/// 基于已知拼音音节表进行正向/反向最大匹配分词。
pub struct Segmenter {
    /// 是否启用歧义切分
    pub ambiguous_cut: bool,
    /// 是否返回词性
    pub return_pos: bool,
    /// 最大词长（音节数）
    pub max_word_len: usize,
}

impl Segmenter {
    /// 创建新的分词器
    pub fn new() -> Self {
        Self {
            ambiguous_cut: false,
            return_pos: false,
            max_word_len: 8,
        }
    }

    /// 启用歧义切分
    pub fn with_ambiguous_cut(mut self, enable: bool) -> Self {
        self.ambiguous_cut = enable;
        self
    }

    /// 启用词性返回
    pub fn with_return_pos(mut self, enable: bool) -> Self {
        self.return_pos = enable;
        self
    }

    /// 设置最大词长
    pub fn with_max_word_len(mut self, len: usize) -> Self {
        self.max_word_len = len;
        self
    }

    /// 正向最大匹配分词
    ///
    /// 从左到右扫描输入，每次取最长的合法音节作为分词。
    ///
    /// # 示例
    /// 输入 "zhongguo" → ["zhong", "guo"]
    /// 输入 "woshida" → ["wo", "shi", "da"]
    pub fn segment(&self, input: &str) -> Vec<Segment> {
        let mut segments = Vec::new();
        let mut pos = 0;
        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();

        while pos < len {
            let mut found = false;
            // 从长到短尝试匹配
            let max_len = std::cmp::min(self.max_word_len * 4, len - pos); // 大约 4 字符/音节
            for seg_len in (1..=max_len).rev() {
                let segment: String = chars[pos..pos + seg_len].iter().collect();
                if self.is_valid_pinyin_part(&segment) {
                    segments.push(Segment::new(segment, pos, pos + seg_len));
                    pos += seg_len;
                    found = true;
                    break;
                }
            }
            if !found {
                // 单字符无法匹配，按单字符处理
                let ch = chars[pos];
                segments.push(Segment::new(ch.to_string(), pos, pos + 1));
                pos += 1;
            }
        }

        segments
    }

    /// 反向最大匹配分词
    ///
    /// 从右到左扫描输入，每次取最长的合法音节作为分词。
    pub fn reverse_segment(&self, input: &str) -> Vec<Segment> {
        let chars: Vec<char> = input.chars().collect();
        let len = chars.len();
        let mut segments = Vec::new();
        let mut pos = len;

        while pos > 0 {
            let mut found = false;
            let max_len = std::cmp::min(self.max_word_len * 4, pos);
            for seg_len in (1..=max_len).rev() {
                let start = pos - seg_len;
                let segment: String = chars[start..pos].iter().collect();
                if self.is_valid_pinyin_part(&segment) {
                    segments.push(Segment::new(segment, start, pos));
                    pos = start;
                    found = true;
                    break;
                }
            }
            if !found {
                pos -= 1;
                segments.push(Segment::new(chars[pos].to_string(), pos, pos + 1));
            }
        }

        segments.reverse();
        segments
    }

    /// 双向分词（正向+反向取交集）
    ///
    /// 用于提高分词准确率，减少歧义。
    pub fn bidirectional_segment(&self, input: &str) -> Vec<Segment> {
        let forward = self.segment(input);
        let backward = self.reverse_segment(input);

        // 如果两种方式结果相同，直接返回
        if forward.len() == backward.len()
            && forward
                .iter()
                .zip(backward.iter())
                .all(|(a, b)| a.text == b.text)
        {
            return forward;
        }

        // 否则以正向结果为准（更常用）
        forward
    }

    /// 检查是否是合法的拼音音节
    fn is_valid_pinyin_part(&self, s: &str) -> bool {
        crate::pinyin::PinyinValidator::is_valid_syllable(s)
    }
}

impl Default for Segmenter {
    fn default() -> Self {
        Self::new()
    }
}

/// 分词结果合并工具
pub struct SegmentMerger;

impl SegmentMerger {
    /// 合并相邻的短分词（总长度 <= max_len 的相邻片段合并）
    pub fn merge_segments(segments: &[Segment], max_len: usize) -> Vec<Segment> {
        if segments.is_empty() {
            return Vec::new();
        }

        let mut merged = Vec::new();
        let mut current = segments[0].clone();

        for seg in &segments[1..] {
            let combined_len = current.len() + seg.len();
            if current.end == seg.start && combined_len <= max_len {
                current.text.push_str(&seg.text);
                current.end = seg.end;
            } else {
                merged.push(current);
                current = seg.clone();
            }
        }
        merged.push(current);
        merged
    }

    /// 去除重复分词
    pub fn deduplicate(segments: &[Segment]) -> Vec<Segment> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();

        for seg in segments {
            if seen.insert(seg.text.clone()) {
                result.push(seg.clone());
            }
        }
        result
    }

    /// 按位置排序
    pub fn sort_by_position(segments: &mut [Segment]) {
        segments.sort_by_key(|s| s.start);
    }
}

/// 分词统计信息
#[derive(Debug, Clone, Default)]
pub struct SegmentStats {
    /// 总分词数
    pub total_segments: usize,
    /// 平均分词长度（字符）
    pub avg_segment_len: f64,
    /// 最长分词
    pub max_segment_len: usize,
    /// 最短分词
    pub min_segment_len: usize,
}

/// 分词性能分析
pub struct SegmentProfiler;

impl SegmentProfiler {
    /// 分析分词统计
    pub fn analyze(segments: &[Segment]) -> SegmentStats {
        if segments.is_empty() {
            return SegmentStats::default();
        }

        let total = segments.len();
        let total_len: usize = segments.iter().map(|s| s.len()).sum();
        let max_len = segments.iter().map(|s| s.len()).max().unwrap_or(0);
        let min_len = segments.iter().map(|s| s.len()).min().unwrap_or(0);

        SegmentStats {
            total_segments: total,
            avg_segment_len: total_len as f64 / total as f64,
            max_segment_len: max_len,
            min_segment_len: min_len,
        }
    }

    /// 计算分词覆盖率
    ///
    /// 覆盖率 = 已分词字符数 / 总字符数
    pub fn coverage(input: &str, segments: &[Segment]) -> f64 {
        let total_len = input.len();
        if total_len == 0 {
            return 1.0;
        }

        let covered: usize = segments.iter().map(|s| s.len()).sum();
        covered as f64 / total_len as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_basic() {
        let seg = Segmenter::new();
        let result = seg.segment("woshida");
        assert!(!result.is_empty());
        assert_eq!(result[0].text, "wo");
        assert_eq!(result[1].text, "shi");
        assert_eq!(result[2].text, "da");
    }

    #[test]
    fn test_segment_long_syllable() {
        let seg = Segmenter::new();
        let result = seg.segment("zhongguo");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "zhong");
        assert_eq!(result[1].text, "guo");
    }

    #[test]
    fn test_reverse_segment() {
        let seg = Segmenter::new();
        let result = seg.reverse_segment("zhongguo");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "zhong");
        assert_eq!(result[1].text, "guo");
    }

    #[test]
    fn test_bidirectional_segment() {
        let seg = Segmenter::new();
        let result = seg.bidirectional_segment("zhongguo");
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_segment_single_syllable() {
        let seg = Segmenter::new();
        let result = seg.segment("wo");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text, "wo");
    }

    #[test]
    fn test_merge_segments() {
        let segments = vec![
            Segment::new("zh".into(), 0, 2),
            Segment::new("ong".into(), 2, 5),
            Segment::new("guo".into(), 5, 8),
        ];
        let merged = SegmentMerger::merge_segments(&segments, 6);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "zhong");
        assert_eq!(merged[1].text, "guo");
    }

    #[test]
    fn test_deduplicate() {
        let segments = vec![
            Segment::new("wo".into(), 0, 2),
            Segment::new("wo".into(), 5, 7),
            Segment::new("shi".into(), 2, 5),
        ];
        let deduped = SegmentMerger::deduplicate(&segments);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_profiler_analyze() {
        let segments = vec![
            Segment::new("wo".into(), 0, 2),
            Segment::new("shi".into(), 2, 5),
        ];
        let stats = SegmentProfiler::analyze(&segments);
        assert_eq!(stats.total_segments, 2);
        assert_eq!(stats.max_segment_len, 3);
        assert_eq!(stats.min_segment_len, 2);
    }
}
