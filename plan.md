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
- wbw-ime-gui — Qt 候选窗口（见下方计划）：引擎/config 已完成并通过 CI（纯 Rust，无 Qt 可编译测试）；Qt/QML `main.rs` 待本机安装 Qt6 后实机验证

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
├── src/config.rs       # GuiConfig：YAML 主题/行为配置（纯 Rust，可测试）
├── src/engine.rs       # WbwIme：ImeHost + Matcher + Ranker 引擎 + GuiState 视图（纯逻辑、可单元测试）
├── src/lib.rs          # 重导出 config + engine
├── src/main.rs         # `wbw-ime-gui` 二进制：QML 应用入口（required-features = ["qt"]）
```

### Cargo.toml 要点

```toml
[features]
default = []
qt = ["dep:qmetaobject"]

[[bin]]
name = "wbw-ime-gui"
path = "src/main.rs"
required-features = ["qt"]          # 仅在启用 qt 时构建，CI 无 Qt 保持全绿

[dependencies]
wbw-imekit = { path = "../wbw-imekit" }
wbw-types = { path = "../wbw-types" }
wbw-matcher = { path = "../wbw-matcher" }
wbw-rank = { path = "../wbw-rank" }
wbw-dict = { path = "../wbw-dict" }
serde_yaml = "0.9"
qmetaobject = { version = "0.2.10", optional = true, default-features = false }
```

> 注：plan 早期写作 qmetaobject "0.8"，实际 crates.io 最新为 **0.2.10**（0.2.x 系列，README 概述里的 `0.8` 系笔误/误导）。`required-features = ["qt"]` 保证无 Qt 时 bin 与 qmetaobject 均不参与编译。

### 配置层（config.rs，YAML，纯 Rust）

提供高度可配置的候选窗口外观与行为，所有字段可选、缺失用默认值：

```yaml
dict_path: "resources/dicts/base.cin"   # 词典路径（.cin / .fst）
page_size: 10                           # 每页候选词数量
window:                                 # 窗口：背景/边框/圆角/透明度/字体/置顶
buffer_bar:                             # 缓冲栏：可见性/颜色/高度/对齐
candidate_bar:                          # 候选栏：背景/间距/排列方向
candidate_item:                         # 候选条目：文字/选中高亮/内边距/是否显示序号
pagination:                             # 翻页区：可见性/上一页/下一页图标/信息颜色
behavior:                               # 行为：模糊/学习/空格确认/数字选词/回车确认
```

示例配置见 `resources/gui-config.yaml`。

### 引擎层（engine.rs，无 Qt 依赖）

复用 `wbw-ime-native` 的架构（不依赖其 cdylib，直接持有引擎）：

```rust
pub struct WbwIme {
    host: ImeHost,      // wbw-imekit 状态机
    matcher: Matcher,   // wbw-matcher 匹配
    ranker: Ranker,     // wbw-rank 排序
    config: GuiConfig,  // 主题 + 行为
}
```

- `new(config, page_size)`：加载词典（同 native `wbw_ime_create`）；向 `host.window_manager_mut()` 注册一个默认候选窗口并 `set_active_window`（imekit 默认无 active window，需显式设置，否则 confirm/select/翻页不工作）
- `process_key(code, ch) -> GuiState`：
  1. 数字键 1-9 且输入中 → `select_candidate(idx)`（imekit `select_candidate` 不会清空缓冲，engine 在 Confirm 后 `host.reset()` 补清）
  2. 空格且 `space_confirms` → `confirm()`
  3. 字母/数字 → `host.input_char(ch)`（imekit 的 KeyMapper 默认**不映射字母**，需引擎层直接输入）
  4. 其余功能键（Enter/Backspace/Esc/方向/翻页）→ `host.process_key`（imekit 默认映射 Enter13/Backspace8/Esc27/Up38/Down40/PageUp33/PageDown34）
  5. `InputChar`/`DeleteChar` 响应后 → `matcher.match_input` → `ranker.rank` → `window.set_candidates` 注入候选 → `show`
  6. 返回统一视图 `GuiState { buffer, candidates, selected_index, page, total_pages, visible, committed }`
- 单元测试直接写进 engine.rs（无需 Qt，CI 可跑）

### QML 层（main.rs，仅 feature "qt"）

- `#[derive(QObject)] struct CandidateController`（QML 实例化，`qml_register_type::<CandidateController>(cstr!("WbwIme")...)`）：
  - 属性：`buffer`（QString）、`candidates`（QStringList）、`selectedIndex`/`page`/`totalPages`（qint32）、`hasCandidates`（bool），均带 `NOTIFY stateChanged`
  - 方法：`key_pressed(qt_key:i32, shift:bool)` 将 Qt 键值映射到 VK 后调引擎
- 引擎经全局单例 `static ENGINE: Mutex<Option<WbwIme>>` 供控制器访问（Qt 单线程，无竞争）
- 键值映射：Qt 字母 65..=90（含 Shift 大小写）、数字 48..=57、空格 32；Return 16777220→13、Backspace 16777219→8、Esc 16777216→27、Up 16777235→38、Down 16777237→40、PageUp 16777238→33、PageDown 16777239→34
- QML（内联字符串 `QmlEngine::load_data`）：无边框半透明置顶 Window（`Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.WindowDoesNotAcceptFocus`），含缓冲栏 / 候选栏（Repeater）/ 翻页区
- 需要本机 Qt6：`qmake6` 在 PATH 或设 `QT_INCLUDE_PATH`/`QT_LIBRARY_PATH`；运行 `cargo run -p wbw-ime-gui --features qt --bin wbw-ime-gui -- resources/gui-config.yaml`

### 验证

1. `cargo build --workspace`（无 qt feature）— CI 各 job 不受影响，全绿（已通过，含 clippy --all-targets -D warnings）
2. `cargo run -p wbw-ime-gui --features qt --bin wbw-ime-gui -- resources/gui-config.yaml` — GUI 演示：输拼音 → 候选出现 → 数字/空格选词 → 上屏（需本机 Qt6）
