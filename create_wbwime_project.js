// create_wbwime_project.js
// 自动创建 wbwIME 输入法引擎项目结构

const fs = require('fs');
const path = require('path');

// 获取命令行参数
const args = process.argv.slice(2);
const projectRoot = args[0] || 'wbwIME';

// ANSI 颜色
const colors = {
	reset: '\x1b[0m',
	cyan: '\x1b[36m',
	green: '\x1b[32m',
	yellow: '\x1b[33m',
	gray: '\x1b[90m',
	white: '\x1b[37m',
	red: '\x1b[31m'
};

function log(message, color = 'reset') {
	console.log(`${colors[color]}${message}${colors.reset}`);
}

// 定义文件结构
const structure = {
	// 根目录文件
	'Cargo.toml': `[workspace]
members = [
    "crates/wbw-core",
    "crates/wbw-dict",
    "crates/wbw-matcher",
    "crates/wbw-rank",
    "crates/wbw-ngram",
    "crates/wbw-imekit",
    "crates/wbw-cli",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["Your Name <your@email.com>"]
license = "MIT OR Apache-2.0"

[workspace.dependencies]
fst = "0.4"
memmap2 = "0.5"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.7"
phf = { version = "0.11", features = ["macros"] }
lru = "0.12"
anyhow = "1.0"
thiserror = "1.0"
imekit = "0.1"
inputx-ngram = "0.1"
criterion = "0.5"
perfgate = "0.1"`,

	'README.md': `# wbwIME

Rust 自构建输入法引擎

## 功能特性
- 支持 .cin 码表
- N-gram 语言模型
- 模糊匹配
- 动态词库 L0 学习
- 加权排序与置顶

## 构建
\`\`\`bash
cargo build --release
\`\`\``,

	'.gitignore': `/target/
**/*.rs.bk
*.swp
*.swo
wbw_l0.json
*.log
.DS_Store
`,

	// docs 目录
	'docs/architecture.md': '# 架构设计文档',
	'docs/cin_spec.md': '# .cin 格式说明',

	// resources 目录
	'resources/dicts/base.cin': `% 基础码表示例
wo 我
wo 喔
ai 爱
ni 你
ta 他
hao 好
shi 是
de 的
le 了
bu 不
`,
	'resources/dicts/ngram.bin': '',
	'resources/config.toml': `[dict]
base_path = "resources/dicts/base.cin"
ngram_path = "resources/dicts/ngram.bin"

[matcher]
fuzzy = true
fuzzy_rules = ["z->zh", "c->ch", "s->sh", "n->l", "l->n"]

[rank]
pin_weight = 100.0
user_weight = 10.0
freq_weight = 1.0
ngram_weight = 0.5
max_candidates = 10

[l0]
threshold = 3
snapshot_path = "wbw_l0.json"

[ngram]
order = 2
smooth = 0.1
`,

	// tests 目录
	'tests/integration_test.rs': '// 集成测试',

	// benches 目录
	'benches/benchmark.rs': '// 性能基准测试',

	// wbw-core
	'crates/wbw-core/Cargo.toml': `[package]
name = "wbw-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
wbw-dict = { path = "../wbw-dict" }
wbw-matcher = { path = "../wbw-matcher" }
wbw-rank = { path = "../wbw-rank" }
anyhow.workspace = true
thiserror.workspace = true
`,
	'crates/wbw-core/src/lib.rs': `pub mod session;
pub mod candidate;
pub mod context;
pub mod error;`,
	'crates/wbw-core/src/session.rs': '// Session 管理',
	'crates/wbw-core/src/candidate.rs': '// 候选数据结构',
	'crates/wbw-core/src/context.rs': '// 输入上下文',
	'crates/wbw-core/src/error.rs': '// 错误类型',

	// wbw-dict
	'crates/wbw-dict/Cargo.toml': `[package]
name = "wbw-dict"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
fst.workspace = true
memmap2.workspace = true
serde.workspace = true
anyhow.workspace = true
tempfile = "3.8"
`,
	'crates/wbw-dict/src/lib.rs': `pub mod fst_dict;
pub mod cin_parser;
pub mod entry;
pub mod builder;`,
	'crates/wbw-dict/src/fst_dict.rs': '// FST 词典',
	'crates/wbw-dict/src/cin_parser.rs': '// .cin 解析器',
	'crates/wbw-dict/src/entry.rs': '// 词条数据结构',
	'crates/wbw-dict/src/builder.rs': '// 词典构建工具',

	// wbw-matcher
	'crates/wbw-matcher/Cargo.toml': `[package]
name = "wbw-matcher"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
wbw-dict = { path = "../wbw-dict" }
phf.workspace = true
anyhow.workspace = true
`,
	'crates/wbw-matcher/src/lib.rs': `pub mod fuzzy;
pub mod segmenter;
pub mod pinyin;
pub mod matcher;`,
	'crates/wbw-matcher/src/fuzzy.rs': '// 模糊匹配',
	'crates/wbw-matcher/src/segmenter.rs': '// 分词',
	'crates/wbw-matcher/src/pinyin.rs': '// 拼音处理',
	'crates/wbw-matcher/src/matcher.rs': '// 匹配器主体',

	// wbw-rank
	'crates/wbw-rank/Cargo.toml': `[package]
name = "wbw-rank"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
wbw-core = { path = "../wbw-core" }
wbw-ngram = { path = "../wbw-ngram" }
serde.workspace = true
serde_json.workspace = true
lru.workspace = true
anyhow.workspace = true
`,
	'crates/wbw-rank/src/lib.rs': `pub mod ranker;
pub mod l0_learn;
pub mod weight;
pub mod config;`,
	'crates/wbw-rank/src/ranker.rs': '// 排序器主体',
	'crates/wbw-rank/src/l0_learn.rs': '// L0 动态学习',
	'crates/wbw-rank/src/weight.rs': '// 权重计算',
	'crates/wbw-rank/src/config.rs': '// 排序配置',

	// wbw-ngram
	'crates/wbw-ngram/Cargo.toml': `[package]
name = "wbw-ngram"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
fst.workspace = true
memmap2.workspace = true
anyhow.workspace = true
`,
	'crates/wbw-ngram/src/lib.rs': `pub mod scorer;
pub mod table;
pub mod smooth;`,
	'crates/wbw-ngram/src/scorer.rs': '// N-gram 评分器',
	'crates/wbw-ngram/src/table.rs': '// 概率表',
	'crates/wbw-ngram/src/smooth.rs': '// 平滑处理',

	// wbw-imekit
	'crates/wbw-imekit/Cargo.toml': `[package]
name = "wbw-imekit"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
wbw-core = { path = "../wbw-core" }
imekit.workspace = true
anyhow.workspace = true
`,
	'crates/wbw-imekit/src/lib.rs': `pub mod ime_host;
pub mod candidate_window;
pub mod key_mapper;`,
	'crates/wbw-imekit/src/ime_host.rs': '// IME 宿主',
	'crates/wbw-imekit/src/candidate_window.rs': '// 候选窗口',
	'crates/wbw-imekit/src/key_mapper.rs': '// 按键映射',

	// wbw-cli
	'crates/wbw-cli/Cargo.toml': `[package]
name = "wbw-cli"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[[bin]]
name = "wbwime"
path = "src/main.rs"

[dependencies]
wbw-core = { path = "../wbw-core" }
wbw-dict = { path = "../wbw-dict" }
wbw-matcher = { path = "../wbw-matcher" }
wbw-rank = { path = "../wbw-rank" }
wbw-ngram = { path = "../wbw-ngram" }
anyhow.workspace = true
`,
	'crates/wbw-cli/src/main.rs': '// CLI 入口'
};

