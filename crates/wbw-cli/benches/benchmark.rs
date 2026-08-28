//! wbwIME 性能基准测试

use criterion::{criterion_group, criterion_main, Criterion};

const TEST_DICT_PATH: &str = "../../resources/dicts/cs-oi.cin";

fn build_dict() -> wbw_dict::FstDict {
    let mut builder = wbw_dict::DictBuilder::new();
    builder
        .load_cin(std::path::Path::new(TEST_DICT_PATH))
        .unwrap();
    builder.deduplicate();
    builder.sort();
    builder.build_fst()
}

fn bench_dict_load(c: &mut Criterion) {
    c.bench_function("dict_load", |b| b.iter(build_dict));
}

fn bench_exact_lookup(c: &mut Criterion) {
    let dict = build_dict();
    c.bench_function("exact_lookup_zdl", |b| b.iter(|| dict.lookup("zdl")));
}

fn bench_prefix_lookup(c: &mut Criterion) {
    let dict = build_dict();
    c.bench_function("prefix_lookup_z", |b| {
        b.iter(|| dict.prefix_lookup("z"));
    });
}

fn bench_fuzzy_lookup(c: &mut Criterion) {
    let dict = build_dict();
    c.bench_function("fuzzy_lookup_zdlu", |b| {
        b.iter(|| dict.fuzzy_lookup("zdlu", 1));
    });
}

fn bench_matcher_match(c: &mut Criterion) {
    let dict = build_dict();
    let config = wbw_matcher::MatcherConfig::default();
    let mut matcher = wbw_matcher::Matcher::with_dict(config, dict);
    let ctx = wbw_types::InputContext {
        buffer: "zdl".to_string(),
        cursor: 0,
        mode: wbw_types::InputMode::Pinyin,
        selected: Vec::new(),
        session_id: 0,
    };
    c.bench_function("matcher_match_zdl", |b| {
        b.iter(|| matcher.match_input(&ctx));
    });
}

fn bench_ranker(c: &mut Criterion) {
    let dict = build_dict();
    let config = wbw_matcher::MatcherConfig::default();
    let mut matcher = wbw_matcher::Matcher::with_dict(config, dict);
    let ctx = wbw_types::InputContext {
        buffer: "zdl".to_string(),
        cursor: 0,
        mode: wbw_types::InputMode::Pinyin,
        selected: Vec::new(),
        session_id: 0,
    };
    let candidates = matcher.match_input(&ctx);
    let rank_config = wbw_types::RankConfig::default();
    let ranker = wbw_rank::Ranker::new(rank_config);
    c.bench_function("ranker_rank", |b| {
        b.iter(|| ranker.rank(candidates.clone()));
    });
}

fn bench_fuzzy_generate_variants(c: &mut Criterion) {
    let fuzzy = wbw_matcher::FuzzyMatcher::pinyin_default();
    c.bench_function("fuzzy_generate_variants", |b| {
        b.iter(|| fuzzy.generate_variants("zongguo"));
    });
}

fn bench_cin_parse(c: &mut Criterion) {
    c.bench_function("cin_parse", |b| {
        b.iter(|| {
            let parser = wbw_dict::CinParser::new(TEST_DICT_PATH);
            parser.parse().unwrap()
        });
    });
}

criterion_group!(
    benches,
    bench_dict_load,
    bench_exact_lookup,
    bench_prefix_lookup,
    bench_fuzzy_lookup,
    bench_matcher_match,
    bench_ranker,
    bench_fuzzy_generate_variants,
    bench_cin_parse,
);

criterion_main!(benches);
