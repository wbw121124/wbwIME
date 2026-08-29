# wbwIME Windows TSF 输入法计划

## 当前状态

### 已完成
- 词典/FST/匹配/排序/N-gram/CLI — 全部完成，137+ 测试通过
- wbw-ime-native C API cdylib
- wbw-ime-fbterm Linux fbterm IM 服务端
- scripts/install.ps1 / uninstall.ps1 一键安装/卸载
- wbw-ime-tsf — 已用 windows-sys 纯手写 vtable 重写，Phase 1 编译通过、clippy 全绿
- CI 修复 — 修复 wbw-ime-tsf 的 5 个 clippy 错误（run 33158385768 失败根因）：
  - `output.rs`：`tsf_insert_text`/`tsf_start_composition`/`tsf_update_composition` 改为 `unsafe fn` 并补 `# Safety`（`not_unsafe_ptr_arg_deref`）
  - `text_service.rs`：`advise_key_sink`/`unadvise_key_sink` 补 `# Safety`（`missing_safety_doc`）
  - `output.rs`：`#![allow(clippy::missing_const_for_thread_local)]`（x86_64-pc-windows-gnu 目标上的已知误报，rust-clippy#13422）

### 进行中
- wbw-ime-gui — Qt 候选窗口（见下方计划，未开始实现，等待确认）

## 参考项目研究

### 1. Microsoft SampleIME (C++)
- 完整 TSF TIP 实现，5000+ 行 C++
- 所有 COM 接口手动 vtable 定义
- 包含: ITfTextInputProcessorEx, ITfKeyEventSink, ITfThreadMgrEventSink, ITfDisplayAttributeProvider 等

### 2. imekit (Rust)
- 仓库: https://github.com/SergioRibera/imekit
- **关键发现**: `windows 0.59` + `windows-core 0.59` 的 `#[implement]` 宏可以实现 COM 接口
- 但 imekit 是**应用端**（调用 TSF 插入文本），不是 DLL 端
- 使用 `ITfInsertAtSelection` + `ITfEditSession` + `ITfContextComposition` 做文本输出
- 剪贴板 + `SendInput` 作为回退方案

### 3. afrim (Rust)
- 仓库: https://github.com/fodydev/afrim
- 架构分层: engine (preprocessor + translator) / memory / config / service
- TSF 前端在单独的 `afrim-wish` crate（用 Tcl/Tk GUI）
- 不直接实现 TSF DLL，而是独立应用 + 前端

### 4. weasel/小狼毫 (C++)
- 基于 librime 的 TSF 实现
- 完整的候选窗口、合成、显示属性

## TSF DLL 技术方案

### 架构决策

**方案 A: 混合 vtable（当前方案）**
- TIP 生命周期（ITfTextInputProcessorEx）→ 手动 vtable（windows crate 没有 TIP 接口）
- 文本输出 → windows crate 的 ITfInsertAtSelection / ITfContextComposition
- COM 基础设施 → windows crate 的 IClassFactory

**方案 B: 全手动 vtable（Microsoft SampleIME 风格）**
- 所有 COM 接口都手动定义 vtable
- 更可控，但代码量大

**方案 C: 用 afrim 的 service 架构**
- 将输入法引擎作为独立 service
- TSF DLL 只做 IPC 通信
- 更模块化，但架构复杂

### 选择方案 A（混合 vtable）

#### 需要实现的接口

| 接口 | 用途 | 实现方式 |
|------|------|----------|
| IClassFactory | COM 类工厂 | windows crate `#[implement]` |
| ITfTextInputProcessorEx | TIP 主接口 | 手动 vtable |
| ITfKeyEventSink | 按键事件 | 手动 vtable |
| ITfThreadMgrEventSink | 线程管理事件 | 手动 vtable（可选） |
| ITfCompositionSink | 合成生命周期 | 手动 vtable（可选） |
| ITfDisplayAttributeProvider | 显示属性 | 手动 vtable（可选） |

#### 核心流程

