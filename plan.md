# wbwIME Windows TSF 输入法计划

## 当前状态

### 已完成
- 词典/FST/匹配/排序/N-gram/CLI — 全部完成，137+ 测试通过
- wbw-ime-native C API cdylib
- wbw-ime-fbterm Linux fbterm IM 服务端
- scripts/install.ps1 / uninstall.ps1 一键安装/卸载
- CI 全绿

### 进行中（暂停）
- wbw-ime-tsf — Windows TSF 输入法 DLL（编译未通过，暂停）
  - 原因：`windows 0.59` crate 的 `ITfKeystrokeMgr::AdviseKeyEventSink` 签名需要 `Param<ITfKeyEventSink>`，无法直接传手动 vtable 指针
  - 下次继续时选择方案A（windows-sys 纯手写）或方案B（IMM32）

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
