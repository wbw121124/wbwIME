#![allow(unused_imports, dead_code, unreachable_code)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::path::Path;

const TEST_DICT_PATH: &str = "resources/dicts/base.cin";

fn bench_dict_load(c: &mut Criterion) {
    let dict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(TEST_DICT_PATH);
    if !dict_path.exists() {
        return;
    }
    c.bench_function("dict_load_cin", |b| {
        b.iter(|| {
            let parser = wbw_dict::CinParser::new(dict_path.to_str().unwrap());
            black_box(parser.parse());
        });
    });
}

fn bench_pinyin_match(c: &mut Criterion) {
    c.bench_function("pinyin_is_valid_pinyin", |b| {
        b.iter(|| {
            black_box(wbw_matcher::PinyinValidator::is_valid_pinyin(black_box(
                "zhongguorenmin",
            )));
        });
    });
}

fn bench_fuzzy_match(c: &mut Criterion) {
    let matcher = wbw_matcher::FuzzyMatcher::pinyin_default();
    c.bench_function("fuzzy_generate_variants", |b| {
        b.iter(|| {
            black_box(matcher.generate_variants(black_box("zongguo")));
        });
    });
}

fn bench_candidate_ranking(c: &mut Criterion) {
    let candidates = vec![
        wbw_types::Candidate {
            text: "中国".into(),
            code: "zhongguo".into(),
            score: 100.0,
            source: wbw_types::CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        },
        wbw_types::Candidate {
            text: "终于".into(),
            code: "zhongyu".into(),
            score: 50.0,
            source: wbw_types::CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        },
        wbw_types::Candidate {
            text: "钟".into(),
            code: "zhong".into(),
            score: 80.0,
            source: wbw_types::CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        },
    ];
    let ranker = wbw_rank::Ranker::new(wbw_types::RankConfig::default());
    c.bench_function("candidate_ranking_3", |b| {
        b.iter(|| {
            black_box(ranker.rank(black_box(&candidates)));
        });
    });
}

fn bench_ngram_scoring(c: &mut Criterion) {
    use smallvec::SmallVec;
    let mut builder = wbw_ngram::NgramTableBuilder::new(2);
    builder.add_count(SmallVec::from_iter(["我".into()]), "爱".into(), 10);
    builder.add_count(SmallVec::from_iter(["爱".into()]), "中国".into(), 8);
    builder.add_count(SmallVec::from_iter(["我".into()]), "是".into(), 5);
    let table = builder.build();
    let scorer =
        wbw_ngram::NgramScorer::new(wbw_ngram::ScorerConfig::default()).with_table(table);
    c.bench_function("ngram_score_sequence", |b| {
        b.iter(|| {
            black_box(scorer.score_sequence(black_box(&["我", "爱", "中国"])));
        });
    });
}

fn bench_session_management(c: &mut Criterion) {
    c.bench_function("session_create_close", |b| {
        b.iter(|| {
            let mut mgr = wbw_core::SessionManager::new();
            let id = mgr.create_session();
            black_box(mgr.close_session(id));
        });
    });
}

fn bench_input_context(c: &mut Criterion) {
    c.bench_function("context_push_8_chars", |b| {
        b.iter(|| {
            let mut ctx = wbw_core::ContextManager::new(1);
            for ch in "zhongguo".chars() {
                ctx.push_char(ch);
            }
            black_box(ctx.buffer());
        });
    });
}

fn bench_candidate_window(c: &mut Criterion) {
    let candidates: Vec<wbw_types::Candidate> = (0..100)
        .map(|i| wbw_types::Candidate {
            text: format!("词{}", i),
            code: format!("ci{}", i),
            score: (100 - i) as f64,
            source: wbw_types::CandidateSource::System,
            ngram_score: None,
            user_weight: None,
        })
        .collect();
    c.bench_function("candidate_list_page_10", |b| {
        b.iter(|| {
            let list = wbw_core::CandidateList::new(candidates.clone(), 0, 10);
            black_box(list.current_page());
        });
    });
}

fn bench_key_mapping(c: &mut Criterion) {
    let segmenter = wbw_matcher::Segmenter::new();
    c.bench_function("segmenter_segment", |b| {
        b.iter(|| {
            black_box(segmenter.segment(black_box("zhongguo")));
        });
    });
}

fn bench_full_input_flow(c: &mut Criterion) {
    let dict_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(TEST_DICT_PATH);
    if !dict_path.exists() {
        return;
    }
    let parser = wbw_dict::CinParser::new(dict_path.to_str().unwrap());
    let cin_entries = parser.parse().unwrap();
    let mut builder = wbw_dict::FstDictBuilder::new();
    for cin_entry in &cin_entries {
        for word_entry in &cin_entry.words {
            builder.add_entry(wbw_dict::DictEntry {
                code: cin_entry.code.clone(),
                word: word_entry.word.clone(),
                freq: word_entry.freq,
                source: wbw_dict::DictSource::Base,
            });
        }
    }
    let dict = builder.build(wbw_dict::DictSource::Base).unwrap();
    let mut matcher =
        wbw_matcher::Matcher::with_dict(wbw_matcher::MatcherConfig::default(), dict);

    c.bench_function("full_match_input", |b| {
        b.iter(|| {
            let ctx = wbw_types::InputContext {
                buffer: "zhongguo".to_string(),
                cursor: 0,
                mode: wbw_types::InputMode::Pinyin,
                selected: Vec::new(),
                session_id: 0,
            };
            black_box(matcher.match_input(black_box(&ctx)));
        });
    });
}

fn bench_memory_usage(c: &mut Criterion) {
    c.bench_function("dict_build_1000", |b| {
        b.iter(|| {
            let mut b = wbw_dict::FstDictBuilder::new();
            for i in 0..1000 {
                b.add_entry(wbw_dict::DictEntry {
                    code: format!("code{}", i),
                    word: format!("word{}", i),
                    freq: i as u32,
                    source: wbw_dict::DictSource::Base,
                });
            }
            black_box(b.build(wbw_dict::DictSource::Base).unwrap());
        });
    });
}

fn bench_concurrent_performance(c: &mut Criterion) {
    let matcher = wbw_matcher::FuzzyMatcher::pinyin_default();
    let inputs = ["zongguo", "woshi", "zhongyu"];
    c.bench_function("fuzzy_is_match_x3", |b| {
        b.iter(|| {
            for input in &inputs {
                black_box(matcher.is_match(black_box(input), "zhongguo"));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_dict_load,
    bench_pinyin_match,
    bench_fuzzy_match,
    bench_candidate_ranking,
    bench_ngram_scoring,
    bench_session_management,
    bench_input_context,
    bench_candidate_window,
    bench_key_mapping,
    bench_full_input_flow,
    bench_memory_usage,
    bench_concurrent_performance,
);

criterion_main!(benches);
