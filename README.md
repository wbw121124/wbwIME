# wbwIME

Rust 自构建输入法引擎

## 功能特性

- **.cin 码表支持** — 解析标准 .cin 格式码表，支持批量导入与合并
- **FST 词典** — 基于有限状态转换器的高性能词典查询
- **N-gram 语言模型** — 支持 bigram/trigram 评分与多种平滑算法
- **模糊匹配** — 支持声母、韵母、声调等多维度模糊规则
- **加权排序** — 拼音匹配、词频、用户行为、N-gram 多因子加权排序
- **L0 动态学习** — 基于用户输入习惯的实时学习与词频提升
- **会话管理** — 多会话隔离，支持历史快照与撤销
- **候选窗口** — 可定制样式的候选词展示层
- **CLI 工具** — 交互式输入、词典构建、测试匹配等命令行功能

## 项目结构

```plain
wbwIME/
├── Cargo.toml              # 工作区配置
├── crates/
│   ├── wbw-types/          # 共享类型定义（打破循环依赖）
│   ├── wbw-dict/           # 词典模块（.cin 解析、FST 词典）
│   ├── wbw-matcher/        # 匹配模块（拼音、分词、模糊匹配）
│   ├── wbw-ngram/          # N-gram 语言模型（评分、平滑）
│   ├── wbw-core/           # 核心模块（会话、上下文、候选词）
│   ├── wbw-rank/           # 排序模块（加权排序、L0 学习）
│   ├── wbw-imekit/         # IME 宿主适配层
│   ├── wbw-ime-native/     # Native IME 接口
│   ├── wbw-ime-fbterm/     # FBTerm 终端适配
│   ├── wbw-ime-tsf/        # Windows TSF 输入法 DLL
│   ├── wbw-ime-ipc/        # IPC 通信协议
│   ├── wbw-ime-gui/        # 图形候选窗口
│   └── wbw-cli/            # 命令行工具
├── tests/                  # 集成测试
├── benches/                # 性能基准测试
├── resources/              # 资源文件（配置、码表）
└── docs/                   # 文档
```

## Crate 依赖关系

```plain
wbw-types (共享类型)
  ↑
wbw-dict ← wbw-matcher ← wbw-core
              ↑              ↑
           wbw-ngram ← wbw-rank
                         ↑
                      wbw-imekit
                         ↑
                      wbw-cli
```

## 构建

```bash
# 调试构建
cargo build

# 发布构建
cargo build --release

# 运行 CLI
cargo run --bin wbwime -- help
```

## 配置

配置文件位于 `resources/config.toml`：

```toml
[dict]
base_path = "resources/dicts/pinyin.cin"   # 基础词典路径
ngram_path = "resources/dicts/ngram.bin"  # N-gram 模型路径
user_dict_path = "resources/dicts/user.txt" # 用户词典路径

[matcher]
fuzzy = true                              # 启用模糊匹配
fuzzy_rules = ["z->zh", "c->ch", "s->sh", "n->l", "l->n"]

[rank]
pin_weight = 100.0    # 拼音匹配权重
user_weight = 10.0    # 用户词库权重
freq_weight = 1.0     # 词频权重
ngram_weight = 0.5    # N-gram 权重
max_candidates = 10   # 最大候选词数量

[l0]
threshold = 3                    # L0 学习触发阈值
snapshot_path = "wbw_l0.json"   # 学习快照路径

[ngram]
order = 3                        # N-gram 阶数（2=bigram, 3=trigram）
smooth = 0.1  # 平滑参数 (f64)
model_path = "resources/dicts/ngram.bin"  # N-gram 模型文件
```

## 码表格式

支持标准 .cin 格式，每行 `编码 汉字`，`%` 开头为注释行：

```cin
% 示例码表
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
```

## 开发

```bash
# 运行所有测试
cargo test

# 运行基准测试
cargo bench

# 代码检查
cargo check

# 格式化代码
cargo fmt

# 代码静态分析
cargo clippy
```

## Windows 安装

1. 编译：`cargo build --release -p wbw-ime-tsf -p wbw-ime-gui`
2. 复制 `target/release/wbw_ime_tsf.dll` 和 `target/release/wbw-ime-gui.exe` 到同一目录
3. 注册 DLL：`regsvr32 wbw_ime_tsf.dll`（需管理员权限）
4. 在 Windows 设置 → 时间和语言 → 语言 → 添加输入法 → 选择 wbwIME

## 许可证

MPL-2.0
