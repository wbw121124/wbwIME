//! 候选数据结构模块

use serde::{Deserialize, Serialize};
use wbw_types::{Candidate, CandidateSource};

/// 候选词列表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateList {
    /// 候选词
    pub candidates: Vec<Candidate>,
    /// 当前页码
    pub page: usize,
    /// 每页数量
    pub page_size: usize,
    /// 总数量
    pub total: usize,
    /// 是否有下一页（必须与 page/total 保持同步，通过 next_page/prev_page/goto_page 更新）
    pub has_next: bool,
    /// 是否有上一页（必须与 page/total 保持同步，通过 next_page/prev_page/goto_page 更新）
    pub has_prev: bool,
}

impl CandidateList {
    /// 创建新的候选词列表
    pub fn new(candidates: Vec<Candidate>, page: usize, page_size: usize) -> Self {
        let total = candidates.len();
        let has_next = (page + 1) * page_size < total;
        let has_prev = page > 0;

        Self {
            candidates,
            page,
            page_size,
            total,
            has_next,
            has_prev,
        }
    }

    /// 获取当前页候选词
    pub fn current_page(&self) -> &[Candidate] {
        let len = self.candidates.len();
        if self.page_size == 0 || len == 0 {
            return &[];
        }
        let start = (self.page * self.page_size).min(len);
        let end = (start + self.page_size).min(len);
        &self.candidates[start..end]
    }

    /// 获取所有候选词
    pub fn all_candidates(&self) -> &[Candidate] {
        &self.candidates
    }

    /// 获取候选词数量
    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// 翻到下一页
    pub fn next_page(&mut self) -> bool {
        if self.has_next {
            self.page += 1;
            self.has_prev = true;
            self.has_next = (self.page + 1) * self.page_size < self.total;
            true
        } else {
            false
        }
    }

    /// 翻到上一页
    pub fn prev_page(&mut self) -> bool {
        if self.has_prev {
            self.page -= 1;
            self.has_next = true;
            self.has_prev = self.page > 0;
            true
        } else {
            false
        }
    }

    /// 跳转到指定页
    pub fn goto_page(&mut self, page: usize) -> bool {
        if page * self.page_size < self.total {
            self.page = page;
            self.has_next = (page + 1) * self.page_size < self.total;
            self.has_prev = page > 0;
            true
        } else {
            false
        }
    }
}

/// 候选词选择器
pub struct CandidateSelector {
    /// 候选词列表
    list: CandidateList,
    /// 当前选中索引
    selected: usize,
    /// 是否自动确认
    auto_confirm: bool,
    /// 自动确认阈值
    auto_confirm_threshold: f64,
}

impl CandidateSelector {
    /// 创建新的选择器
    pub fn new(list: CandidateList) -> Self {
        Self {
            list,
            selected: 0,
            auto_confirm: false,
            auto_confirm_threshold: 0.8,
        }
    }

    /// 设置自动确认
    pub fn with_auto_confirm(mut self, enable: bool, threshold: f64) -> Self {
        self.auto_confirm = enable;
        self.auto_confirm_threshold = threshold;
        self
    }

    /// 获取当前选中的候选词
    pub fn selected(&self) -> Option<&Candidate> {
        self.list.current_page().get(self.selected)
    }

    /// 选择下一个
    pub fn select_next(&mut self) -> bool {
        let page_candidates = self.list.current_page();
        if self.selected + 1 < page_candidates.len() {
            self.selected += 1;
            true
        } else if self.list.next_page() {
            self.selected = 0;
            true
        } else {
            false
        }
    }

    /// 选择上一个
    pub fn select_prev(&mut self) -> bool {
        if self.selected > 0 {
            self.selected -= 1;
            true
        } else if self.list.prev_page() {
            self.selected = self.list.current_page().len().saturating_sub(1);
            true
        } else {
            false
        }
    }

    /// 选择指定索引
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.list.current_page().len() {
            self.selected = index;
            true
        } else {
            false
        }
    }

    /// 确认选择
    pub fn confirm(&self) -> Option<&Candidate> {
        self.selected()
    }

    /// 检查是否应该自动确认
    pub fn should_auto_confirm(&self) -> bool {
        if let Some(candidate) = self.selected() {
            self.auto_confirm && candidate.score >= self.auto_confirm_threshold
        } else {
            false
        }
    }

    /// 获取列表引用
    pub fn list(&self) -> &CandidateList {
        &self.list
    }

    /// 获取当前选中索引
    pub fn selected_index(&self) -> usize {
        self.selected
    }
}

