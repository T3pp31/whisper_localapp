#!/bin/bash

# WhisperBackendAPI ビルドスクリプト（CUDA 12.4 向け互換版）
# 使用方法: ./build_kyodo.sh [cpu|gpu]
#
# 目的:
#  - CUDA 12.4 と新しめの GCC/glibc の不整合を避けるため、
#    GPU ビルド時にホストコンパイラを g++-12/gcc-12 に強制固定します。
#  - g++-12 が見つからない場合は、明確にエラーと解決手順を案内します。

set -euo pipefail

echo "=== WhisperBackendAPI ビルドスクリプト (CUDA12.4 対応版) ==="
echo

BUILD_TYPE=${1:-gpu}   # cpu | gpu

have() { command -v "$1" >/dev/null 2>&1; }

die() {
  echo "❌ $*" 1>&2
  exit 1
}

detect_clblast() {
  if have pkg-config && pkg-config --exists clblast 2>/dev/null; then return 0; fi
  if have ldconfig && ldconfig -p 2>/dev/null | grep -qi clblast; then return 0; fi
  if [ -f "/usr/lib/x86_64-linux-gnu/libclblast.so" ] || [ -f "/usr/local/lib/libclblast.so" ]; then return 0; fi
  return 1
}

setup_bindgen_clang_args() {
  if have clang; then
    local RES_DIR
    RES_DIR=$(clang -print-resource-dir 2>/dev/null || true)
    if [ -n "${RES_DIR:-}" ] && [ -d "$RES_DIR/include" ]; then
      export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -I${RES_DIR}/include -I/usr/include -I/usr/include/x86_64-linux-gnu"
    else
      for cdir in /usr/lib/llvm-*/lib/clang/*/include; do
        if [ -d "$cdir" ]; then
          export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -I${cdir} -I/usr/include -I/usr/include/x86_64-linux-gnu"
          break
        fi
      done
    fi
    echo "✓ BINDGEN_EXTRA_CLANG_ARGS 設定: ${BINDGEN_EXTRA_CLANG_ARGS:-<none>}"
  else
    echo "⚠ clang が見つかりません。bindgen が標準ヘッダを見つけられない可能性があります（libclang-dev の導入を推奨）。"
  fi
}

# CUDA 12.4 互換のためのホストコンパイラ固定 (g++-12/gcc-12)
setup_cuda124_host_compiler() {
  local CXX_BIN="" CC_BIN=""

  # 優先順: g++-12 -> g++-13 -> g++
  # ただし CUDA12.4 では g++-12 を強制。見つからなければエラーで案内。
  if have g++-12 && have gcc-12; then
    CXX_BIN=$(command -v g++-12)
    CC_BIN=$(command -v gcc-12)
  else
    echo ""
    echo "⚠ CUDA 12.4 互換のため g++-12/gcc-12 が必要です。"
    echo "  インストール例 (Debian/Ubuntu):"
    echo "    sudo apt-get update && sudo apt-get install -y gcc-12 g++-12 libclang-dev"
    echo ""
    die "g++-12/gcc-12 が見つかりませんでした"
  fi

  export CC="$CC_BIN"
  export CXX="$CXX_BIN"
  export CUDAHOSTCXX="$CXX_BIN"
  export CMAKE_CUDA_HOST_COMPILER="$CXX_BIN"

  # nvcc にホストコンパイラを明示（CMake の CUDA 識別時にも効かせる）
  # 追加で新しめの GCC を使わざるを得ない場合の保険として allow-unsupported を含める
  # glibc の GNU 拡張で追加される cospi/sinpi 系宣言と CUDA ヘッダの食い違い回避のため
  # _GNU_SOURCE を明示的に解除し、古い GPU ターゲット警告も抑制
  export NVCC_PREPEND_FLAGS="${NVCC_PREPEND_FLAGS:-} -ccbin=${CXX_BIN} --allow-unsupported-compiler -U_GNU_SOURCE -Wno-deprecated-gpu-targets"

  echo "✓ Host compiler 固定: CC=$CC, CXX=$CXX"
  echo "✓ NVCC_PREPEND_FLAGS: ${NVCC_PREPEND_FLAGS}"
}

setup_cuda_env() {
  if have nvcc; then
    local CUDA_VERSION
    CUDA_VERSION=$(nvcc --version | grep "release" | sed 's/.*release //' | sed 's/,.*//')
    echo "✓ CUDA Toolkit が見つかりました: ${CUDA_VERSION}"
  else
    echo "⚠ CUDA Toolkit が見つかりません。GPU ビルドには CUDA が必要です。"
  fi

  if [ -d "/usr/local/cuda" ]; then
    export CUDA_PATH="/usr/local/cuda"
    export PATH="/usr/local/cuda/bin:$PATH"
    export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
    echo "✓ CUDA_PATH設定: $CUDA_PATH"
  fi

  if have nvidia-smi; then
    echo "✓ NVIDIA GPU ドライバーが見つかりました"
    nvidia-smi -L || true
  else
    echo "⚠ NVIDIA GPU ドライバーが見つかりません"
  fi
}

if [ "$BUILD_TYPE" = "gpu" ]; then
  echo "🚀 GPU対応ビルド（CUDA12.4 向け）を開始します..."
  echo "必要な環境:"
  echo "  - CUDA Toolkit 12.4 (cuBLAS含む)"
  echo "  - g++-12 / gcc-12 (ホストコンパイラ)"
  echo "  - 適切なGPUドライバー"
  echo

  setup_cuda_env
  setup_cuda124_host_compiler
  setup_bindgen_clang_args

  echo "📦 ビルド前にクリーンします（構成切替の取りこぼし防止）..."
  cargo clean || true

  echo "📦 GPU対応でビルド中..."
  export WHISPER_CUBLAS=1

  FEATURES="cuda"
  if detect_clblast; then
    export WHISPER_OPENCL=1
    FEATURES="${FEATURES},opencl"
    echo "✓ CLBlast/OpenCL を検出: OpenCL 機能を有効化します"
  else
    echo "ℹ CLBlast/OpenCL が見つからないため OpenCL 機能は無効化します（CUDA のみでビルド）"
  fi
  echo "🧩 Cargo features: ${FEATURES}"

  cargo build --release --features "${FEATURES}"

  echo
  echo "✅ GPU対応ビルドが完了しました"
  echo "実行方法: WHISPER_CUBLAS=1 ./target/release/WhisperBackendAPI"

elif [ "$BUILD_TYPE" = "cpu" ]; then
  echo "🖥️  CPU専用ビルドを開始します..."
  cargo build --release
  echo
  echo "✅ CPU専用ビルドが完了しました"
  echo "実行方法: ./target/release/WhisperBackendAPI"

else
  echo "❌ 不正なビルドタイプ: $BUILD_TYPE"
  echo "使用方法: $0 [cpu|gpu]"
  exit 1
fi

echo
echo "📋 ビルド情報:"
echo "  バイナリ: ./target/release/WhisperBackendAPI"
echo "  設定ファイル: ./config.toml"
echo "  モデルディレクトリ: ./models/"
