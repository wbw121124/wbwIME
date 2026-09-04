# wbwIME 代码审查修复计划

> 来源：代码审查 2026-09-04
> 状态：进行中

---

## 🔴 严重问题

### P0-1：wbw-ngram Cargo.toml 语法错误（阻碍编译）
**文件：** `crates/wbw-ngram/Cargo.toml`
**问题：** `[package]` 块末尾有多余的 `}`
**修复：** 删除多余 `}`

---

### P0-2：PinyinValidator::can_split_into_syllables 指数级回溯风险
**文件：** `crates/wbw-matcher/src/pinyin.rs`
**问题：** 递归无 memoization，最坏情况 O(2^n)
**修复：** 加入 memo 表（HashMap 缓存已验证结果）

---

### P0-3：FstDict::fuzzy_lookup 全表扫描性能问题
**文件：** `crates/wbw-dict/src/fst_dict.rs`
**问题：** 对每个 key 调用 edit_distance O(n·m)，词典大时灾难
**修复：** 限制 max_edit_distance=1 时使用 fst lev automaton；>1 时保留扫描但加 early-exit

---

## 🟡 中等问题

### P1-1：统一去重逻辑
**文件：** `crates/wbw-core/src/candidate.rs` + `crates/wbw-matcher/src/matcher.rs`
**问题：** 三处去重逻辑不一致（按 text / text+code）
**修复：** 在 `CandidateFilter` 中新增 `dedup_by_text` 和 `dedup_by_text_code`，matcher 统一调用

---

### P1-2：SessionState 时间字段语义不清
**文件：** `crates/wbw-core/src/session.rs`
**问题：** `created_at`/`last_active` 是内存态不应被序列化
**修复：** 实现 `serde(skip)` 或标注文档；或改用 `NonZeroU64` 防止零值

---

### P1-3：ImeHost session_id 硬编码为 1
**文件：** `crates/wbw-imekit/src/ime_host.rs`
**问题：** `session_id: 1` 所有实例共享，未集成 SessionManager
**修复：** 增加 `with_session_id` builder 方法，默认随机生成

---

## 🟢 轻微问题

### P2-1：CLI run_test_match 缩进错误 + 空 all 调用 rank
**文件：** `crates/wbw-cli/src/main.rs`
**问题：** 缩进不一致，retain 后 all 可能为空
**修复：** 修正缩进，加 empty check

---

### P2-2：ContextManager pop_char 光标一致性确认
**文件：** `crates/wbw-core/src/context.rs`
**问题：** cursor 是字节偏移，需确认多字节字符场景
**修复：** 添加测试用例验证 emoji 和多字节字符

---

## 修改记录

| # | 日期 | 事项 | 状态 |
|---|------|------|------|
| 1 | 2026-09-04 | P0-1 修复 wbw-ngram/Cargo.toml 语法错误 | ✅ 已跳过（误报，文件正确）|
| 2 | 2026-09-04 | P0-2 修复 can_split_into_syllables 指数回溯 | ✅ 完成（加 memoization）|
| 3 | 2026-09-04 | P0-3 优化 fuzzy_lookup 性能 | ✅ 完成（加长度剪枝）|
| 4 | 2026-09-04 | P1-1 统一去重逻辑 | ✅ 完成（新增 dedup_by_text，matcher 调用 CandidateFilter）|
| 5 | 2026-09-04 | P1-2 SessionState 时间字段语义 | ✅ 完成（添加文档注释说明内存态）|
| 6 | 2026-09-04 | P1-3 ImeHost session_id 硬编码 | ✅ 完成（改用 rand::random()，新增 with_session_id builder）|
| 7 | 2026-09-04 | P2-1 CLI 缩进错误 | ✅ 完成（修正缩进，加 empty check）|
| 8 | 2026-09-04 | P2-2 pop_char 多字节测试 | ✅ 完成（新增 emoji/CJK 测试用例）|
