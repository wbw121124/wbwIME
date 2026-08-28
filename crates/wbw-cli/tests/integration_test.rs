//! wbwIME 集成测试
//!
//! 覆盖完整输入流程：词典 → 匹配 → 排序 → N-gram → 会话 → IME 宿主。

use wbw_core::candidate::CandidateList;
use wbw_core::context::ContextManager;
use wbw_core::error::ErrorRecovery;
use wbw_core::session::SessionManager;
use wbw_dict::DictBuilder;
use wbw_imekit::{ImeConfig, ImeHost, KeyAction, KeyEvent, KeyMapper};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_ngram::{NgramScorer, NgramTableBuilder, ScorerConfig};
use wbw_rank::{L0Learner, RankConfigManager, Ranker};
use wbw_types::{Candidate, CandidateSource, InputContext, InputMode, L0Config, RankConfig};

/// 测试用 .cin 内容
const TEST_CIN: &str = "\
wo 我
wo 喔
ni 你
hao 好
shi 是
zhongguo 中国
zhonguo 终于
ni 你好
";

/// 构造一个简单词典（内存构建）
fn build_test_dict() -> wbw_dict::FstDict {
    let mut builder = DictBuilder::new();
    builder.load_cin_str(TEST_CIN).unwrap();
    builder.deduplicate();
    builder.sort();
    builder.build_fst()
}

/// 测试词典加载（通过 DictBuilder 从字符串构建）
#[test]
fn test_dict_load() {
    let dict = build_test_dict();
    assert!(dict.entry_count() > 0);
    assert_eq!(dict.lookup("wo").len(), 2); // 我、喔
    assert_eq!(dict.lookup("ni").len(), 2); // 你、你好
}

/// 测试 .cin 解析
#[test]
fn test_cin_parsing() {
    let parser = wbw_dict::CinParser::new("_");
    let entries = parser.parse_str(TEST_CIN).unwrap();
    assert!(!entries.is_empty());
    // 找到 wo
    let wo = entries
        .iter()
        .find(|e| e.code == "wo")
        .expect("wo 编码存在");
    assert_eq!(wo.words.len(), 2);
}

/// 测试拼音匹配（精确匹配）
#[test]
fn test_pinyin_match() {
    let dict = build_test_dict();
    let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
    let exact = matcher.exact_lookup("wo");
    assert!(!exact.is_empty());
    let has_wo = exact.iter().any(|c| c.text == "我");
    assert!(has_wo, "应包含 '我'");
}

/// 测试模糊匹配
#[test]
fn test_fuzzy_match() {
    let dict = build_test_dict();
    let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
    let fuzzy = matcher.fuzzy_lookup("w");
    // 模糊匹配不应报错
    assert!(!fuzzy.iter().any(|c| c.text.is_empty()));
}

/// 测试候选词排序
#[test]
fn test_candidate_ranking() {
    let ranker = Ranker::new(RankConfig::default());
    let candidates = vec![
        Candidate {
            text: "我".into(),
            code: "wo".into(),
            score: 100.0,
            source: CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        },
        Candidate {
            text: "喔".into(),
            code: "wo".into(),
            score: 10.0,
            source: CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        },
    ];
    let ranked = ranker.rank(candidates);
    assert_eq!(ranked.len(), 2);
    // 排序后第一个应是高分项
    assert_eq!(ranked[0].text, "我");
}

/// 测试 N-gram 评分
#[test]
fn test_ngram_scoring() {
    let mut builder = NgramTableBuilder::new(2);
    builder.from_sentences(&[
        vec!["我".to_string(), "是".to_string(), "中国".to_string()],
        vec!["我".to_string(), "爱".to_string(), "你".to_string()],
    ]);
    let table = builder.build();

    let scorer = NgramScorer::new(ScorerConfig::default()).with_table(table);
    let score = scorer.score_word(&[], "我");
    assert!(score.is_finite(), "N-gram 分数应为有限值");
}

/// 测试会话管理
#[test]
fn test_session_management() {
    let mut manager = SessionManager::new();
    let id = manager.create_session();
    assert!(manager.get_session(id).is_some());
    assert_eq!(manager.active_count(), 1);
    assert!(manager.close_session(id));
    assert_eq!(manager.active_count(), 0);
}

