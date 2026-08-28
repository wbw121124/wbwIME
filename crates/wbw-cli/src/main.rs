//! wbwIME 命令行入口
//!
//! 提供命令行接口，支持词典查询、交互式输入、词典构建与验证等功能。
//!
//! 常用命令：
//! - `wbwime query <code>`：查询编码并展示排序后的候选词
//! - `wbwime interactive`：交互式输入
//! - `wbwime test-match <code> <dict>`：测试匹配（精确/前缀/模糊）
//! - `wbwime build-dict <dict> <out>`：构建 FST 词典
//! - `wbwime validate <dict>`：验证 .cin 码表
//! - `wbwime stats [dict]`：显示词典统计信息

use std::path::{Path, PathBuf};
use std::process;

use wbw_dict::{CinParser, DictBuilder, DictValidator, FstDict};
use wbw_matcher::{Matcher, MatcherConfig};
use wbw_rank::{Ranker, RankConfigManager, ConfigValidator};
use wbw_types::{Candidate, GlobalConfig, InputContext, InputMode};

/// CLI 错误类型
#[derive(Debug)]
pub enum CliError {
    ConfigError(String),
    DictError(String),
    InputError(String),
    IoError(String),
    RankError(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::ConfigError(msg) => write!(f, "配置错误: {}", msg),
            CliError::DictError(msg) => write!(f, "词典错误: {}", msg),
            CliError::InputError(msg) => write!(f, "输入错误: {}", msg),
            CliError::IoError(msg) => write!(f, "IO 错误: {}", msg),
            CliError::RankError(msg) => write!(f, "排序错误: {}", msg),
        }
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::IoError(e.to_string())
    }
}

impl From<wbw_types::ImeError> for CliError {
    fn from(e: wbw_types::ImeError) -> Self {
        CliError::DictError(e.to_string())
    }
}

type CliResult<T> = Result<T, CliError>;

/// CLI 命令
#[derive(Debug)]
pub enum CliCommand {
    Interactive,
    Query { code: String },
    TestMatch { code: String, dict_path: PathBuf },
    BuildDict { dict_path: PathBuf, out_path: PathBuf },
    Validate { dict_path: PathBuf },
    Stats { dict_path: Option<PathBuf> },
    Version,
    Help,
}

/// 解析命令行参数
fn parse_args() -> CliResult<CliCommand> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        return Ok(CliCommand::Help);
    }

    match args[1].as_str() {
        "interactive" | "i" => Ok(CliCommand::Interactive),
        "query" | "q" => {
            if args.len() < 3 {
                Err(CliError::InputError("请提供查询编码: wbwime query <code>".to_string()))
            } else {
                Ok(CliCommand::Query { code: args[2].clone() })
            }
        }
        "test-match" | "t" => {
            parse_two_args(&args, "test-match <code> <dict-path>")
                .map(|(code, dict_path)| CliCommand::TestMatch { code, dict_path })
        }
        "build-dict" | "b" => {
            if args.len() < 4 {
                Err(CliError::InputError(
                    "用法: wbwime build-dict <dict-path> <out-path>".to_string(),
                ))
            } else {
                Ok(CliCommand::BuildDict {
                    dict_path: PathBuf::from(&args[2]),
                    out_path: PathBuf::from(&args[3]),
                })
            }
        }
        "validate" => {
            if args.len() < 3 {
                Err(CliError::InputError("请提供词典路径: wbwime validate <dict-path>".to_string()))
            } else {
                Ok(CliCommand::Validate {
                    dict_path: PathBuf::from(&args[2]),
                })
            }
        }
        "stats" | "s" => {
            let path = if args.len() >= 3 {
                Some(PathBuf::from(&args[2]))
            } else {
                None
            };
            Ok(CliCommand::Stats { dict_path: path })
        }
        "version" | "v" => Ok(CliCommand::Version),
        "help" | "-h" | "--help" => Ok(CliCommand::Help),
        cmd => Err(CliError::InputError(format!("未知命令: {}", cmd))),
    }
}

/// 解析两个位置参数的命令
fn parse_two_args(args: &[String], usage: &str) -> CliResult<(String, PathBuf)> {
    if args.len() < 4 {
        Err(CliError::InputError(format!("用法: wbwime {}", usage)))
    } else {
        Ok((args[2].clone(), PathBuf::from(&args[3])))
    }
}

