//! N-gram 概率表模块
//!
//! 提供 N-gram 语言模型的概率存储和查询功能。

use smallvec::SmallVec;
use std::collections::HashMap;

/// N-gram 概率表
///
/// 使用 HashMap 存储 N-gram 计数，支持条件概率查询。
pub struct NgramTable {
    /// N-gram 阶数
    order: usize,
    /// N-gram 计数：(context, word) → count
    counts: HashMap<(Vec<String>, String), u64>,
    /// 上下文计数：context → count
    context_counts: HashMap<Vec<String>, u64>,
    /// 词条数量
    entry_count: usize,
    /// 词汇表大小
    vocab_size: usize,
}

impl NgramTable {
    /// 创建空表
    pub fn new(order: usize) -> Self {
        Self {
            order,
            counts: HashMap::new(),
            context_counts: HashMap::new(),
            entry_count: 0,
            vocab_size: 0,
        }
    }

    /// 从计数数据构建
    pub fn from_counts(order: usize, counts: HashMap<(Vec<String>, String), u64>) -> Self {
        let mut context_counts: HashMap<Vec<String>, u64> = HashMap::new();
        let mut vocab: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut entry_count: u64 = 0;

        for ((context, word), count) in &counts {
            *context_counts.entry(context.clone()).or_insert(0) += count;
            vocab.insert(word.clone());
            entry_count += count;
        }

        Self {
            order,
            counts,
            context_counts,
            entry_count: entry_count as usize,
            vocab_size: vocab.len(),
        }
    }

    /// 查询 P(word | context)
    ///
    /// 使用最大似然估计：P(w|c) = count(c,w) / count(c)
    ///
    /// 注意：`lookup`/`conditional_probability`/`count` 为高频调用路径。
    /// 上下文通常很小（1~2 项），因此使用 `SmallVec` 内联构建以避免堆分配。
    pub fn lookup(&self, context: &[&str], word: &str) -> f64 {
        let ctx: SmallVec<[String; 2]> = context.iter().map(|s| s.to_string()).collect();
        let key = (ctx.to_vec(), word.to_string());

        let ngram_count = self.counts.get(&key).copied().unwrap_or(0);
        let ctx_count = self.context_counts.get(ctx.as_slice()).copied().unwrap_or(0);

        if ctx_count == 0 {
            0.0
        } else {
            ngram_count as f64 / ctx_count as f64
        }
    }

    /// 查询条件概率（带 Laplace 平滑）
    pub fn conditional_probability(&self, context: &[&str], word: &str) -> f64 {
        let ctx: SmallVec<[String; 2]> = context.iter().map(|s| s.to_string()).collect();
        let key = (ctx.to_vec(), word.to_string());

        let ngram_count = self.counts.get(&key).copied().unwrap_or(0);
        let ctx_count = self
            .context_counts
            .get(ctx.as_slice())
            .copied()
            .unwrap_or(0);

        // Laplace 平滑：(count + 1) / (context_count + vocab_size)
        let vocab = self.vocab_size.max(1) as f64;
        (ngram_count as f64 + 1.0) / (ctx_count as f64 + vocab)
    }

    /// 获取 N-gram 计数
    pub fn count(&self, context: &[&str], word: &str) -> u64 {
        let ctx: SmallVec<[String; 2]> = context.iter().map(|s| s.to_string()).collect();
        self.counts
            .get(&(ctx.to_vec(), word.to_string()))
            .copied()
            .unwrap_or(0)
    }

    /// 获取阶数
    pub fn order(&self) -> usize {
        self.order
    }

    /// 获取词条数量
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// 获取词汇表大小
    pub fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    /// 检查表是否为空
    pub fn is_empty(&self) -> bool {
        self.entry_count == 0
    }

    /// 获取表统计信息
    pub fn stats(&self) -> TableStats {
        let unique_contexts = self.context_counts.len();
        let avg = if unique_contexts > 0 {
            self.entry_count as f64 / unique_contexts as f64
        } else {
            0.0
        };

        TableStats {
            total_ngrams: self.counts.len(),
            unique_contexts,
            unique_words: self.vocab_size,
            avg_words_per_context: avg,
        }
    }
}

