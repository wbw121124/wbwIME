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
$targetDir = "$ProjectRoot\target\release"
if (-not (Test-Path $targetDir)) {
    $targetDir = "$ProjectRoot\target\debug"
}
Write-Host "  构建目录: $targetDir"

$tsfDll = Join-Path $targetDir "wbw_ime_tsf.dll"
$nativeDll = Join-Path $targetDir "wbw_ime_native.dll"
$cliExe = Join-Path $targetDir "wbwime.exe"

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

if (Test-Path $tsfDll) {
    Copy-Item $tsfDll $InstallDir -Force
    $filesToCopy += "wbw_ime_tsf.dll"
}
if (Test-Path $nativeDll) {
    Copy-Item $nativeDll $InstallDir -Force
    $filesToCopy += "wbw_ime_native.dll"
}
if (Test-Path $cliExe) {
    Copy-Item $cliExe $InstallDir -Force
    $filesToCopy += "wbwime.exe"
}

# 复制词典
$dictsSrc = Join-Path $ProjectRoot "resources\dicts"
if (Test-Path $dictsSrc) {
    $dictsDst = Join-Path $InstallDir "dicts"
    if (-not (Test-Path $dictsDst)) {
        New-Item -ItemType Directory -Path $dictsDst -Force | Out-Null
    }
    Copy-Item -Recurse "$dictsSrc\*" $dictsDst -Force
    $filesToCopy += "dicts\"
    Write-Host "  已复制词典目录" -ForegroundColor Green
}

# 复制配置
$configSrc = Join-Path $ProjectRoot "resources\config.toml"
if (Test-Path $configSrc) {
    $configDst = Join-Path $InstallDir "config.toml"
    Copy-Item $configSrc $configDst -Force
    $filesToCopy += "config.toml"
}

# 复制头文件
$headerSrc = Join-Path $ProjectRoot "crates\wbw-ime-native\include\wbw_ime_native.h"
if (Test-Path $headerSrc) {
    $includeDir = Join-Path $InstallDir "include"
    if (-not (Test-Path $includeDir)) {
        New-Item -ItemType Directory -Path $includeDir -Force | Out-Null
    }
    Copy-Item $headerSrc $includeDir -Force
    $filesToCopy += "include\wbw_ime_native.h"
}

Write-Host "  已复制 $($filesToCopy.Count) 个项目" -ForegroundColor Green

# ---------- 5. 注册 TSF 输入法 ----------
Write-Host "`n[5/6] 注册 TSF 输入法..." -ForegroundColor Yellow
$tsfDllPath = Join-Path $InstallDir "wbw_ime_tsf.dll"

if (Test-Path $tsfDllPath) {
    # 注册 COM DLL
    Write-Host "  注册 COM 服务器..."
    $regResult = & regsvr32 /s /i "$tsfDllPath" 2>&1
    if ($LASTEXITCODE -eq 0) {
        Write-Host "  COM 注册成功" -ForegroundColor Green
    } else {
        Write-Host "  COM 注册失败 (错误码: $LASTEXITCODE)" -ForegroundColor Red
        Write-Host "  请手动运行: regsvr32 `"$tsfDllPath`""
    }

    # 添加 TSF 语言配置
    $clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}"
    $tipKey = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid"

    if (-not (Test-Path $tipKey)) {
        New-Item -Path $tipKey -Force | Out-Null
    }
    Set-ItemProperty -Path $tipKey -Name "Description" -Value "wbwIME" -Force

    $langProfileKey = "$tipKey\LanguageProfile\0x00000804\{00000000-0000-0000-0000-000000000000}"
    if (-not (Test-Path $langProfileKey)) {
        New-Item -Path $langProfileKey -Force | Out-Null
    }
    Set-ItemProperty -Path $langProfileKey -Name "Description" -Value "wbwIME" -Force
    Set-ItemProperty -Path $langProfileKey -Name "Display Description" -Value "wbwIME" -Force

    Write-Host "  TSF 配置已写入注册表" -ForegroundColor Green
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