/// 候选词过滤器
pub struct CandidateFilter;

impl CandidateFilter {
    /// 按来源过滤
    pub fn by_source(candidates: &[Candidate], source: CandidateSource) -> Vec<&Candidate> {
        candidates.iter().filter(|c| c.source == source).collect()
    }

    /// 按分数过滤
    pub fn by_min_score(candidates: &[Candidate], min_score: f64) -> Vec<&Candidate> {
        candidates.iter().filter(|c| c.score >= min_score).collect()
    }

    /// 按最大数量过滤
    pub fn limit(candidates: &[Candidate], max_count: usize) -> Vec<&Candidate> {
        candidates.iter().take(max_count).collect()
    }

    /// 按 (text, code) 全局去重，保留首次出现的条目
    pub fn deduplicate(candidates: &mut Vec<Candidate>) {
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert((c.text.clone(), c.code.clone())));
    }

    /// 按 text 全局去重，保留首次出现的条目
    pub fn dedup_by_text(candidates: &mut Vec<Candidate>) {
        let mut seen = std::collections::HashSet::new();
        candidates.retain(|c| seen.insert(c.text.clone()));
    }

    /// 排序（按分数降序）
    pub fn sort_by_score(candidates: &mut [Candidate]) {
        candidates.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// 候选词转换器
#[allow(dead_code)]
pub struct CandidateConverter;

impl CandidateConverter {
    /// 转换为文本列表
    pub fn to_texts(candidates: &[Candidate]) -> Vec<String> {
        candidates.iter().map(|c| c.text.clone()).collect()
    }

    /// 转换为编码-文本映射
    pub fn to_code_text_map(candidates: &[Candidate]) -> Vec<(String, String)> {
        candidates
            .iter()
            .map(|c| (c.code.clone(), c.text.clone()))
            .collect()
    }

    /// 转换为分数映射
    pub fn to_score_map(candidates: &[Candidate]) -> Vec<(String, f64)> {
        candidates
            .iter()
            .map(|c| (c.text.clone(), c.score))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_candidates() -> Vec<Candidate> {
        vec![
            Candidate {
                text: "中国".into(),
                code: "zhongguo".into(),
                score: 100.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
            Candidate {
                text: "终于".into(),
                code: "zhongyu".into(),
                score: 50.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
        ]
    }

    #[test]
    fn test_candidate_list_pagination() {
        let candidates = test_candidates();
        let list = CandidateList::new(candidates, 0, 10);
        assert_eq!(list.len(), 2);
        assert!(!list.has_next);
        assert!(!list.has_prev);
    }

    #[test]
    fn test_candidate_selector() {
        let candidates = test_candidates();
        let list = CandidateList::new(candidates, 0, 10);
        let mut selector = CandidateSelector::new(list);
        assert!(selector.select_next());
        assert!(selector.selected().is_some());
    }

    #[test]
    fn test_candidate_filter() {
        let candidates = test_candidates();
        let filtered = CandidateFilter::by_min_score(&candidates, 60.0);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].text, "中国");
    }

    #[test]
    fn test_candidate_deduplicate() {
        let mut candidates = test_candidates();
        // 插入一个连续的重复项
        candidates.insert(1, candidates[0].clone());
        assert_eq!(candidates.len(), 3);
        CandidateFilter::deduplicate(&mut candidates);
        assert_eq!(candidates.len(), 2);
    }

    #[test]
    fn test_candidate_deduplicate_non_adjacent() {
        // 相同词条（text+code 完全一致）被非相同项隔开时也应去重，
        // 依赖"相邻去重"的实现会漏掉这种情况。
        let mut candidates = vec![
            Candidate {
                text: "最大流".into(),
                code: "zdl".into(),
                score: 100.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
            Candidate {
                text: "最短路".into(),
                code: "zdl".into(),
                score: 90.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
            Candidate {
                text: "最大流".into(),
                code: "zdl".into(),
                score: 60.0,
                source: CandidateSource::System,
                ngram_score: None,
                user_weight: None,
            },
        ];
        assert_eq!(candidates.len(), 3);
        CandidateFilter::deduplicate(&mut candidates);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].text, "最大流");
        assert_eq!(candidates[1].text, "最短路");
    }
}
