# wbwIME 文档 P0-P4 全级别审查报告

**审查日期：** 2026-09-06
**审查范围：** plan.md, README.md, resources/config.toml

## 发现的问题

### P0 — 严重问题（阻碍发布）

| # | 问题 | 文件 | 说明 |
|---|------|------|------|
| P0-1 | plan.md 文档结构严重混乱，无法判断代码真实状态 | plan.md | 包含 Round 1-17 共 17 轮审查记录，多个"终审"章节互相矛盾（如 Round 7、Round 13 均声称"终审"），修复统计表在 3+ 处重复出现且数据可能不一致，导致发布决策无法基于文档做出。 |

### P1 — 高优先级问题

| # | 问题 | 文件 | 说明 |
|---|------|------|------|
| P1-1 | plan.md 修复统计表重复且数据不一致 | plan.md:183-190, 449-464, 565-582 | 多处统计表声称"累计 25+ 个问题已修复"，但各表数据存在冲突（如 Round 9 修复数分别为 3 和 4，Round 11 修复数分别为 4 和 3）。 |
| P1-2 | plan.md 待办事项未勾选 | plan.md:279, 308 | "实机验证"等关键待办事项未标记完成，可能导致遗漏发布前验证步骤。 |
| P1-3 | README.md 缺少安装路径详细说明 | README.md:145-151 | 安装路径部分过于简略，未说明安装脚本（install.ps1）与代码默认路径的对应关系，可能导致用户配置错误。 |

### P2 — 中等优先级问题

| # | 问题 | 文件 | 说明 |
|---|------|------|------|
| P2-1 | README.md 缺少故障排除/FAQ 部分 | README.md | 未提供常见问题解答，如"安装后无窗口"、"按键无反应"等已在 plan.md 中记录的问题的解决方案。 |
| P2-2 | config.toml 缺少安装后路径说明 | config.toml:1-8 | 配置文件中的路径（如 `resources/dicts/pinyin.cin`）是开发路径，未说明安装后应改为 `%LOCALAPPDATA%\wbwIME\dicts\pinyin.cin`。 |
| P2-3 | plan.md 文档膨胀（1740行） | plan.md | 文档过长，包含大量已修复问题的详细记录，不利于快速查阅当前状态。 |
| P2-4 | README.md 缺少 wbw-cli 用法示例 | README.md:15 | 功能特性中提到"CLI 工具"，但未提供基本使用示例。 |

### P3 — 低优先级问题

| # | 问题 | 文件 | 说明 |
|---|------|------|------|
| P3-1 | README.md 依赖关系图缺少 wbw-ime-fbterm | README.md:44-54 | 依赖图未包含 wbw-ime-fbterm crate，与项目结构不完全一致。 |
| P3-2 | config.toml smooth_method 字段缺少可选值注释 | config.toml:38 | 注释中说明"当前仅 laplace 完整实现"，但未列出所有可选值（good_turing, kneser_ney），且 kneser_ney 在代码中未实现。 |
| P3-3 | README.md 未提及 LICENSE 文件位置 | README.md:146 | 许可证部分仅写"MPL-2.0"，未说明 LICENSE 文件在仓库中的位置。 |
| P3-4 | plan.md emoji 图标混用 | plan.md 多处 | 使用 🔴🟡🟢 等 emoji 标记问题级别，但未在文档中定义其含义。 |

### P4 — 信息性问题

| # | 问题 | 文件 | 说明 |
|---|------|------|------|
| P4-1 | plan.md 缺少 Round 14-17 修复摘要 | plan.md | Round 14-17 仅列出问题，未像 Round 1-13 那样提供修复验证总结。 |
| P4-2 | README.md 缺少开发环境搭建说明 | README.md | 未说明 Rust 版本要求、依赖安装等开发环境信息。 |
| P4-3 | config.toml 无版本信息 | config.toml | 配置文件未标注适用的程序版本，可能导致版本不匹配。 |

## 修复记录与实际代码一致性

| 修复项 | 状态 | 说明 |
|--------|------|------|
| P0-1 (Cargo.toml 语法错误) | ✅ 一致 | 已跳过（误报），文件正确。 |
| P0-2 (can_split_into_syllables 回溯) | ✅ 一致 | 已添加 memoization（HashMap 缓存）。 |
| P0-3 (fuzzy_lookup 性能) | ⚠️ 部分一致 | 已添加长度剪枝和结果限制，但仍为全表扫描（文档说"使用 fst lev automaton"，实际为长度剪枝）。 |
| P1-1 (统一去重逻辑) | ✅ 一致 | 已添加 dedup_by_text 方法。 |
| P1-2 (SessionState 时间字段语义) | ✅ 一致 | 已添加文档注释说明"内存态，不参与序列化"。 |
| P1-3 (ImeHost session_id 硬编码) | ✅ 一致 | 已改用 rand::random()，新增 with_session_id builder。 |
| P2-1 (CLI run_test_match 缩进错误) | ✅ 一致 | 已修正缩进，加 empty check。 |
| P2-2 (ContextManager pop_char 光标一致性) | ✅ 一致 | 已添加 emoji 和多字节字符测试用例。 |

## 发布建议

**当前状态：存在 P0 问题，不建议发布。**

必须先解决 P0-1（plan.md 文档结构混乱），才能进行发布决策。建议：
1. 重写 plan.md，仅保留当前状态和待办事项，删除历史轮次详细记录。
2. 将已验证的修复记录移至单独的 `CHANGELOG.md` 或 `docs/audit-history.md`。
3. 在 README.md 中补充安装路径详细说明和故障排除章节。
4. 在 config.toml 中添加安装后路径注释。
5. 修正 config.toml 中 smooth_method 的注释，移除未实现的 kneser_ney。
