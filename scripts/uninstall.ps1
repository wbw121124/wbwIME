# uninstall.ps1 — wbwIME 卸载脚本
# 用法: 以管理员权限运行
#   pwsh -ExecutionPolicy Bypass -File scripts/uninstall.ps1

$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\wbwIME"

Write-Host "=== wbwIME 卸载程序 ===" -ForegroundColor Cyan
Write-Host ""

# ---------- 1. 权限检查 ----------
Write-Host "[1/3] 检查管理员权限..." -ForegroundColor Yellow
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator
)
if (-not $isAdmin) {
    Write-Host "  需要管理员权限" -ForegroundColor Red
    Write-Host "  pwsh -ExecutionPolicy Bypass -File `"$($MyInvocation.MyCommand.Path)`""
    exit 1
}
Write-Host "  OK" -ForegroundColor Green

# ---------- 2. 注销 COM ----------
Write-Host "`n[2/3] 注销 TSF 输入法..." -ForegroundColor Yellow
$tsfDllPath = Join-Path $InstallDir "wbw_ime_tsf.dll"
if (Test-Path $tsfDllPath) {
    & regsvr32 /s /u "$tsfDllPath" 2>&1
    Write-Host "  COM 已注销" -ForegroundColor Green
}

# 删除注册表项
$clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}"
$comKey = "HKLM:\SOFTWARE\Classes\CLSID\$clsid"
$tipKey = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid"

if (Test-Path $comKey) {
    Remove-Item -Recurse -Force $comKey
    Write-Host "  已删除 COM 注册项" -ForegroundColor Green
}
if (Test-Path $tipKey) {
    Remove-Item -Recurse -Force $tipKey
    Write-Host "  已删除 TSF 配置项" -ForegroundColor Green
}

# ---------- 3. 删除文件 ----------
Write-Host "`n[3/3] 删除安装文件..." -ForegroundColor Yellow
if (Test-Path $InstallDir) {
    Remove-Item -Recurse -Force $InstallDir
    Write-Host "  已删除 $InstallDir" -ForegroundColor Green
}

# 从 PATH 移除
$currentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
if ($currentPath -like "*$InstallDir*") {
    $newPath = ($currentPath -split ";" | Where-Object { $_ -ne $InstallDir }) -join ";"
    [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
    Write-Host "  已从 PATH 移除" -ForegroundColor Green
}

# ---------- 完成 ----------
Write-Host ""
Write-Host "=== 卸载完成 ===" -ForegroundColor Green
Write-Host "  提示: 如需重新安装，运行 scripts/install.ps1"
