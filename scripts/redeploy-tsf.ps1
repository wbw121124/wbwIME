# redeploy-tsf.ps1: 重装 TSF 输入法 DLL（卸载 -> 复制 -> 重新注册）
# 用法: 以管理员运行
$ErrorActionPreference = "Stop"
$InstallDir = "$env:LOCALAPPDATA\wbwIME"
$SrcDll = "D:\project\wbwIME\target\release\wbw_ime_tsf.dll"
$DstDll = Join-Path $InstallDir "wbw_ime_tsf.dll"
$clsid = "{E8A3B0F2-1234-5678-9ABC-DEF012345678}"

Write-Host "=== 重装 TSF DLL ===" -ForegroundColor Cyan

# 1. 取消注册 COM（可释放 DLL 占用）
if (Test-Path $DstDll) {
    & regsvr32 /s /u "$DstDll" 2>&1
    Write-Host "已注销 COM，尝试释放占用" -ForegroundColor Yellow
    Start-Sleep -Milliseconds 500
}

# 2. 清理注册表
$comKey = "HKLM:\SOFTWARE\Classes\CLSID\$clsid"
$tipKey = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid"
if (Test-Path $comKey) { Remove-Item -Recurse -Force $comKey; Write-Host "已删 CLSID" }
if (Test-Path $tipKey) { Remove-Item -Recurse -Force $tipKey; Write-Host "已删 TIP" }

# 3. 复制新 DLL（重试处理占用）
if (-not (Test-Path $InstallDir)) { New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null }
$copied = $false
for ($i = 0; $i -lt 5; $i++) {
    try { Copy-Item $SrcDll $DstDll -Force -ErrorAction Stop; $copied = $true; break }
    catch { Write-Host "复制失败第 $i 次 (可能占用)，重试..." -ForegroundColor DarkYellow; Start-Sleep -Seconds 1 }
}
if (-not $copied) { Write-Host "无法复制 DLL（被占用）。请关闭所有 wbwIME/文本编辑器后重试。" -ForegroundColor Red; exit 1 }
Write-Host "已复制新 DLL" -ForegroundColor Green

# 4. 重新注册
$regProc = Start-Process regsvr32.exe -ArgumentList "/s", "`"$DstDll`"" -Wait -PassThru -WindowStyle Hidden
Write-Host "注册退出码: $($regProc.ExitCode)" -ForegroundColor Green

Write-Host "=== 完成 ===" -ForegroundColor Green