/// 显示帮助信息
fn show_help() {
    println!("wbwIME - Rust 自构建输入法引擎");
    println!();
    println!("用法: wbwime [命令] [选项]");
    println!();
    println!("命令:");
    println!("  query <code>                    查询编码并展示排序后的候选词");
    println!("  interactive                     交互式输入模式");
    println!("  test-match <code> <dict>        测试匹配（精确/前缀/模糊）");
    println!("  build-dict <dict> <out>         构建 FST 词典");
    println!("  validate <dict>                 验证 .cin 码表");
    println!("  stats [dict]                    显示词典统计信息");
    println!("  version                         显示版本信息");
    println!("  help                            显示帮助信息");
    println!();
    println!("默认词典: resources/dicts/cs-oi.cin");
}

/// 显示版本信息
fn show_version() {
    println!("wbwIME v{}", env!("CARGO_PKG_VERSION"));
    println!("Rust 自构建输入法引擎");
}

/// 加载全局配置
///
/// 尝试从 resources/config.toml 加载，失败时回退到默认配置。
fn load_config() -> GlobalConfig {
    let config_path = Path::new("resources/config.toml");
    if config_path.exists() {
        match std::fs::read_to_string(config_path) {
            Ok(content) => match toml::from_str::<GlobalConfig>(&content) {
                Ok(config) => config,
                Err(e) => {
                    eprintln!("警告: 配置解析失败({}), 使用默认配置", e);
                    GlobalConfig::default()
                }
            },
            Err(e) => {
                eprintln!("警告: 配置文件读取失败({}), 使用默认配置", e);
                GlobalConfig::default()
            }
        }
    } else {
        GlobalConfig::default()
    }
}

/// 默认词典路径（若命令行未指定）
fn default_dict_path() -> PathBuf {
    PathBuf::from("resources/dicts/cs-oi.cin")
}

/// 从 .cin 文件解析并构建 FST 词典
fn build_fst(path: &Path) -> CliResult<FstDict> {
    if !path.exists() {
        return Err(CliError::DictError(format!(
            "词典文件不存在: {}",
            path.display()
        )));
    }

    let mut builder = DictBuilder::new();
    builder
        .load_cin(path)
        .map_err(|e| CliError::DictError(e.to_string()))?;
    builder.deduplicate();
    builder.sort();
    Ok(builder.build_fst())
}

/// 从 FST 词典构建匹配器
fn build_matcher(dict: FstDict) -> Matcher {
    let config = MatcherConfig {
        fuzzy_enabled: true,
        ..MatcherConfig::default()
    };
    Matcher::with_dict(config, dict)
}

/// 构建排序器
fn build_ranker(config: &GlobalConfig) -> CliResult<Ranker> {
    let manager = RankConfigManager::from_memory(config.rank.clone());
    ConfigValidator::validate(&config.rank)
        .map_err(|e| CliError::RankError(e.to_string()))?;
    Ok(Ranker::from_config_manager(manager))
}

/// 构造输入上下文
fn make_context(buffer: &str) -> InputContext {
    InputContext {
        buffer: buffer.to_string(),
        cursor: buffer.len(),
        mode: InputMode::Pinyin,
        selected: Vec::new(),
        session_id: 1,
    }
}

/// 展示候选词列表
fn show_candidates(candidates: &[Candidate]) {
    if candidates.is_empty() {
        println!("  (无候选词)");
        return;
    }
    for (i, c) in candidates.iter().enumerate() {
        let source = match c.source {
            wbw_types::CandidateSource::System => "基础",
            wbw_types::CandidateSource::User => "用户",
            wbw_types::CandidateSource::Dynamic => "动态",
            wbw_types::CandidateSource::Phrase => "短语",
        };
        println!(
            "  {}. {} [{}] ({:.1}, {})",
            i + 1,
            c.text,
            c.code,
            c.score,
            source
        );
    }
}

