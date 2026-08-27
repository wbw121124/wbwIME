# .cin 码表格式说明

## 概述

.cin（Chinese Input）是一种广泛使用的输入法码表格式，最初由酷仓输入法方案定义。wbwIME 完整支持该格式，并在此基础上扩展了部分功能。

## 文件编码

- 推荐使用 **UTF-8** 编码
- 支持 **GBK/GB2312** 编码（需指定编码参数）

## 基本格式

每行一个词条，格式为 `编码 汉字`，中间以空格分隔：

```
编码 汉字
```

- **编码**：按键序列（如拼音 `wo`、五笔 `r`）
- **汉字**：对应的中文词文本

## 注释行

以 `%` 开头的行视为注释，会被解析器跳过：

```
% 这是一条注释
wo 我
% 下面是常用字
ai 爱
```

## 示例码表

```
% 基础码表示例
% 编码：拼音全拼
% 汉字：常用汉字

wo 我
wo 喔
wo 涡
ai 爱
ai 哎
ai 唉
ni 你
ni 泥
ni 尼
ta 他
ta 她
ta 它
hao 好
hao 豪
hao 嚎
shi 是
shi 时
shi 十
shi 石
shi 使
shi 师
de 的
de 地
de 得
le 了
le 乐
bu 不
bu 步
bu 布
bu 部
da 大
da 打
da 达
```

## 多编码词条

同一汉字可以有多个编码（多行）：

```
% "我" 的多种输入方式
wo 我       # 拼音输入
wg 我       # 双拼输入
r 我        # 五笔输入
```

## 词频标注

部分输入法格式支持词频标注，格式为 `编码 汉字 词频`：

```
% 编码 汉字 词频
wo 我 10000
ai 爱 8000
ni 你 9000
```

wbwIME 会在解析时提取词频值（如果存在）。

## 分组标记

使用 `%gen` 标记定义分组：

```
%gen 拼音
wo 我
ai 爱

%gen 符号
. 句号
, 逗号
; 分号
```

## 特殊编码

### 单键编码
```
a 啊
o 哦
e 额
```

### 长编码
```
zhuang 壮
chuang 创
shuang 双
```

### 混合编码
```
% 数字+汉字
0 零
1 一
2 二
3 三
```

## wbwIME 扩展

### 词性标注

使用 `#pos` 注释标记词性：

```
% #pos=noun
wo 我
ta 他

% #pos=verb
ai 爱
shi 是
```

### 置顶标记

使用 `#top` 注释标记置顶词：

```
% #top
de 的
shi 是
le 了
```

### 分类标记

使用 `#cat` 注释标记分类：

```
% #cat=common
wo 我
ni 你

% #cat=technical
cpu 处理器
ram 内存
```

## 解析规则

1. **跳过空行** — 空行不会被解析
2. **跳过注释** — `%` 开头的行被跳过
3. **空格分隔** — 编码和汉字之间以空格分隔
4. **编码去重** — 相同编码的词条会被合并
5. **长度限制** — 默认最大编码长度 32 字符

## 码表统计

解析后的码表统计信息：

```rust
struct DictStats {
    total_entries: usize,     // 总词条数
    total_codes: usize,       // 总编码数
    avg_words_per_code: f64,  // 平均每编码词条数
    top_words: Vec<(String, u32)>,  // 最高频词
}
```

## 常见码表来源

| 来源 | 说明 | 推荐 |
|------|------|------|
| 酷仓方案 | 原始 .cin 格式 | ✅ |
| 超级拼音 | 兼容格式 | ✅ |
| 五笔方案 | 需要词频 | ⚠️ |
| 自定义方案 | 需验证格式 | ⚠️ |

## 工具支持

```bash
# 验证码表格式
wbwime validate --dict path/to/file.cin

# 显示码表统计
wbwime stats --dict path/to/file.cin

# 合并多个码表
wbwime build-dict --input a.cin --input b.cin --output merged.cin

# 转换格式
wbwime build-dict --input source.cin --output target.json --format json
```
