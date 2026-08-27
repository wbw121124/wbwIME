# API 参考

## wbw-types

共享类型定义 crate，所有其他 crate 的基础依赖。

### 核心类型

```rust
/// 候选词数据结构
pub struct Candidate {
    pub text: String,           // 词文本
    pub code: String,           // 编码（如拼音）
    pub score: f64,             // 词频分数
    pub source: CandidateSource, // 来源标识
    pub ngram_score: Option<f64>,   // N-gram 评分
    pub user_weight: Option<f64>,   // 用户权重
}

/// 候选词来源
pub enum CandidateSource {
    System,     // 系统词典
    User,       // 用户词典
    Dynamic,    // 动态学习词典
    Phrase,     // 短语/固定词组
}

/// 输入上下文
pub struct InputContext {
    pub buffer: String,         // 输入缓冲区
    pub cursor: usize,          // 光标位置
    pub mode: InputMode,        // 输入模式
    pub selected: Vec<String>,  // 已选择的候选词
    pub session_id: u64,        // 会话 ID
}

/// 输入模式
pub enum InputMode {
    Pinyin,     // 拼音输入
    Wubi,       // 五笔输入
    English,    // 英文输入
    Symbol,     // 符号输入
}
```

### 全局配置

```rust
pub struct GlobalConfig {
    pub dict: DictConfig,       // 词典配置
    pub matcher: MatcherConfig, // 匹配器配置
    pub rank: RankConfig,       // 排序配置
    pub l0: L0Config,           // L0 学习配置
    pub ngram: NgramConfig,     // N-gram 配置
}

pub struct RankConfig {
    pub pin_weight: f64,        // 拼音匹配权重
    pub user_weight: f64,       // 用户词库权重
    pub freq_weight: f64,       // 词频权重
    pub ngram_weight: f64,      // N-gram 权重
    pub max_candidates: usize,  // 最大候选词数量
}
```

---

## wbw-dict

词典模块，负责 .cin 码表解析和 FST 词典管理。

### CinParser

```rust
/// .cin 解析器
pub struct CinParser { ... }

impl CinParser {
    /// 创建新的解析器
    pub fn new(path: &str) -> Self;

    /// 设置编码格式
    pub fn with_encoding(self, encoding: &str) -> Self;

    /// 设置是否跳过注释行
    pub fn with_skip_comments(self, skip: bool) -> Self;

    /// 解析码表文件
    pub fn parse(&self) -> ImeResult<Vec<CinEntry>>;

    /// 从字符串解析
    pub fn parse_str(&self, content: &str) -> ImeResult<Vec<CinEntry>>;
}

/// 批量解析多个 .cin 文件
pub fn parse_multiple(paths: &[&str]) -> ImeResult<Vec<CinEntry>>;

/// 合并多个码表条目（按编码分组）
pub fn merge_entries(entries: Vec<CinEntry>) -> Vec<CinEntry>;
```

### FstDict

```rust
/// FST 词典
pub struct FstDict { ... }

impl FstDict {
    /// 从文件加载词典
    pub fn from_file(path: &Path) -> ImeResult<Self>;

    /// 从内存加载词典
    pub fn from_memory(data: Vec<u8>, source: DictSource) -> ImeResult<Self>;

    /// 查询编码
    pub fn lookup(&self, code: &str) -> ImeResult<Vec<DictEntry>>;

    /// 模糊查询
    pub fn fuzzy_lookup(
        &self,
        code: &str,
        max_edit_distance: usize,
    ) -> ImeResult<Vec<DictEntry>>;

    /// 获取词条数量
    pub fn entry_count(&self) -> usize;

    /// 获取编码数量
    pub fn code_count(&self) -> usize;
}

/// 合并两个词典
pub fn merge_dicts(dict1: &FstDict, dict2: &FstDict) -> ImeResult<FstDict>;
```

### DictBuilder

```rust
/// 词典构建器
pub struct DictBuilder { ... }

impl DictBuilder {
    /// 创建新的构建器
    pub fn new() -> Self;

    /// 使用配置创建构建器
    pub fn with_config(config: DictBuilderConfig) -> Self;

    /// 添加词条
    pub fn add_entry(&mut self, entry: DictEntry);

    /// 从 .cin 文件加载
    pub fn load_cin(&mut self, path: &Path) -> ImeResult<()>;

    /// 构建 FST 词典
    pub fn build_fst(self) -> ImeResult<FstDict>;

    /// 构建并保存到文件
    pub fn build_and_save(&self, path: &Path) -> ImeResult<()>;
}
```

---

## wbw-matcher

匹配模块，处理拼音解析、分词和模糊匹配。

### PinyinSyllable

```rust
/// 拼音音节
pub struct PinyinSyllable {
    pub initial: Option<String>, // 声母
    pub final_: String,          // 韵母
    pub tone: u8,                // 声调（1-4，0=轻声）
    pub full: String,            // 完整拼音
}

impl PinyinSyllable {
    /// 从字符串解析
    pub fn from_str(s: &str) -> ImeResult<Self>;

    /// 获取不带声调的拼音
    pub fn without_tone(&self) -> &str;

    /// 检查是否是有效拼音
    pub fn is_valid(&self) -> bool;
}
```

