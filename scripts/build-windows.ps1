# build-windows.ps1 — Windows 平台 IME 模块构建脚本
# 用法: pwsh ./scripts/build-windows.ps1
# 输出: dist/wbwime-windows-x86_64.zip

$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$DistDir = Join-Path $ProjectRoot "dist"
$BuildProfile = if ($env:RELEASE) { "release" } else { "debug" }

Write-Host "=== wbwIME Windows 构建 ===" -ForegroundColor Cyan
Write-Host "项目根: $ProjectRoot"
Write-Host "构建模式: $BuildProfile"

# ---------- 1. 环境检查 ----------
Write-Host "`n[1/5] 检查 Rust 环境..." -ForegroundColor Yellow
try {
    $rustcVersion = rustc --version 2>&1
    Write-Host "  rustc: $rustcVersion"
} catch {
    Write-Error "未找到 rustc，请先安装 Rust: https://rustup.rs"
    exit 1
}

# ---------- 2. 运行测试 ----------
Write-Host "`n[2/5] 运行测试..." -ForegroundColor Yellow
cargo test --workspace
if ($LASTEXITCODE -ne 0) {
    Write-Error "测试失败"
    exit 1
}

# ---------- 3. Clippy 检查 ----------
Write-Host "`n[3/5] Clippy 检查..." -ForegroundColor Yellow
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Error "Clippy 检查失败"
    exit 1
}

# ---------- 4. 编译 ----------
Write-Host "`n[4/5] 编译项目..." -ForegroundColor Yellow
$cargoArgs = @("build", "--workspace")
if ($BuildProfile -eq "release") {
    $cargoArgs += "--release"
}
& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    Write-Error "编译失败"
    exit 1
}

# ---------- 5. 打包 ----------
Write-Host "`n[5/5] 打包分发..." -ForegroundColor Yellow
if (Test-Path $DistDir) { Remove-Item -Recurse -Force $DistDir }
New-Item -ItemType Directory -Path $DistDir -Force | Out-Null

$targetDir = if ($BuildProfile -eq "release") { "target/release" } else { "target/debug" }
$stagingDir = Join-Path $DistDir "wbwime-windows-x86_64"
if (Test-Path $stagingDir) { Remove-Item -Recurse -Force $stagingDir }
New-Item -ItemType Directory -Path $stagingDir -Force | Out-Null

# 复制二进制
$binSrc = Join-Path $ProjectRoot "$targetDir/wbwime.exe"
if (-not (Test-Path $binSrc)) {
    Write-Error "找不到二进制文件: $binSrc"
    exit 1
}
Copy-Item $binSrc $stagingDir

# 复制词典资源
$dictsSrc = Join-Path $ProjectRoot "resources/dicts"
if (Test-Path $dictsSrc) {
    Copy-Item -Recurse $dictsSrc (Join-Path $stagingDir "dicts")
}

# 复制配置
$configSrc = Join-Path $ProjectRoot "resources/config.toml"
if (Test-Path $configSrc) {
    $resourcesDir = Join-Path $stagingDir "resources"
    New-Item -ItemType Directory -Path $resourcesDir -Force | Out-Null
    Copy-Item $configSrc $resourcesDir
}

# 复制 README
$readmeSrc = Join-Path $ProjectRoot "README.md"
if (Test-Path $readmeSrc) { Copy-Item $readmeSrc $stagingDir }

# 复制 LICENSE
$licenseSrc = Join-Path $ProjectRoot "LICENSE"
if (Test-Path $licenseSrc) { Copy-Item $licenseSrc $stagingDir }

# 压缩为 zip
$version = (Get-Content (Join-Path $ProjectRoot "Cargo.toml") | Select-String 'version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
$zipName = "wbwime-windows-x86_64-v${version}.zip"
$zipPath = Join-Path $DistDir $zipName
Compress-Archive -Path $stagingDir -DestinationPath $zipPath -Force

Write-Host "`n=== 构建完成 ===" -ForegroundColor Green
Write-Host "输出: $zipPath"
$zipSize = (Get-Item $zipPath).Length
Write-Host "大小: $([math]::Round($zipSize / 1KB, 1)) KB"

# 清理暂存目录
Remove-Item -Recurse -Force $stagingDir
