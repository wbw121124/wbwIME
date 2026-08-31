# uninstall-clean.ps1: 完全清理 wbwIME 系统安装（注销 COM + 删注册表 + 删 DLL）
# 用法: 以管理员运行
$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\wbwIME"
$DstDll = Join-Path $InstallDir "wbw_ime_tsf.dll"
$clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}"

Write-Host "=== 清理 wbwIME 系统安装 ===" -ForegroundColor Cyan

if (Test-Path $DstDll) {
    & regsvr32 /s /u "$DstDll" 2>&1
    Write-Host "已注销 COM" -ForegroundColor Yellow
    Start-Sleep -Milliseconds 500
}

$comKey = "HKLM:\SOFTWARE\Classes\CLSID\$clsid"
$tipKey = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid"
if (Test-Path $comKey) { Remove-Item -Recurse -Force $comKey; Write-Host "已删 CLSID 注册" }
else { Write-Host "CLSID 注册不存在" }
if (Test-Path $tipKey) { Remove-Item -Recurse -Force $tipKey; Write-Host "已删 TIP 注册" }
else { Write-Host "TIP 注册不存在" }

if (Test-Path $DstDll) {
    $deleted = $false
    for ($i = 0; $i -lt 5; $i++) {
        try { Remove-Item $DstDll -Force -ErrorAction Stop; $deleted = $true; break }
        catch { Write-Host "删除 DLL 失败第 $i 次（占用），重试..." -ForegroundColor DarkYellow; Start-Sleep -Seconds 1 }
    }
    if ($deleted) { Write-Host "已删除 DLL" -ForegroundColor Green }
    else { Write-Host "无法删除 DLL（占用）" -ForegroundColor Red }
} else {
    Write-Host "DLL 不存在" 
}

Write-Host "=== 清理完成 ===" -ForegroundColor Green
