#!/usr/bin/env bash
# build-linux.sh — Linux 平台 IME 模块构建脚本
# 用法: bash ./scripts/build-linux.sh
# 输出: dist/wbwime-linux-x86_64.tar.gz

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST_DIR="$PROJECT_ROOT/dist"
BUILD_PROFILE="${RELEASE:+release}"
BUILD_PROFILE="${BUILD_PROFILE:-debug}"

echo "=== wbwIME Linux 构建 ==="
echo "项目根: $PROJECT_ROOT"
echo "构建模式: $BUILD_PROFILE"

# ---------- 1. 环境检查 ----------
echo ""
echo "[1/5] 检查 Rust 环境..."
if ! command -v rustc &>/dev/null; then
    echo "错误: 未找到 rustc，请先安装 Rust: https://rustup.rs" >&2
    exit 1
fi
echo "  rustc: $(rustc --version)"
echo "  目标: $(rustc -vV | grep 'host:' | awk '{print $2}')"

# ---------- 2. 运行测试 ----------
echo ""
echo "[2/5] 运行测试..."
cargo test --workspace --exclude wbw-ime-gui

# ---------- 3. Clippy 检查 ----------
echo ""
echo "[3/5] Clippy 检查..."
cargo clippy --workspace --exclude wbw-ime-gui --all-targets -- -D warnings

# ---------- 4. 编译 ----------
echo ""
echo "[4/5] 编译项目..."
CARGO_ARGS=(build --workspace --exclude wbw-ime-gui)
if [ "$BUILD_PROFILE" = "release" ]; then
    CARGO_ARGS+=(--release)
fi
cargo "${CARGO_ARGS[@]}"

# ---------- 5. 打包 ----------
echo ""
echo "[5/5] 打包分发..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

TARGET_DIR="$PROJECT_ROOT/target/$BUILD_PROFILE"
STAGING_DIR="$DIST_DIR/wbwime-linux-x86_64"
rm -rf "$STAGING_DIR"
mkdir -p "$STAGING_DIR"

# 复制二进制
BINARY="$TARGET_DIR/wbwime"
if [ ! -f "$BINARY" ]; then
    echo "错误: 找不到二进制文件: $BINARY" >&2
    exit 1
fi
cp "$BINARY" "$STAGING_DIR/"
chmod +x "$STAGING_DIR/wbwime"

# 复制 IME 模块 (.so)
SO_FILE="$TARGET_DIR/libwbw_ime_native.so"
if [ -f "$SO_FILE" ]; then
    cp "$SO_FILE" "$STAGING_DIR/"
    echo "  已包含: libwbw_ime_native.so"
else
    echo "  警告: 未找到 .so: $SO_FILE" >&2
fi

# 复制 fbterm 输入法服务端
FBTERM_BIN="$TARGET_DIR/wbw-ime-fbterm"
if [ -f "$FBTERM_BIN" ]; then
    cp "$FBTERM_BIN" "$STAGING_DIR/"
    chmod +x "$STAGING_DIR/wbw-ime-fbterm"
    echo "  已包含: wbw-ime-fbterm"
else
    echo "  警告: 未找到 fbterm 二进制: $FBTERM_BIN" >&2
fi

# 复制 C 头文件
HEADER="$PROJECT_ROOT/crates/wbw-ime-native/include/wbw_ime_native.h"
if [ -f "$HEADER" ]; then
    mkdir -p "$STAGING_DIR/include"
    cp "$HEADER" "$STAGING_DIR/include/"
    echo "  已包含: include/wbw_ime_native.h"
fi

# 复制词典资源
if [ -d "$PROJECT_ROOT/resources/dicts" ]; then
    cp -r "$PROJECT_ROOT/resources/dicts" "$STAGING_DIR/"
fi

# 复制配置
if [ -f "$PROJECT_ROOT/resources/config.toml" ]; then
    mkdir -p "$STAGING_DIR/resources"
    cp "$PROJECT_ROOT/resources/config.toml" "$STAGING_DIR/"
fi

# 复制 README / LICENSE
[ -f "$PROJECT_ROOT/README.md" ] && cp "$PROJECT_ROOT/README.md" "$STAGING_DIR/"
[ -f "$PROJECT_ROOT/LICENSE" ] && cp "$PROJECT_ROOT/LICENSE" "$STAGING_DIR/"

# 压缩为 tar.gz
VERSION=$(grep '^version' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
TARBALL_NAME="wbwime-linux-x86_64-v${VERSION}.tar.gz"
TARBALL_PATH="$DIST_DIR/$TARBALL_NAME"

tar -czf "$TARBALL_PATH" -C "$DIST_DIR" "wbwime-linux-x86_64"

echo ""
echo "=== 构建完成 ==="
echo "输出: $TARBALL_PATH"
echo "大小: $(du -h "$TARBALL_PATH" | cut -f1)"

# 清理暂存目录
rm -rf "$STAGING_DIR"