/// 表统计信息
#[derive(Debug, Clone, Default)]
pub struct TableStats {
    pub total_ngrams: usize,
    pub unique_contexts: usize,
    pub unique_words: usize,
    pub avg_words_per_context: f64,
}

/// N-gram 表构建器
pub struct NgramTableBuilder {
    order: usize,
    counts: HashMap<(Vec<String>, String), u64>,
    min_count: u64,
}

impl NgramTableBuilder {
    pub fn new(order: usize) -> Self {
        Self {
            order,
            counts: HashMap::new(),
            min_count: 1,
        }
    }

    pub fn with_min_count(mut self, min_count: u64) -> Self {
        self.min_count = min_count;
        self
    }

    /// 添加 N-gram 计数
    pub fn add_count(&mut self, context: Vec<String>, word: String, count: u64) {
        *self.counts.entry((context, word)).or_insert(0) += count;
    }

    /// 从句子列表构建
    ///
    /// 每个句子是一个词序列，自动提取 N-gram 计数。
    pub fn from_sentences(&mut self, sentences: &[Vec<String>]) {
        for sentence in sentences {
            if sentence.len() < self.order {
                continue;
            }
            for i in 0..=sentence.len() - self.order {
                let context = sentence[i..i + self.order - 1].to_vec();
                let word = sentence[i + self.order - 1].clone();
                *self.counts.entry((context, word)).or_insert(0) += 1;
            }
        }
    }

    /// 构建表（过滤低频 N-gram）
    pub fn build(self) -> NgramTable {
        let counts: HashMap<(Vec<String>, String), u64> = self
            .counts
            .into_iter()
            .filter(|(_, count)| *count >= self.min_count)
            .collect();

        NgramTable::from_counts(self.order, counts)
    }

    pub fn count_entries(&self) -> usize {
        self.counts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_table() -> NgramTable {
        let mut builder = NgramTableBuilder::new(2);
        // "我 爱 中国" 的 bigram
        builder.add_count(vec!["我".into()], "爱".into(), 10);
        builder.add_count(vec!["爱".into()], "中国".into(), 8);
        builder.add_count(vec!["我".into()], "是".into(), 5);
        builder.add_count(vec!["是".into()], "好人".into(), 3);
        builder.build()
    }

    #[test]
    fn test_lookup() {
        let table = build_test_table();
        assert_eq!(table.lookup(&["我"], "爱"), 10.0 / 15.0);
        assert_eq!(table.lookup(&["我"], "是"), 5.0 / 15.0);
        assert_eq!(table.lookup(&["我"], "不"), 0.0);
    }

    #[test]
    fn test_conditional_probability() {
        let table = build_test_table();
        let prob = table.conditional_probability(&["我"], "爱");
        assert!(prob > 0.0);
        assert!(prob <= 1.0);
    }

    #[test]
    fn test_count() {
        let table = build_test_table();
        assert_eq!(table.count(&["我"], "爱"), 10);
        assert_eq!(table.count(&["我"], "是"), 5);
        assert_eq!(table.count(&["我"], "不"), 0);
    }

    #[test]
    fn test_stats() {
        let table = build_test_table();
        let stats = table.stats();
        assert!(stats.total_ngrams > 0);
        assert!(stats.unique_contexts > 0);
    }

    #[test]
    fn test_from_sentences() {
        let mut builder = NgramTableBuilder::new(2);
        let sentences = vec![
            vec!["我".into(), "爱".into(), "你".into()],
            vec!["我".into(), "爱".into(), "中国".into()],
        ];
        builder.from_sentences(&sentences);
        let table = builder.build();
        assert_eq!(table.count(&["我"], "爱"), 2);
    }

    #[test]
    fn test_min_count_filter() {
        let mut builder = NgramTableBuilder::new(2).with_min_count(5);
        builder.add_count(vec!["a".into()], "b".into(), 10);
        builder.add_count(vec!["c".into()], "d".into(), 2); // 低于阈值
        let table = builder.build();
        assert_eq!(table.count(&["a"], "b"), 10);
        assert_eq!(table.count(&["c"], "d"), 0); // 被过滤
    }
}
