# 架构设计文档

## 概述

wbwIME 是一个用 Rust 编写的自构建输入法引擎，采用工作区（workspace）架构，由 8 个独立 crate 组成。每个 crate 负责单一职责，通过清晰的依赖关系组合成完整的输入法系统。

## 设计目标

1. **高性能** — 基于 FST 词典和内存映射，实现毫秒级响应
2. **可扩展** — 模块化设计，支持自定义码表、排序策略、匹配规则
3. **低内存** — FST 压缩存储 + LRU 缓存，控制内存占用
4. **自学习** — L0 动态学习机制，根据用户习惯自动调整候选词排序

## 核心架构

### 数据流

```
用户按键
  │
  ▼
┌─────────┐    ┌──────────┐    ┌──────────┐
│ imekit   │───▶│  core    │───▶│ matcher  │
│ 按键映射 │    │ 会话管理 │    │ 拼音匹配 │
└─────────┘    └──────────┘    └──────────┘
                    │                │
                    ▼                ▼
              ┌──────────┐    ┌──────────┐
              │  rank    │    │  dict    │
              │ 加权排序 │    │ FST 词典 │
              └──────────┘    └──────────┘
                    │
                    ▼
              ┌──────────┐
              │ ngram    │
              │ 语言模型 │
              └──────────┘
                    │
                    ▼
              候选词列表 → 用户选择 → 输出文本
```

### Crate 职责

| Crate | 职责 | 关键类型 |
|-------|------|----------|
| **wbw-types** | 共享类型定义，打破循环依赖 | `Candidate`, `InputContext`, `Session`, `GlobalConfig` |
| **wbw-dict** | 码表解析与词典管理 | `CinParser`, `FstDict`, `DictBuilder` |
| **wbw-matcher** | 拼音处理、分词与模糊匹配 | `Matcher`, `FuzzyMatcher`, `Segmenter`, `PinyinSyllable` |
| **wbw-ngram** | N-gram 语言模型 | `NgramScorer`, `NgramTable`, `Smoother` |
| **wbw-core** | 会话管理与输入上下文 | `SessionManager`, `ContextManager`, `CandidateList` |
| **wbw-rank** | 候选词排序与动态学习 | `Ranker`, `L0Learner`, `WeightCalculator` |
| **wbw-imekit** | IME 宿主平台适配 | `ImeHost`, `CandidateWindow`, `KeyMapper` |
| **wbw-cli** | 命令行工具入口 | `CliParser`, `CliExecutor` |

### 依赖关系图

```
wbw-types ◄──────────────────────────────────────────┐
    ▲                                                 │
    │                                                 │
wbw-dict ◄── wbw-matcher ◄── wbw-core ◄── wbw-rank ──┘
                  │                  ▲         │
                  │                  │         │
                  └──────────────────┘    wbw-ngram
                                               ▲
                                               │
                                         wbw-imekit
                                               ▲
                                               │
                                          wbw-cli
```

## 模块详解

### 1. wbw-types — 共享类型

所有 crate 共享的类型定义，用于打破循环依赖。

**关键类型：**
- `Candidate` — 候选词数据（词文本、编码、分数、来源）
- `InputContext` — 输入上下文（缓冲区、光标位置、输入模式）
- `Session` — 会话状态（ID、上下文、候选词、配置）
- `GlobalConfig` — 全局配置（词典、匹配器、排序、N-gram）
- `ImeResult<T>` — 统一结果类型

### 2. wbw-dict — 词典模块

负责 .cin 码表解析和 FST 词典管理。

**解析流程：**
```
.cin 文件 → CinParser → Vec<CinEntry> → DictBuilder → FstDict
```

**FST 词典优势：**
- 内存映射加载，零拷贝查询
- 压缩存储，支持大规模词典
- 前缀查询效率 O(k)，k 为编码长度

### 3. wbw-matcher — 匹配模块

处理拼音解析、分词和模糊匹配。

**拼音处理：**
- `PinyinSyllable` — 声母+韵母+声调
- 支持声调标记与去除
- 声韵母有效性验证

**模糊匹配规则：**
- 声母替换：z↔zh, c↔ch, s↔sh
- 鼻音混淆：n↔l
- 韵母混淆：an↔ang, en↔eng, in↔ing

### 4. wbw-ngram — 语言模型

提供 N-gram 概率评分和平滑处理。

**支持的平滑算法：**
- 拉普拉斯平滑（加一平滑）
- Good-Turing 平滑
- Kneser-Ney 平滑
- 插值平滑
- 回退平滑

### 5. wbw-core — 核心模块

管理会话、输入上下文和候选词列表。

**会话管理：**
- 多会话隔离（每个输入框独立会话）
- 上下文历史快照
- 支持撤销操作

**候选词管理：**
- 分页显示
- 键盘导航（上/下/翻页）
- 来源分类（系统/用户/动态/短语）

### 6. wbw-rank — 排序模块

候选词加权排序和 L0 动态学习。

**排序因子：**
- `pin_weight` — 拼音匹配得分（基础权重）
- `user_weight` — 用户词库加成
- `freq_weight` — 词频加成
- `ngram_weight` — N-gram 上下文得分

**L0 学习机制：**
- 记录用户每次选择
- 达到阈值后提升词频
- 支持快照持久化

### 7. wbw-imekit — IME 宿主适配

适配操作系统输入法框架。

**组件：**
- `ImeHost` — IME 状态机（空闲→输入→选择→确认）
- `CandidateWindow` — 候选词窗口（位置、样式、分页）
- `KeyMapper` — 按键映射（可自定义）

### 8. wbw-cli — 命令行工具

提供交互式测试和管理功能。

**命令：**
- `wbwime interactive` — 交互式输入模式
- `wbwime query <code>` — 查询编码
- `wbwime build-dict` — 构建词典
- `wbwime validate` — 验证词典
- `wbwime stats` — 显示统计信息

## 输入处理流程

```
1. 用户按键
   ↓
2. KeyMapper 解析按键 → KeyAction
   ↓
3. ImeHost 根据状态分发：
   - Idle → InputChar / 其他
   - Inputting → 输入字符 / 删除 / 确认
   - Selecting → 选择候选词 / 翻页
   ↓
4. ContextManager 更新缓冲区
   ↓
5. Matcher 查询候选词：
   - PinyinSyllable 解析拼音
   - FstDict 查询匹配词条
   - FuzzyMatcher 生成模糊变体
   ↓
6. Ranker 排序候选词：
   - WeightCalculator 计算权重
   - NgramScorer 上下文评分
   - L0Learner 历史偏好加成
   ↓
7. CandidateWindow 展示候选词
   ↓
8. 用户选择 → 输出文本
```

## 性能设计

### FST 词典
- 内存映射（mmap）加载，避免完整读取
- 查询时间 O(k)，k 为编码长度
- 支持 10 万级词条，内存占用 < 5MB

### LRU 缓存
- 匹配结果缓存（默认 1000 条）
- 缓存最近查询，减少重复计算

### 并发安全
- 会话隔离，无共享可变状态
- 支持多线程并行查询

## 扩展点

1. **自定义码表** — 实现 `CinParser` 支持其他格式
2. **自定义排序** — 实现 `RankStrategy` 特征
3. **自定义模糊规则** — 通过 `FuzzyRule` 配置
4. **自定义平滑算法** — 实现 `SmoothMethod`
5. **IME 平台适配** — 实现 `ImeAdapter` 特征