### FuzzyMatcher

```rust
/// 模糊匹配器
pub struct FuzzyMatcher { ... }

impl FuzzyMatcher {
    /// 创建新的模糊匹配器
    pub fn new(config: FuzzyConfig) -> Self;

    /// 从规则列表创建
    pub fn from_rules(rules: Vec<FuzzyRule>) -> Self;

    /// 生成所有可能的变体
    pub fn generate_variants(&self, input: &str) -> Vec<String>;

    /// 计算编辑距离
    pub fn edit_distance(&self, s1: &str, s2: &str) -> usize;

    /// 检查是否匹配
    pub fn is_match(&self, input: &str, target: &str) -> bool;
}

/// 预定义模糊规则
pub struct FuzzyRulePresets;

impl FuzzyRulePresets {
    /// 获取拼音模糊规则
    pub fn pinyin_rules() -> Vec<FuzzyRule>;

    /// 获取常见拼写错误规则
    pub fn typo_rules() -> Vec<FuzzyRule>;
}
```

### Matcher

```rust
/// 匹配器
pub struct Matcher { ... }

impl Matcher {
    /// 创建新的匹配器
    pub fn new(config: MatcherConfig) -> Self;

    /// 匹配输入
    pub fn match_input(
        &mut self,
        context: &InputContext,
    ) -> ImeResult<MatchResult>;

    /// 精确匹配
    pub fn exact_match(&self, code: &str) -> ImeResult<Vec<Candidate>>;

    /// 前缀匹配
    pub fn prefix_match(&self, code: &str) -> ImeResult<Vec<Candidate>>;

    /// 模糊匹配
    pub fn fuzzy_match(&self, code: &str) -> ImeResult<Vec<Candidate>>;
}

/// 匹配器构建器
pub struct MatcherBuilder { ... }

impl MatcherBuilder {
    pub fn new() -> Self;
    pub fn with_fuzzy(self, enabled: bool) -> Self;
    pub fn with_fuzzy_rules(self, rules: Vec<FuzzyRule>) -> Self;
    pub fn with_max_candidates(self, max: usize) -> Self;
    pub fn build(self) -> Matcher;
}
```

---

## wbw-ngram

N-gram 语言模型模块。

### NgramScorer

```rust
/// N-gram 评分器
pub struct NgramScorer { ... }

impl NgramScorer {
    /// 创建新的评分器
    pub fn new(config: ScorerConfig) -> Self;

    /// 从文件加载
    pub fn from_file(config: ScorerConfig, path: &Path) -> ImeResult<Self>;

    /// 设置概率表
    pub fn with_table(self, table: NgramTable) -> Self;

    /// 评分单个词
    pub fn score_word(
        &self,
        context: &[&str],
        word: &str,
    ) -> ImeResult<f64>;

    /// 评分序列
    pub fn score_sequence(&self, words: &[&str]) -> ImeResult<f64>;

    /// 计算困惑度
    pub fn perplexity(&self, test_data: &[&str]) -> ImeResult<f64>;
}
```

### Smoother

```rust
/// 平滑处理器
pub struct Smoother { ... }

impl Smoother {
    /// 使用拉普拉斯平滑
    pub fn laplace(count: f64, total: f64, alpha: f64) -> f64;

    /// 使用加k平滑
    pub fn add_k(count: f64, total: f64, k: f64) -> f64;

    /// 使用 Good-Turing 平滑
    pub fn good_turing(count: u64, freq_counts: &[u64]) -> f64;

    /// 插值平滑
    pub fn interpolation(
        high_order_prob: f64,
        low_order_prob: f64,
        lambda: f64,
    ) -> f64;
}
```

---

## wbw-core

核心模块，管理会话、上下文和候选词。

### SessionManager

```rust
/// 会话管理器
pub struct SessionManager { ... }

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new() -> Self;

    /// 创建新会话
    pub fn create_session(&mut self) -> u64;

    /// 创建带配置的会话
    pub fn create_session_with_config(
        &mut self,
        config: SessionConfig,
    ) -> u64;

    /// 获取会话状态
    pub fn get_session(&self, id: u64) -> Option<&SessionState>;

    /// 关闭会话
    pub fn close_session(&mut self, id: u64) -> bool;

    /// 获取所有活跃会话
    pub fn active_sessions(&self) -> Vec<u64>;
}
```

### ContextManager

```rust
/// 上下文管理器
pub struct ContextManager { ... }

impl ContextManager {
    /// 创建新的上下文管理器
    pub fn new(session_id: u64) -> Self;

    /// 添加字符到缓冲区
    pub fn push_char(&mut self, ch: char);

    /// 删除缓冲区末尾字符
    pub fn pop_char(&mut self) -> Option<char>;

    /// 清空缓冲区
    pub fn clear_buffer(&mut self);

    /// 设置输入模式
    pub fn set_mode(&mut self, mode: InputMode);

    /// 获取缓冲区内容
    pub fn buffer(&self) -> &str;

    /// 撤销操作
    pub fn undo(&mut self) -> bool;
}
```