```
DllMain (DLL_PROCESS_ATTACH)
  └─ 加载词典到 IME_STATE

DllGetClassObject (CLSID_WBW_IME)
  └─ 返回 IClassFactory

IClassFactory::CreateInstance (ITfTextInputProcessorEx)
  └─ 返回 TextServiceCOM

ITfTextInputProcessorEx::Activate (ITfThreadMgr)
  ├─ 获取 client_id
  ├─ 注册 ITfKeyEventSink (AdviseKeyEventSink)
  └─ 存储 thread_mgr 引用

ITfKeyEventSink::OnTestKeyDown / OnKeyDown
  ├─ 检查按键是否需要处理
  ├─ 调用 ImeState::process_key
  ├─ 候选词匹配
  └─ 输出文本（剪贴板 + SendInput）

ITfTextInputProcessorEx::Deactivate
  ├─ 取消注册 ITfKeyEventSink
  ├─ 释放 thread_mgr
  └─ 清理状态
```

#### 注册表配置

```
HKLM\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200804
  ├─ Ime File = "C:\path\to\wbw_ime_tsf.dll"
  ├─ Layout Text = "wbwIME"
  └─ Language Id = 0x0804

HKLM\SOFTWARE\Microsoft\CTF\TIP\{CLSID}\LanguageProfile\0x0804\{CLSID}
  ├─ Description = "wbwIME Pinyin Input Method"
  └─ Enable = 1
```

## 下一步

### Phase 1: 编译通过
1. ~~重写 Cargo.toml 使用 windows 0.59 + windows-core 0.59~~
2. ~~重写 lib.rs（DLL 导出 + 全局状态）~~
3. ~~重写 text_service.rs（TIP vtable）~~
4. ~~重写 output.rs（剪贴板输出）~~
5. **修复编译错误**（当前 5 个错误）
   - `Interface::IID` → 定义 `IID_IUNKNOWN` 常量
   - `AdviseEventSink` → `AdviseKeyEventSink`，参数签名不同
   - `SetClipboardData` 的 HANDLE 类型转换
   - `IClassFactory.query_interface` 方法不存在
   - `RegCreateKeyW` 参数类型

### Phase 2: 基本功能
1. 编译通过
2. clippy 无警告
3. regsvr32 注册成功
4. Windows 设置中出现输入法
5. 基本按键处理（A-Z 输入，空格选词）

### Phase 3: TSF 直接输出
1. 实现 ITfInsertAtSelection 文本插入
2. 实现 ITfContextComposition 合成显示
3. 候选窗口跟随光标

### Phase 4: 完善
1. ITfDisplayAttributeProvider（合成文本样式）
2. ITfThreadMgrEventSink（焦点跟踪）
3. ITfActiveLanguageProfileNotifySink（语言切换）
4. 候选窗口 UI（自绘或系统候选）

## 文件结构

```
crates/wbw-ime-tsf/
├── Cargo.toml          # windows 0.59 + windows-core 0.59
├── src/
│   ├── lib.rs          # DLL 导出 + 全局状态
│   ├── text_service.rs # TIP vtable + DllGetClassObject + 注册
│   └── output.rs       # 剪贴板 + SendInput 输出
```

## 关键依赖

```toml
windows = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Security",
    "Win32_System_Com",
    "Win32_System_Registry",
    "Win32_System_Memory",
    "Win32_System_DataExchange",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_TextServices",
] }
windows-core = "0.59"  # for #[implement] macro
```

## 待解决的问题

1. **AdviseKeyEventSink 签名**: windows 0.59 的 `ITfKeystrokeMgr::AdviseKeyEventSink` 接受 `P1: Param<ITfKeyEventSink>`，不能直接传 `*mut c_void`
2. **ITfThreadMgr::Activate**: 返回 `Result<u32>`，需要确认 client_id 用途
3. **剪贴板输出延迟**: 50ms sleep + Ctrl+V 可能不够可靠
4. **多线程安全**: IME_STATE 用 Mutex，TSF 可能在不同线程调用

---

## Qt 候选窗口计划：wbw-ime-gui

### 动机

README 声称有"候选窗口"功能，但现状只有 `wbw-imekit::CandidateWindow` 的纯逻辑模型（分页/选择/样式字段齐备，`render()` 只是 `println!` 到控制台），没有任何真实 GUI。真正的候选展示端只有 CLI 文本输出、fbterm（交由终端按 `ImWin` 渲染）、native C ABI（返回数据结构）。需要一个真正的 Qt 候选窗口。