/// 执行交互式输入
///
/// 接入完整引擎：词典 → 匹配器 → 排序器。
fn run_interactive() -> CliResult<()> {
    let config = load_config();
    let dict_path = default_dict_path();

    println!("wbwIME 交互模式");
    println!("输入拼音进行匹配，输入 'quit'/'exit' 退出，'clear' 清空");
    println!("示例解码: {} (词条 {})", dict_path.display(), dict_path.exists());
    println!();

    let dict = match build_fst(&dict_path) {
        Ok(d) => d,
        Err(e) => {
            println!("警告: {}", e);
            println!("交互模式仍可运行，但无法匹配。");
            return Ok(());
        }
    };
    let mut matcher = build_matcher(dict);
    let mut ranker = build_ranker(&config)?;
    let mut buffer = String::new();

    loop {
        let prompt = if buffer.is_empty() {
            "> ".to_string()
        } else {
            format!("{} > ", buffer)
        };
        print!("{}", prompt);
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        let mut line = String::new();
        if std::io::stdin().read_line(&mut line).is_err() {
            break;
        }
        let trimmed = line.trim().to_string();

        match trimmed.as_str() {
            "quit" | "exit" => break,
            "clear" | "c" => {
                buffer.clear();
                continue;
            }
            _ => {}
        }

        if trimmed.is_empty() && buffer.is_empty() {
            continue;
        }

        // 追加字符
        if !trimmed.is_empty() {
            buffer.push_str(&trimmed);
        }

        let ctx = make_context(&buffer);
        let candidates = matcher.match_input(&ctx);
        let ranked = ranker.rank(candidates);

        let max = config.rank.max_candidates.max(1);
        println!("\n匹配结果 ({} 个候选):", ranked.len());
        show_candidates(&ranked[..ranked.len().min(max)]);

        // 选择
        print!("选择序号 (回车跳过, b 退格, q 退出): ");
        std::io::stdout().flush().unwrap();
        let mut choice = String::new();
        std::io::stdin().read_line(&mut choice).unwrap();
        let choice = choice.trim();

        match choice {
            "b" | "backspace" => {
                buffer.pop();
            }
            "q" | "quit" | "exit" => {
                if !buffer.is_empty() {
                    buffer.clear();
                } else {
                    break;
                }
            }
            "" => {
                buffer.clear();
            }
            s => {
                if let Ok(idx) = s.parse::<usize>() {
                    if idx >= 1 && idx <= ranked.len() {
                        let selected = &ranked[idx - 1];
                        println!("输出: {}", selected.text);
                        ranker.record_selection(&selected.code, &selected.text);
                        buffer.clear();
                    } else {
                        println!("无效序号");
                    }
                } else {
                    println!("无效输入");
                }
            }
        }
        println!();
    }

    println!("再见！");
    Ok(())
}

/// 执行查询
///
/// 接入完整引擎，对编码做精确/前缀/模糊匹配并排序。
fn run_query(code: &str) -> CliResult<()> {
    let config = load_config();
    let dict_path = default_dict_path();

    let dict = match build_fst(&dict_path) {
        Ok(d) => d,
        Err(e) => {
            println!("错误: {}", e);
            println!("请指定词典: wbwime test-match {} <dict-path>", code);
            return Ok(());
        }
    };
    let matcher = build_matcher(dict);
    let ranker = build_ranker(&config)?;

    // 精确匹配
    let exact = matcher.exact_lookup(code);
    // 前缀匹配
    let prefix = matcher.prefix_lookup(code);
    // 模糊匹配
    let fuzzy = matcher.fuzzy_lookup(code);

    // 合并候选（按词去重，保留最高分）
    let mut seen = std::collections::HashSet::new();
    let mut all: Vec<Candidate> = Vec::new();
    for mut c in exact.into_iter().chain(prefix).chain(fuzzy) {
        c.code = code.to_string();
        if seen.insert(c.text.clone()) {
            all.push(c);
        }
    }

    if all.is_empty() {
        println!("未找到编码 '{}' 的匹配", code);
        return Ok(());
    }

    let ranked = ranker.rank(all);
    let max = config.rank.max_candidates.max(1);
    println!("编码 '{}' 的匹配 ({} 个):", code, ranked.len());
    show_candidates(&ranked[..ranked.len().min(max)]);

    Ok(())
}