### CandidateList

```rust
/// 候选词列表
pub struct CandidateList { ... }

impl CandidateList {
    /// 创建新的候选词列表
    pub fn new(
        candidates: Vec<Candidate>,
        page: usize,
        page_size: usize,
    ) -> Self;

    /// 获取当前页候选词
    pub fn current_page(&self) -> &[Candidate];

    /// 翻到下一页
    pub fn next_page(&mut self) -> bool;

    /// 翻到上一页
    pub fn prev_page(&mut self) -> bool;

    /// 获取总页数
    pub fn total_pages(&self) -> usize;
}
```

---

## wbw-rank

排序模块，候选词加权排序和动态学习。

### Ranker

```rust
/// 排序器
pub struct Ranker { ... }

impl Ranker {
    /// 创建新的排序器
    pub fn new(config: RankConfig) -> Self;

    /// 排序候选词
    pub fn rank(
        &self,
        candidates: Vec<Candidate>,
    ) -> ImeResult<Vec<Candidate>>;

    /// 记录用户选择（用于 L0 学习）
    pub fn record_selection(&mut self, code: &str, word: &str);

    /// 获取配置
    pub fn config(&self) -> &RankConfig;
}

/// 排序器构建器
pub struct RankerBuilder { ... }

impl RankerBuilder {
    pub fn new() -> Self;
    pub fn with_config(self, config: RankConfig) -> Self;
    pub fn with_cache_size(self, size: usize) -> Self;
    pub fn build(self) -> Ranker;
}
```

### L0Learner

```rust
/// L0 学习器
pub struct L0Learner { ... }

impl L0Learner {
    /// 创建新的学习器
    pub fn new(config: L0Config) -> Self;

    /// 从快照加载
    pub fn from_snapshot(
        config: L0Config,
        path: &Path,
    ) -> ImeResult<Self>;

    /// 记录用户选择
    pub fn record_selection(&mut self, code: &str, word: &str);

    /// 检查是否达到学习阈值
    pub fn should_learn(&self, code: &str, word: &str) -> bool;

    /// 获取学习建议
    pub fn get_suggestions(&self) -> Vec<LearningSuggestion>;

    /// 保存快照
    pub fn save_snapshot(&self) -> ImeResult<()>;
}
```

### WeightCalculator

```rust
/// 权重计算器
pub struct WeightCalculator { ... }

impl WeightCalculator {
    /// 创建新的权重计算器
    pub fn new(config: RankConfig) -> Self;

    /// 计算候选词权重
    pub fn calculate_weight(&self, candidate: &Candidate) -> f64;

    /// 批量计算权重
    pub fn calculate_weights(
        &self,
        candidates: &[Candidate],
    ) -> Vec<(Candidate, f64)>;
}
```

---

## wbw-imekit

IME 宿主适配层。

### ImeHost

```rust
/// IME 宿主
pub struct ImeHost { ... }

impl ImeHost {
    /// 创建新的 IME 宿主
    pub fn new(config: ImeConfig) -> Self;

    /// 初始化 IME
    pub fn initialize(&mut self) -> ImeResult<()>;

    /// 处理按键事件
    pub fn process_key(
        &mut self,
        key: KeyEvent,
    ) -> ImeResult<ImeResponse>;

    /// 输入字符
    pub fn input_char(
        &mut self,
        ch: char,
    ) -> ImeResult<ImeResponse>;

    /// 确认输入
    pub fn confirm(&mut self) -> ImeResult<ImeResponse>;

    /// 选择候选词
    pub fn select_candidate(
        &mut self,
        index: usize,
    ) -> ImeResult<ImeResponse>;

    /// 切换输入模式
    pub fn switch_mode(&mut self, mode: InputMode) -> ImeResult<()>;

    /// 重置 IME 状态
    pub fn reset(&mut self);
}

/// IME 工厂
pub struct ImeFactory;

impl ImeFactory {
    /// 创建默认 IME
    pub fn create_default() -> ImeHost;

    /// 创建拼音 IME
    pub fn create_pinyin() -> ImeHost;

    /// 创建五笔 IME
    pub fn create_wubi() -> ImeHost;
}
```

### CandidateWindow

```rust
/// 候选窗口
pub struct CandidateWindow { ... }

impl CandidateWindow {
    /// 创建新的候选窗口
    pub fn new(
        position: WindowPosition,
        style: WindowStyle,
    ) -> Self;

    /// 设置候选词
    pub fn set_candidates(&mut self, candidates: Vec<Candidate>);

    /// 显示窗口
    pub fn show(&mut self) -> ImeResult<()>;

    /// 隐藏窗口
    pub fn hide(&mut self) -> ImeResult<()>;

    /// 选择下一个
    pub fn select_next(&mut self) -> bool;

    /// 选择上一个
    pub fn select_prev(&mut self) -> bool;

    /// 获取当前选中的候选词
    pub fn selected_candidate(&self) -> Option<&Candidate>;

    /// 翻到下一页
    pub fn next_page(&mut self) -> bool;

    /// 翻到上一页
    pub fn prev_page(&mut self) -> bool;
}
```