### 决策（已与用户确认）

- **技术路线**：`qmetaobject` crate（纯 Rust Qt/QML 绑定，无需写 C++），QML 内联定义 UI
- **功能范围**：完整接入 IME 按键流程（拼音输入 → 候选更新 → 选词上屏 → 翻页）
- **CI 策略**：新增 cargo feature `qt`（默认关闭），`[[bin]] required-features = ["qt"]`。CI 无 Qt，默认 feature 下 bin 被跳过、qmetaobject 不编译，CI 保持全绿。

### 目录结构

```plain
crates/wbw-ime-gui/
├── Cargo.toml          # 依赖 wbw-types/dict/matcher/rank/imekit；qmetaobject = { optional = true }
├── src/lib.rs          # Engine：WbwIme 封装（ImeHost + Matcher + Ranker），纯逻辑、可单元测试（编译不依赖 Qt）
└── src/main.rs         # `wbwime-qt` 二进制：QML 应用入口（#[cfg(feature = "qt")]）
```

### Cargo.toml 要点

```toml
[features]
default = []
qt = ["dep:qmetaobject"]

[lib]
name = "wbw_ime_gui"

[[bin]]
name = "wbwime-qt"
path = "src/main.rs"
required-features = ["qt"]

[dependencies]
wbw-imekit = { path = "../wbw-imekit" }
wbw-types = { path = "../wbw-types" }
wbw-matcher = { path = "../wbw-matcher" }
wbw-rank = { path = "../wbw-rank" }
wbw-dict = { path = "../wbw-dict" }
qmetaobject = { version = "0.8", optional = true }
```

### 引擎层（lib.rs，无 Qt 依赖）

复用 `wbw-ime-native` 的架构（不依赖其 cdylib，直接持有引擎）：

```rust
pub struct WbwIme {
    host: ImeHost,      // wbw-imekit 状态机
    matcher: Matcher,   // wbw-matcher 匹配
    ranker: Ranker,     // wbw-rank 排序
}
```

- `new(dict_path)`：从 `.cin`/`.fst` 加载词典（同 native `wbw_ime_create`）
- `process(key_code: u32, char: Option<char>) -> WbwImeView`：
  1. `host.process_key(KeyEvent)` 驱动状态机（输入/删除/确认/取消/翻页/选择）
  2. 响应为 `InputChar` 时：`matcher.match_input(&InputContext{buffer,...})` → `ranker.rank()` 生成候选
  3. 数字键 1-9 在候选非空时 → `select_candidate(idx)`；空格 → 选首个候选确认（IME 层补充按键映射）
  4. 返回统一视图 `WbwImeView { buffer, candidates, selected_index, confirmed_text, visible }`
- 单元测试直接写进 lib.rs（无需 Qt，CI 可跑）

### QML 层（main.rs，仅 feature "qt"）

- `#[derive(QObject)] struct ImeQml`：
  - 属性：`buffer`、`candidates`（`QVariantList<String>`）、`selectedIndex`、`visible`、`document`（已上屏文本）
  - `#[invokable]`：`processKey(key: i32, text: QString)`、`selectCandidate(i)`
- QML（内联字符串，`QmlEngine::load_data`）：
  - 根 `Item`（`focus: true` + `Keys.onPressed`）捕获全部按键 → `ime.processKey(...)`
  - 顶部：文档区 `Text`（上屏文本，可换行）
  - 底部：候选条 `Rectangle`（合成缓冲区 + 候选列表 `ListView`/`Repeater`，选中高亮）
- 需要本机 Qt6：`qmake6` 在 PATH 或设 `Qt6_DIR`；运行 `cargo run -p wbw-ime-gui --features qt --bin wbwime-qt -- resources/dicts/base.cin`

### 验证

1. `cargo build --workspace`（无 qt feature）— CI 各 job 不受影响，全绿
2. `cargo run -p wbw-ime-gui --features qt --bin wbwime-qt -- <dict.cin>` — GUI 演示：输拼音 → 候选出现 → 数字/空格选词 → 上屏
