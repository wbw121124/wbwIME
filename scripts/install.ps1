# install.ps1 — wbwIME Windows 一键安装脚本
# 用法: 以管理员权限运行
#   pwsh -ExecutionPolicy Bypass -File scripts/install.ps1
#
# 功能:
#   1. 复制 TSF DLL 到安装目录
#   2. 注册 COM 服务器 (regsvr32)
#   3. 添加 TSF 输入法配置
#   4. 复制词典和配置文件

$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\wbwIME"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$ProjectRoot = Split-Path -Parent $ScriptDir

Write-Host "=== wbwIME 安装程序 ===" -ForegroundColor Cyan
Write-Host ""

# ---------- 1. 权限检查 ----------
Write-Host "[1/6] 检查管理员权限..." -ForegroundColor Yellow
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    Write-Host "  需要管理员权限，请右键选择'以管理员身份运行'" -ForegroundColor Red
    Write-Host ""
    Write-Host "  或在管理员 PowerShell 中执行:" -ForegroundColor Yellow
    Write-Host "  pwsh -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Path)`""
    exit 1
}
Write-Host "  已获得管理员权限" -ForegroundColor Green

# ---------- 2. 查找构建产物 ----------
Write-Host "`n[2/6] 查找构建产物..." -ForegroundColor Yellow
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition

# 分发包模式：DLL/EXE 与 install.ps1 同目录
$tsfDll = Join-Path $scriptDir "wbw_ime_tsf.dll"
$nativeDll = Join-Path $scriptDir "wbw_ime_native.dll"
$cliExe = Join-Path $scriptDir "wbwime.exe"

# 构建模式：查找 target/release 或 target/debug
if (-not (Test-Path $tsfDll) -and -not (Test-Path $cliExe)) {
    $targetDir = "$ProjectRoot\target\release"
    $releaseOk = (Test-Path $targetDir) -and ((Test-Path (Join-Path $targetDir "wbw_ime_tsf.dll")) -or (Test-Path (Join-Path $targetDir "wbwime.exe")))
    if (-not $releaseOk) {
        $targetDir = "$ProjectRoot\target\debug"
    }
    Write-Host "  构建目录: $targetDir"
    if (-not $releaseOk) {
        Write-Host "  警告: 使用 debug 版 (体积大、速度慢)。建议先运行 cargo build --release" -ForegroundColor DarkYellow
    }
    $tsfDll = Join-Path $targetDir "wbw_ime_tsf.dll"
    $nativeDll = Join-Path $targetDir "wbw_ime_native.dll"
    $cliExe = Join-Path $targetDir "wbwime.exe"
}

$foundFiles = @()
if (Test-Path $tsfDll) { $foundFiles += "wbw_ime_tsf.dll (TSF 输入法)" }
if (Test-Path $nativeDll) { $foundFiles += "wbw_ime_native.dll (C API)" }
if (Test-Path $cliExe) { $foundFiles += "wbwime.exe (命令行工具)" }

if ($foundFiles.Count -eq 0) {
    Write-Host "  未找到构建产物，请先运行:" -ForegroundColor Red
    Write-Host "  cargo build --release"
    exit 1
}
Write-Host "  找到文件:" -ForegroundColor Green
foreach ($f in $foundFiles) {
    Write-Host "    - $f"
}

# ---------- 3. 创建安装目录 ----------
Write-Host "`n[3/6] 创建安装目录..." -ForegroundColor Yellow
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
Write-Host "  $InstallDir"

# ---------- 4. 复制文件 ----------
Write-Host "`n[4/6] 复制文件..." -ForegroundColor Yellow
$filesToCopy = @()

# 若已安装的输入法 DLL 正被系统占用（TSF 加载了它），复制会失败。
# 尝试移除 COM 注册以释放占用，若仍失败则给出提示。
function Copy-TsfDll {
    param([string]$Src, [string]$DstDir)
    for ($i = 0; $i -lt 3; $i++) {
        try {
            Copy-Item $Src $DstDir -Force -ErrorAction Stop
            return $true
        } catch {
            Start-Sleep -Milliseconds 300
        }
    }
    return $false
}