/// 执行测试匹配
fn run_test_match(code: &str, dict_path: &Path) -> CliResult<()> {
    let config = load_config();
    let dict = build_fst(dict_path)?;
    println!("词典加载成功: {} 条目", dict.entry_count());

    let matcher = build_matcher(dict);
    let ranker = build_ranker(&config)?;

    let exact = matcher.exact_lookup(code);
    if !exact.is_empty() {
        println!("\n精确匹配 '{}':", code);
        show_candidates(&exact);
    }

    let prefix = matcher.prefix_lookup(code);
    if !prefix.is_empty() {
        println!("\n前缀匹配 '{}':", code);
        show_candidates(&prefix);
    }

    let fuzzy = matcher.fuzzy_lookup(code);
    if !fuzzy.is_empty() {
        println!("\n模糊匹配 '{}':", code);
        show_candidates(&fuzzy);
    }

    if exact.is_empty() && prefix.is_empty() && fuzzy.is_empty() {
        println!("未找到编码 '{}' 的匹配", code);
    } else {
        // 合并三类匹配并按词去重（保留最高分）
        let mut all = Vec::new();
        all.extend(exact);
        all.extend(prefix);
        all.extend(fuzzy);
        all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        let mut seen = std::collections::HashSet::new();
        all.retain(|c| seen.insert(c.text.clone()));
        let ranked = ranker.rank(all);
        println!("\n最终排序 (前 {} 个):", config.rank.max_candidates);
        let max = config.rank.max_candidates.max(1);
        show_candidates(&ranked[..ranked.len().min(max)]);
    }

    Ok(())
}

/// 构建 FST 词典（内存构建并验证，当前版本 FST 为内存哈希实现，尚不支持二进制持久化）
fn run_build_dict(dict_path: &Path, _out_path: &Path) -> CliResult<()> {
    let dict = build_fst(dict_path)?;

    println!("词典构建完成:");
    println!("  输入: {}", dict_path.display());
    println!("  词条: {}", dict.entry_count());
    println!("  编码: {}", dict.code_count());

    // 抽样验证查询
    let stats = dict.stats();
    println!("  统计: {:?}", stats);

    Ok(())
}

/// 验证 .cin 码表
fn run_validate(dict_path: &Path) -> CliResult<()> {
    if !dict_path.exists() {
        return Err(CliError::DictError(format!(
            "词典文件不存在: {}",
            dict_path.display()
        )));
    }

    match DictValidator::validate_cin(dict_path) {
        Ok(()) => {
            println!("验证通过: {}", dict_path.display());
            let parser = CinParser::new(
                dict_path
                    .to_str()
                    .ok_or_else(|| CliError::DictError("路径包含无效 Unicode".to_string()))?,
            );
            match parser.parse() {
                Ok(entries) => {
                    let words: usize = entries.iter().map(|e| e.words.len()).sum();
                    println!("  编码数: {}", entries.len());
                    println!("  词条数: {}", words);
                }
                Err(e) => eprintln!("  解析附加统计失败: {}", e),
            }
        }
        Err(e) => {
            println!("验证失败: {}", e);
            return Err(CliError::DictError(e.to_string()));
        }
    }

    Ok(())
}

/// 显示统计信息
fn run_stats(dict_path: &Option<PathBuf>) -> CliResult<()> {
    let path = match dict_path {
        Some(p) => p.clone(),
        None => default_dict_path(),
    };

    if !path.exists() {
        return Err(CliError::DictError(format!(
            "词典文件不存在: {}",
            path.display()
        )));
    }

    let parser = CinParser::new(
        path.to_str()
            .ok_or_else(|| CliError::DictError("路径包含无效 Unicode".to_string()))?,
    );
    let entries = parser
        .parse()
        .map_err(|e| CliError::DictError(e.to_string()))?;

    let total_words: usize = entries.iter().map(|e| e.words.len()).sum();
    let max_code_len = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
    let min_code_len = entries.iter().map(|e| e.code.len()).min().unwrap_or(0);
    let avg = if entries.is_empty() {
        0.0
    } else {
        total_words as f64 / entries.len() as f64
    };

    println!("词典统计: {}", path.display());
    println!("  编码数量: {}", entries.len());
    println!("  词条数量: {}", total_words);
    println!("  最短编码: {} 字符", min_code_len);
    println!("  最长编码: {} 字符", max_code_len);
    println!("  平均每编码词条: {:.1}", avg);

    Ok(())
}

/// CLI 主函数
fn main() {
    let command = match parse_args() {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("错误: {}", e);
            process::exit(1);
        }
    };

    let result = match command {
        CliCommand::Interactive => run_interactive(),
        CliCommand::Query { code } => run_query(&code),
        CliCommand::TestMatch { code, dict_path } => run_test_match(&code, &dict_path),
        CliCommand::BuildDict { dict_path, out_path } => run_build_dict(&dict_path, &out_path),
        CliCommand::Validate { dict_path } => run_validate(&dict_path),
        CliCommand::Stats { dict_path } => run_stats(&dict_path),
        CliCommand::Version => {
            show_version();
            Ok(())
        }
        CliCommand::Help => {
            show_help();
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("错误: {}", e);
        process::exit(1);
    }
}
