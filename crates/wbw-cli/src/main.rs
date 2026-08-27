//! wbwIME 命令行入口
//!
//! 提供命令行接口，支持词典查询、测试匹配等功能。

use std::path::PathBuf;
use std::process;
use wbw_types::GlobalConfig;

/// CLI 错误类型
#[derive(Debug)]
pub enum CliError {
    ConfigError(String),
    DictError(String),
    InputError(String),
    IoError(String),
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::ConfigError(msg) => write!(f, "配置错误: {}", msg),
            CliError::DictError(msg) => write!(f, "词典错误: {}", msg),
            CliError::InputError(msg) => write!(f, "输入错误: {}", msg),
            CliError::IoError(msg) => write!(f, "IO 错误: {}", msg),
        }
    }
}

type CliResult<T> = Result<T, CliError>;

/// CLI 命令
#[derive(Debug)]
pub enum CliCommand {
    Interactive,
    Query { code: String },
    TestMatch { code: String, dict_path: PathBuf },
    Stats { dict_path: PathBuf },
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
                Err(CliError::InputError("请提供查询编码".to_string()))
            } else {
                Ok(CliCommand::Query { code: args[2].clone() })
            }
        }
        "test-match" | "t" => {
            if args.len() < 4 {
                Err(CliError::InputError("用法: wbwime test-match <code> <dict-path>".to_string()))
            } else {
                Ok(CliCommand::TestMatch {
                    code: args[2].clone(),
                    dict_path: PathBuf::from(&args[3]),
                })
            }
        }
        "stats" | "s" => {
            if args.len() < 3 {
                Err(CliError::InputError("请提供词典路径".to_string()))
            } else {
                Ok(CliCommand::Stats {
                    dict_path: PathBuf::from(&args[2]),
                })
            }
        }
        "version" | "v" => Ok(CliCommand::Version),
        "help" | "-h" | "--help" => Ok(CliCommand::Help),
        cmd => Err(CliError::InputError(format!("未知命令: {}", cmd))),
    }
}

/// 显示帮助信息
fn show_help() {
    println!("wbwIME - Rust 自构建输入法引擎");
    println!();
    println!("用法: wbwime [命令] [选项]");
    println!();
    println!("命令:");
    println!("  interactive    交互式输入模式");
    println!("  query <code>   查询编码");
    println!("  test-match     测试匹配");
    println!("  stats          显示统计信息");
    println!("  version        显示版本信息");
    println!("  help           显示帮助信息");
}

/// 显示版本信息
fn show_version() {
    println!("wbwIME v{}", env!("CARGO_PKG_VERSION"));
    println!("Rust 自构建输入法引擎");
}

/// 执行查询
fn run_query(code: &str) -> CliResult<()> {
    // 尝试加载词典
    let dict_path = std::path::Path::new("resources/dicts/cs-oi.cin");
    if !dict_path.exists() {
        println!("词典文件不存在: {}", dict_path.display());
        println!("请先运行: wbwime test-match {} resources/dicts/cs-oi.cin", code);
        return Ok(());
    }

    let parser = wbw_dict::CinParser::new(dict_path.to_str().unwrap());
    let entries = parser.parse().map_err(|e| CliError::DictError(e.to_string()))?;

    // 直接查找
    let matches: Vec<&wbw_dict::CinEntry> = entries.iter().filter(|e| e.code == code).collect();

    if matches.is_empty() {
        println!("未找到编码 '{}' 的匹配", code);
    } else {
        println!("编码 '{}' 的匹配:", code);
        for entry in &matches {
            for word in &entry.words {
                println!("  {} (词频: {})", word.word, word.freq);
            }
        }
    }

    Ok(())
}

/// 执行测试匹配
fn run_test_match(code: &str, dict_path: &PathBuf) -> CliResult<()> {
    if !dict_path.exists() {
        return Err(CliError::DictError(format!("词典文件不存在: {}", dict_path.display())));
    }

    let parser = wbw_dict::CinParser::new(dict_path.to_str().unwrap());
    let entries = parser.parse().map_err(|e| CliError::DictError(e.to_string()))?;

    println!("词典加载成功: {} 条目", entries.len());

    // 构建匹配器
    let mut builder = wbw_dict::FstDictBuilder::new();
    for entry in &entries {
        for word in &entry.words {
            builder.add_entry(wbw_dict::entry::DictEntry {
                code: entry.code.clone(),
                word: word.word.clone(),
                freq: word.freq,
                source: wbw_dict::entry::DictSource::Base,
            });
        }
    }
    let dict = builder.build(wbw_dict::entry::DictSource::Base);

    let matcher = wbw_matcher::Matcher::with_dict(
        wbw_matcher::MatcherConfig::default(),
        dict,
    );

    // 测试精确匹配
    let exact = matcher.exact_lookup(code);
    if !exact.is_empty() {
        println!("\n精确匹配 '{}':", code);
        for c in &exact {
            println!("  {} (分数: {:.1})", c.text, c.score);
        }
    }

    // 测试前缀匹配
    let prefix = matcher.prefix_lookup(code);
    if !prefix.is_empty() {
        println!("\n前缀匹配 '{}':", code);
        for c in &prefix {
            println!("  {} [{}] (分数: {:.1})", c.text, c.code, c.score);
        }
    }

    // 测试模糊匹配
    let fuzzy = matcher.fuzzy_lookup(code);
    if !fuzzy.is_empty() {
        println!("\n模糊匹配 '{}':", code);
        for c in &fuzzy {
            println!("  {} [{}] (分数: {:.1})", c.text, c.code, c.score);
        }
    }

    if exact.is_empty() && prefix.is_empty() && fuzzy.is_empty() {
        println!("未找到编码 '{}' 的匹配", code);
    }

    Ok(())
}

/// 显示统计信息
fn run_stats(dict_path: &PathBuf) -> CliResult<()> {
    if !dict_path.exists() {
        return Err(CliError::DictError(format!("词典文件不存在: {}", dict_path.display())));
    }

    let parser = wbw_dict::CinParser::new(dict_path.to_str().unwrap());
    let entries = parser.parse().map_err(|e| CliError::DictError(e.to_string()))?;

    let total_words: usize = entries.iter().map(|e| e.words.len()).sum();
    let max_code_len = entries.iter().map(|e| e.code.len()).max().unwrap_or(0);
    let min_code_len = entries.iter().map(|e| e.code.len()).min().unwrap_or(0);

    println!("词典统计: {}", dict_path.display());
    println!("  编码数量: {}", entries.len());
    println!("  词条数量: {}", total_words);
    println!("  最短编码: {} 字符", min_code_len);
    println!("  最长编码: {} 字符", max_code_len);

    Ok(())
}

/// 交互模式
fn run_interactive() -> CliResult<()> {
    println!("wbwIME 交互模式");
    println!("输入编码进行查询，输入 'quit' 退出");
    println!();

    // 尝试加载词典
    let dict_path = std::path::Path::new("resources/dicts/cs-oi.cin");
    if !dict_path.exists() {
        println!("警告: 词典文件不存在，无法进行匹配");
    }

    loop {
        print!("> ");
        use std::io::Write;
        std::io::stdout().flush().unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input == "quit" || input == "exit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        if let Err(e) = run_query(input) {
            eprintln!("错误: {}", e);
        }
    }

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