/// 测试输入上下文
#[test]
fn test_input_context() {
    let mut ctx = ContextManager::new(1);
    ctx.push_char('w');
    ctx.push_char('o');
    assert_eq!(ctx.buffer(), "wo");
    assert!(ctx.pop_char().is_some());
    assert_eq!(ctx.buffer(), "w");
    ctx.set_mode(InputMode::Pinyin);
    assert_eq!(ctx.mode(), InputMode::Pinyin);
}

/// 测试候选词列表
#[test]
fn test_candidate_list() {
    let candidates = (0..7)
        .map(|i| Candidate {
            text: format!("词{}", i),
            code: "code".into(),
            score: i as f64,
            source: CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        })
        .collect();
    let mut list = CandidateList::new(candidates, 0, 3);
    assert_eq!(list.len(), 7);
    assert_eq!(list.current_page().len(), 3);
    assert!(list.next_page());
    assert!(list.next_page());
    assert!(!list.next_page()); // 已到最后一页
}

/// 测试按键映射
#[test]
fn test_key_mapping() {
    let mapper = KeyMapper::new();
    // 回车应映射为确认
    let action = mapper.process_key(&KeyEvent::new(13, None));
    assert!(matches!(action, Some(KeyAction::Confirm)));
    // 退格
    let backspace = mapper.process_key(&KeyEvent::new(8, None));
    assert!(matches!(backspace, Some(KeyAction::DeleteChar)));
}

/// 测试完整输入流程（词典 → 匹配 → 排序）
#[test]
fn test_full_input_flow() {
    let dict = build_test_dict();
    let mut matcher = Matcher::with_dict(MatcherConfig::default(), dict);
    let ctx = InputContext {
        buffer: "wo".to_string(),
        cursor: 2,
        mode: InputMode::Pinyin,
        selected: Vec::new(),
        session_id: 1,
    };
    let candidates = matcher.match_input(&ctx);
    assert!(!candidates.is_empty(), "输入 'wo' 应有候选词");
    assert!(candidates.iter().any(|c| c.text == "我"));
}

/// 测试错误处理
#[test]
fn test_error_handling() {
    use wbw_core::error::{CoreError, RecoveryStrategy};
    let err = CoreError::ConfigError("测试错误".to_string());
    // Abort 策略应返回错误
    let abort: Result<i32, _> = ErrorRecovery::try_recover(&err, RecoveryStrategy::Abort, || Ok(0));
    assert!(abort.is_err());
    // Fallback 策略应返回默认值
    let fallback: Result<i32, _> =
        ErrorRecovery::try_recover(&err, RecoveryStrategy::Fallback, || Ok(42));
    assert_eq!(fallback.unwrap(), 42);
}

/// 测试配置加载（RankConfig）
#[test]
fn test_config_loading() {
    let config = RankConfig {
        pin_weight: 100.0,
        user_weight: 10.0,
        freq_weight: 1.0,
        ngram_weight: 0.5,
        max_candidates: 10,
    };
    let manager = RankConfigManager::from_memory(config);
    assert_eq!(manager.config().max_candidates, 10);
    // 验证通过
    assert!(wbw_rank::ConfigValidator::validate(manager.config()).is_ok());
}

/// 测试 L0 学习
#[test]
fn test_l0_learning() {
    let config = L0Config::default();
    let mut learner = L0Learner::new(config);
    learner.record_selection("wo", "我");
    learner.record_selection("wo", "我");
    assert!(!learner.should_learn("wo", "我")); // 默认阈值 3，还没到
    learner.record_selection("wo", "我");
    assert!(learner.should_learn("wo", "我"));
    assert_eq!(learner.data_count(), 3);
}

/// 测试 IME 宿主基本流程
#[test]
fn test_ime_host_flow() {
    let mut host = ImeHost::new(ImeConfig::default());
    host.initialize().unwrap();
    // 输入字符
    host.input_char('w').unwrap();
    host.input_char('o').unwrap();
    assert_eq!(host.buffer(), "wo");
    assert!(host.is_inputting());
    // 确认
    host.confirm().unwrap();
}

/// 测试性能（简单耗时断言）
#[test]
fn test_performance_baseline() {
    let dict = build_test_dict();
    let matcher = Matcher::with_dict(MatcherConfig::default(), dict);
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        matcher.exact_lookup("wo");
        matcher.prefix_lookup("w");
    }
    let elapsed = start.elapsed();
    // 1000 次精确+前缀查询应在一个合理时间内完成（宽松界限）
    assert!(elapsed.as_millis() < 5000, "查询应快速完成: {:?}", elapsed);
}
