//! wbwIME 命令行入口

use std::path::PathBuf;
use std::process;
use thiserror::Error;
use wbw_types::{GlobalConfig, ImeResult};

/// CLI 错误类型
#[derive(Error, Debug)]
pub enum CliError {
    #[error("配置错误: {0}")]
    ConfigError(String),
    
    #[error("词典错误: {0}")]
    DictError(String),
    
    #[error("输入错误: {0}")]
    InputError(String),
    
    #[error("输出错误: {0}")]
    OutputError(String),
    
    #[error("IO 错误: {0}")]
    IoError(String),
}

/// CLI 命令
#[derive(Debug, Clone)]
pub enum CliCommand {
    /// 交互式输入
    Interactive,
    /// 单次查询
    Query { code: String },
    /// 构建词典
    BuildDict { input: PathBuf, output: PathBuf },
    /// 验证词典
    ValidateDict { dict_path: PathBuf },
    /// 显示统计信息
    Stats { dict_path: PathBuf },
    /// 测试匹配
    TestMatch { code: String, dict_path: PathBuf },
    /// 显示版本信息
    Version,
    /// 显示帮助信息
    Help,
}

/// CLI 配置
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// 全局配置
    pub global: GlobalConfig,
    /// 词典路径
    pub dict_path: Option<PathBuf>,
    /// N-gram 模型路径
    pub ngram_path: Option<PathBuf>,
    /// 是否详细输出
    pub verbose: bool,
    /// 是否静默模式
    pub quiet: bool,
    /// 输出格式
    pub output_format: OutputFormat,
}

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// 文本格式
    Text,
    /// JSON 格式
    Json,
    /// CSV 格式
    Csv,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            global: GlobalConfig::default(),
            dict_path: None,
            ngram_path: None,
            verbose: false,
            quiet: false,
            output_format: OutputFormat::Text,
        }
    }
}

/// CLI 解析器
pub struct CliParser;

impl CliParser {
    /// 解析命令行参数
    pub fn parse() -> CliResult<CliCommand> {
        // TODO: 实现参数解析
        todo!("实现命令行参数解析")
    }

    /// 解析配置文件
    pub fn parse_config(path: &PathBuf) -> CliResult<CliConfig> {
        // TODO: 实现配置文件解析
        todo!("实现配置文件解析")
    }
}

/// CLI 执行器
pub struct CliExecutor {
    /// 配置
    config: CliConfig,
}

impl CliExecutor {
    /// 创建新的执行器
    pub fn new(config: CliConfig) -> Self {
        Self { config }
    }

    /// 执行命令
    pub fn execute(&self, command: CliCommand) -> CliResult<()> {
        match command {
            CliCommand::Interactive => self.run_interactive(),
            CliCommand::Query { code } => self.run_query(&code),
            CliCommand::BuildDict { input, output } => self.run_build_dict(&input, &output),
            CliCommand::ValidateDict { dict_path } => self.run_validate_dict(&dict_path),
            CliCommand::Stats { dict_path } => self.run_stats(&dict_path),
            CliCommand::TestMatch { code, dict_path } => self.run_test_match(&code, &dict_path),
            CliCommand::Version => self.run_version(),
            CliCommand::Help => self.run_help(),
        }
    }

    /// 运行交互模式
    fn run_interactive(&self) -> CliResult<()> {
        // TODO: 实现交互模式
        todo!("实现交互模式")
    }

    /// 运行查询
    fn run_query(&self, code: &str) -> CliResult<()> {
        // TODO: 实现查询
        todo!("实现查询")
    }

    /// 运行构建词典
    fn run_build_dict(&self, input: &PathBuf, output: &PathBuf) -> CliResult<()> {
        // TODO: 实现构建词典
        todo!("实现构建词典")
    }

    /// 运行验证词典
    fn run_validate_dict(&self, dict_path: &PathBuf) -> CliResult<()> {
        // TODO: 实现验证词典
        todo!("实现验证词典")
    }

    /// 运行统计信息
    fn run_stats(&self, dict_path: &PathBuf) -> CliResult<()> {
        // TODO: 实现统计信息
        todo!("实现统计信息")
    }

    /// 运行测试匹配
    fn run_test_match(&self, code: &str, dict_path: &PathBuf) -> CliResult<()> {
        // TODO: 实现测试匹配
        todo!("实现测试匹配")
    }

    /// 运行版本信息
    fn run_version(&self) -> CliResult<()> {
        println!("wbwIME v{}", env!("CARGO_PKG_VERSION"));
        println!("Rust 自构建输入法引擎");
        Ok(())
    }

    /// 运行帮助信息
    fn run_help(&self) -> CliResult<()> {
        println!("wbwIME - Rust 自构建输入法引擎");
        println!();
        println!("用法: wbwime [命令] [选项]");
        println!();
        println!("命令:");
        println!("  interactive    交互式输入模式");
        println!("  query <code>   查询编码");
        println!("  build-dict     构建词典");
        println!("  validate       验证词典");
        println!("  stats          显示统计信息");
        println!("  test-match     测试匹配");
        println!("  version        显示版本信息");
        println!("  help           显示帮助信息");
        println!();
        println!("选项:");
        println!("  --config <path>    配置文件路径");
        println!("  --dict <path>      词典路径");
        println!("  --ngram <path>     N-gram 模型路径");
        println!("  --verbose          详细输出");
        println!("  --quiet            静默模式");
        println!("  --format <fmt>     输出格式 (text, json, csv)");
        Ok(())
    }
}

/// CLI 结果类型
pub type CliResult<T> = Result<T, CliError>;

/// CLI 主函数
fn main() {
    // 解析命令行参数
    let command = match CliParser::parse() {
        Ok(cmd) => cmd,
        Err(e) => {
            eprintln!("错误: {}", e);
            process::exit(1);
        }
    };
    
    // 加载配置
    let config = CliConfig::default();
    
    // 创建执行器
    let executor = CliExecutor::new(config);
    
    // 执行命令
    if let Err(e) = executor.execute(command) {
        eprintln!("错误: {}", e);
        process::exit(1);
    }
}