// 额外空目录
const emptyDirs = [
	'target',
	'resources/scripts'
];

// 统计变量
let createdCount = 0;
let skippedCount = 0;

/**
 * 创建文件（如果不存在）
 */
function createFile(filePath, content = '') {
	const fullPath = path.join(projectRoot, filePath);
	const dir = path.dirname(fullPath);

	// 创建目录
	if (!fs.existsSync(dir)) {
		fs.mkdirSync(dir, { recursive: true });
	}

	// 检查文件是否已存在
	if (fs.existsSync(fullPath)) {
		log(`  ⏭️  跳过已存在: ${filePath}`, 'gray');
		skippedCount++;
		return false;
	}

	// 创建文件
	fs.writeFileSync(fullPath, content, 'utf8');
	log(`  ✅ 创建: ${filePath}`, 'green');
	createdCount++;
	return true;
}

/**
 * 创建空目录
 */
function createEmptyDir(dirPath) {
	const fullPath = path.join(projectRoot, dirPath);
	if (!fs.existsSync(fullPath)) {
		fs.mkdirSync(fullPath, { recursive: true });
		log(`  📁 创建目录: ${dirPath}`, 'cyan');
	}
}

/**
 * 主函数
 */
function main() {
	log(`🚀 开始创建 wbwIME 项目结构...`, 'cyan');
	log(`项目路径: ${projectRoot}`, 'yellow');

	// 创建根目录
	if (!fs.existsSync(projectRoot)) {
		fs.mkdirSync(projectRoot, { recursive: true });
	} else {
		log(`⚠️  项目目录已存在，将跳过已创建的文件`, 'yellow');
	}

	log(`\n📁 创建目录结构...`, 'cyan');

	// 创建所有文件
	const filePaths = Object.keys(structure);
	for (const filePath of filePaths) {
		const content = structure[filePath];
		createFile(filePath, content);
	}

	// 创建额外空目录
	for (const dir of emptyDirs) {
		createEmptyDir(dir);
	}

	// 输出统计信息
	log(`\n📊 创建完成!`, 'green');
	log(`  ✅ 新建文件: ${createdCount}`, 'green');
	log(`  ⏭️  跳过文件: ${skippedCount}`, 'gray');
	log(`\n📂 项目路径: ${projectRoot}`, 'yellow');

	// 显示下一步操作提示
	log(`\n🚀 下一步:`, 'cyan');
	log(`  1. cd ${projectRoot}`, 'white');
	log(`  2. cargo build`, 'white');
	log(`  3. cargo run -p wbw-cli`, 'white');
}

// 运行主函数
main();