if (Test-Path $tsfDll) {
    if (Copy-TsfDll $tsfDll $InstallDir) {
        $filesToCopy += "wbw_ime_tsf.dll"
    } else {
        Write-Host "  错误: 无法覆盖 wbw_ime_tsf.dll（正被占用）。" -ForegroundColor Red
        Write-Host "  请先关闭/移除当前使用的 wbwIME 输入法，然后重试。" -ForegroundColor Red
        Write-Host "  或在管理员 PowerShell 中执行: regsvr32 /u `"$InstallDir\wbw_ime_tsf.dll`" 后重试。" -ForegroundColor Yellow
        exit 1
    }
}
if (Test-Path $nativeDll) {
    Copy-Item $nativeDll $InstallDir -Force
    $filesToCopy += "wbw_ime_native.dll"
}
if (Test-Path $cliExe) {
    Copy-Item $cliExe $InstallDir -Force
    $filesToCopy += "wbwime.exe"
}

# 复制词典 — 先查分发包目录，再查项目目录
$dictsSrc = Join-Path $scriptDir "dicts"
if (-not (Test-Path $dictsSrc)) {
    $dictsSrc = Join-Path $ProjectRoot "resources\dicts"
}
if (Test-Path $dictsSrc) {
    $dictsDst = Join-Path $InstallDir "dicts"
    if (-not (Test-Path $dictsDst)) {
        New-Item -ItemType Directory -Path $dictsDst -Force | Out-Null
    }
    Copy-Item -Recurse "$dictsSrc\*" $dictsDst -Force
    $filesToCopy += "dicts\"
    Write-Host "  已复制词典目录" -ForegroundColor Green
}

# 复制配置 — 先查分发包目录，再查项目目录
$configSrc = Join-Path $scriptDir "config.toml"
if (-not (Test-Path $configSrc)) {
    $configSrc = Join-Path $ProjectRoot "resources\config.toml"
}
if (Test-Path $configSrc) {
    $configDst = Join-Path $InstallDir "config.toml"
    Copy-Item $configSrc $configDst -Force
    $filesToCopy += "config.toml"
}

# 复制头文件 — 先查分发包目录，再查项目目录
$headerSrc = Join-Path $scriptDir "include\wbw_ime_native.h"
if (-not (Test-Path $headerSrc)) {
    $headerSrc = Join-Path $ProjectRoot "crates\wbw-ime-native\include\wbw_ime_native.h"
}
if (Test-Path $headerSrc) {
    $includeDir = Join-Path $InstallDir "include"
    if (-not (Test-Path $includeDir)) {
        New-Item -ItemType Directory -Path $includeDir -Force | Out-Null
    }
    Copy-Item $headerSrc $includeDir -Force
    $filesToCopy += "include\wbw_ime_native.h"
}

# 复制图标（TSF 输入法在"设置"里显示的图标，DllRegisterServer 会把它写进 IconFile）
$iconSrc = Join-Path $ProjectRoot "resources\wbwime.ico"
if (Test-Path $iconSrc) {
    Copy-Item $iconSrc (Join-Path $InstallDir "wbwime.ico") -Force
    $filesToCopy += "wbwime.ico（输入法图标）"
}

Write-Host "  已复制 $($filesToCopy.Count) 个项目" -ForegroundColor Green

# ---------- 5. 注册 TSF 输入法 ----------
Write-Host "`n[5/6] 注册 TSF 输入法..." -ForegroundColor Yellow
$tsfDllPath = Join-Path $InstallDir "wbw_ime_tsf.dll"

if (Test-Path $tsfDllPath) {
    # 注册 COM DLL
    Write-Host "  注册 COM 服务器..."

    # 校验被注册的 DLL 是有效且非空的文件（避免复制不完整/损坏导致 regsvr32 崩溃）
    $fileInfo = Get-Item $tsfDllPath
    if ($fileInfo.Length -eq 0) {
        Write-Host "  错误: 未发现有效 DLL ($tsfDllPath)" -ForegroundColor Red
        exit 1
    }

    $regProc = Start-Process regsvr32.exe -ArgumentList "/s", "`"$tsfDllPath`"" -Wait -PassThru -WindowStyle Hidden
    $regExit = $regProc.ExitCode
    if ($regExit -eq 0) {
        Write-Host "  COM 注册成功" -ForegroundColor Green
        Write-Host "  TSF 配置已由 DllRegisterServer 完整写入" -ForegroundColor Green
    } else {
        Write-Host "  COM 注册失败 (错误码: $regExit)" -ForegroundColor Red
        # 0xC000013A = STATUS_CONTROL_C_EXIT：通常是 DLL 加载崩溃（DllMain 加载器锁下的重工作）
        if (($regExit -band 0xFFFFFFFF) -eq 0xC000013A) {
            Write-Host "  提示: 0xC000013A 通常是 DLL 加载失败/崩溃。请确认:"
            Write-Host "    1) DLL 为 64 位且完整复制: Get-Item `"$tsfDllPath`""
            Write-Host "    2) 使用 release 构建 (cargo build --release)"
            Write-Host "    3) 手动尝试: regsvr32 `"$tsfDllPath`"（看错误对话框）"
        } else {
            Write-Host "  请手动运行: regsvr32 `"$tsfDllPath`""
        }
        exit 1
    }
} else {
    Write-Host "  TSF DLL 不存在，跳过注册" -ForegroundColor DarkYellow
}

# ---------- 6. 添加 PATH ----------
Write-Host "`n[6/6] 配置环境变量..." -ForegroundColor Yellow
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -notlike "*$InstallDir*") {
    $newPath = "$InstallDir;$currentPath"
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    Write-Host "  已添加 $InstallDir 到用户 PATH" -ForegroundColor Green
} else {
    Write-Host "  PATH 已包含安装目录" -ForegroundColor Green
}

# ---------- 完成 ----------
Write-Host ""
Write-Host "=== 安装完成 ===" -ForegroundColor Green
Write-Host ""
Write-Host "安装目录: $InstallDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "使用方法:" -ForegroundColor Yellow
Write-Host "  1. 打开 Windows 设置 > 时间和语言 > 语言和区域"
Write-Host "  2. 添加语言 > 搜索 '中文' > 安装中文(简体)"
Write-Host "  3. 在语言选项中添加 'wbwIME' 输入法"
Write-Host "  4. 使用 Ctrl+Space 切换输入法"
Write-Host ""
Write-Host "命令行工具:" -ForegroundColor Yellow
Write-Host "  wbwime build-fst resources/dicts/cs-oi.cin"
Write-Host "  wbwime query resources/dicts/cs-oi.fst"
Write-Host "  wbwime interactive resources/dicts/cs-oi.fst"
Write-Host ""
Write-Host "卸载: pwsh scripts/uninstall.ps1" -ForegroundColor DarkGray
