# wbwIME 更新计划

按已确认顺序执行：**bug → 重构 → 测试 → FST**。死代码「重新接线」而非删除。

决策确认：
- FST 持久化：真 FST + 二进制快照（fst crate）
- 死代码：重新接线（保留能力）而非删除
- 顺序：bug → 重构 → 测试 → FST
- FST 定位：作为唯一数据源（替换运行态 HashMap），模糊采用 fst Levenshtein automaton

## 阶段 1：修 Bug（优先，风险最高）

1. **候选去重全局化** — `crates/wbw-matcher/src/matcher.rs:153`
   - 现 `dedup_by` 排序后只去相邻重复，相同词不同分数（如「最大流」200/100）两侧夹着别的词导致去重失效。
   - 改为先按 text 分组求最高分（或先 sort 后 `retain(HashSet<text>)` 保留首个=最高分），与 CLI 路径（`main.rs:394`、`:452`）行为统一。
2. **缓存一致性** — `crates/wbw-matcher/src/matcher.rs:97`
   - `load_cin`/`load_dict` 重新加载词典后调用 `clear_cache()`，避免旧词典缓存污染。
3. **可触发崩溃的风险点**：
   - `matcher.rs:52,66` / `ranker.rs:200`：`cache_size==0` 时 `NonZeroUsize::unwrap()` panic → 回退为禁用缓存（None）或 clamp 到 1。
   - `fuzzy.rs:140`：`input[search_pos..]` 字节切片可能在多字节字符内 panic → 用字符安全遍历（`char_indices`）修 `generate_variants`（同时修掉替换错位 bug）。
   - `candidate.rs:44`：`current_page` 越界切片 → 加 start/end 边界守卫。
   - `main.rs:329`：`read_line().unwrap()` → 匹配 Result。
4. **翻页重置** — `candidate_window.rs:152-155`：`update_candidates` 同时重置 `selected_index` 与 `page=0`，避免少页时 `select_next` 卡死。

## 阶段 2：重构（去死代码 by 重新接线）

1. **FuzzyMatcher 重新接线** — `matcher.rs` + `fuzzy.rs`
   - 把 `fuzzy_matcher: FuzzyMatcher` 字段重新加回 `Matcher`。
   - `fuzzy_lookup` 改为：编辑距离结果（`dict.fuzzy_lookup`）+ 规则变体结果（`generate_variants` 后 `dict.lookup`）合并。
   - 修复 `generate_variants` 字节偏移 bug（fuzzy.rs:171 `replace_range` 用字符索引），使 `ei→ie`/`ui→iu`（编辑距离 2，编辑距离引擎覆盖不到）由规则引擎补足。
   - 合并重复的 `edit_distance`（fuzzy.rs:185 与 fst_dict.rs:227 两份）。
2. **unused 依赖清理**（保留将接线的）：
   - 删除全工作区未用 `anyhow`（7 crates）、`phf`、未接线 `tempfile`；`wbw-matcher` 移除 `thiserror`。
   - `wbw-cli` 的 `wbw-core`/`wbw-ngram` 移到 dev-dependencies（仅集成测试用）。
   - 根 `Cargo.toml` 移除 `inputx-ngram`、`perfgate`（`criterion` 保留用于阶段 3）。
   - **保留** `fst`/`memmap2`（阶段 4 接入）、`serde`（快照用）。
3. **Ranker 死字段清理** — `ranker.rs:17` cache 与 `:69-70` 无效归一化：按「重新接线」方针把 cache 真正用于 rank 缓存，删除无效归一化代码。
4. **命名整改留待阶段 4**：`FstDict::from_file` 空实现 + `build-dict` 哑输出。

## 阶段 3：补测试 / 基准（criterion）

1. **根 benchmark 改造**：`benches/benchmark.rs` 现为 12 个 `todo!()` 且不属于任何 crate 的 `[[bench]]`（不会被编译）。
   - 移入 `crates/wbw-dict/benches/`（或用 `[[bench]]` 目标），实现 `bench_dict_load`、`bench_fuzzy_match`、`bench_candidate_ranking` 等核心基准（用 `resources/dicts/cs-oi.cin` 真实数据）。
   - `bench_fuzzy_match` 作为阶段 4 FST/编辑距离优化前后的对比基准。
2. **补集成测试**：覆盖 bug 修复（去重、缓存 reload、FuzzyMatcher 对调规则 `ei→ie` 接线），加快照 round-trip 测试（阶段 4 后）。
3. 维持 `cargo test --workspace` 与 `cargo clippy --workspace --all-targets -- -D warnings` 全绿。

## 阶段 4：真 FST + 二进制快照（最后、工作量最大）

1. **接入 `fst::Map`** — `crates/wbw-dict/src/fst_dict.rs`
   - 数据模型：key = `code + '\u{0001}' + word`（0x01 分隔，code/word 不含控制字符），value = `freq`（u64）。
   - 构建：收集全部 `(code,word,freq)` 排序后用 `fst::MapBuilder` 写入。
   - 查询：
     - 精确：`map.get(code ^ word)` → 词条。
     - 前缀：`map.range().ge(code)` 流式取码 `code` 开头的所有键，解析回 `(code,word,freq)` 列表，替换 HashMap 线性扫描。
     - 模糊：用 `fst::automaton::Levenshtein::new(code, max)` 做 automaton 编辑距离搜索，替代当前 O(m·n) 全表 DP。
   - 多词同码自然展开为多个 key，`lookup(code)`/`prefix_lookup` 合并返回。
2. **二进制快照**：
   - `FstDict::to_bytes()`（`Map::into_bytes`）与 `from_bytes()/from_file()`（`Map::new(reader)`，支持 `memmap2` 映射只读加载）。
   - `build-dict <dict> <out>` 真正写 `.fst` 文件；新增 CLI 支持从 `.fst` 直接加载运行（query/test-match 可不经 .cin 即时构建）。
   - FST 作为唯一数据源替换运行态 HashMap。
3. **验证**：字典 round-trip 测试（构建→写→读→相等）、前缀/模糊查询结果与现状一致，跑基准对比性能。

## 提交策略

沿用现有约定，每个逻辑变更 `git commit`（中文信息）：
- `[FIX] 修复候选去重、缓存一致性、崩溃风险`（阶段 1）
- `[REFACTOR] 重新接线 FuzzyMatcher、清理未用依赖`（阶段 2）
- `[TEST] 补基准与集成测试`（阶段 3）
- `[FEAT] 接入 fst 实现 FST 词典与二进制快照`（阶段 4）

## 环境备忘

- Windows + PowerShell；rustup gnu 工具链在 `C:\Users\yl\.cargo\bin`，需 prepend PATH 到每条命令。
- `$env:CARGO_INCREMENTAL="0"` 规避环境错误。
- 验证命令：
  - `cargo test --workspace`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- `resources/dicts/cs-oi.cin` 用户可能编辑，勿随意 stage。
