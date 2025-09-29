#!/bin/bash

# WhisperBackendAPI ビルドスクリプト（GPU/CUDA 対応強化版）
# 使用方法: ./build.sh [cpu|gpu]

set -e

BUILD_CONFIG_FILE="build_config.toml"

load_build_config() {
  # build_config.toml から NVCC 向けフラグを読み取り、環境変数に反映
  if [ ! -f "$BUILD_CONFIG_FILE" ]; then
    return
  fi

  local configured_flags
  configured_flags=$(sed -n 's/^[[:space:]]*nvcc_prepend_flags[[:space:]]*=[[:space:]]*"\(.*\)"[[:space:]]*$/\1/p' "$BUILD_CONFIG_FILE" | head -n1)

  if [ -z "$configured_flags" ]; then
    return
  fi

  if [ -n "${NVCC_PREPEND_FLAGS:-}" ]; then
    NVCC_PREPEND_FLAGS="${NVCC_PREPEND_FLAGS} ${configured_flags}"
  else
    NVCC_PREPEND_FLAGS="$configured_flags"
  fi

  export NVCC_PREPEND_FLAGS
  echo "✓ build_config.toml から NVCC_PREPEND_FLAGS を適用: ${NVCC_PREPEND_FLAGS}"
}

echo "=== WhisperBackendAPI ビルドスクリプト ==="
echo

# デフォルトはGPUビルド
BUILD_TYPE=${1:-gpu}

# 小さなユーティリティ関数
have() { command -v "$1" >/dev/null 2>&1; }

detect_clblast() {
  # CLBlast の存在確認（OpenCL 機能の可否判定）
  if have pkg-config && pkg-config --exists clblast 2>/dev/null; then return 0; fi
  if have ldconfig && ldconfig -p 2>/dev/null | grep -qi clblast; then return 0; fi
  if [ -f "/usr/lib/x86_64-linux-gnu/libclblast.so" ] || [ -f "/usr/local/lib/libclblast.so" ]; then return 0; fi
  return 1
}

setup_bindgen_clang_args() {
  # bindgen が標準ヘッダを見つけられるようにパスを補う
  if have clang; then
    local RES_DIR
    RES_DIR=$(clang -print-resource-dir 2>/dev/null || true)
    if [ -n "${RES_DIR:-}" ] && [ -d "$RES_DIR/include" ]; then
      export BINDGEN_EXTRA_CLANG_ARGS="${BINDGEN_EXTRA_CLANG_ARGS:-} -I${RES_DIR}/include -I/usr/include -I/usr/include/x86_64-linux-gnu"
    else
      # LLVMの標準パスを総当たり
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

setup_host_compiler() {
  # NVCC のホストC++コンパイラに g++-13/12 を優先して設定
  local CXX_BIN=""
  for x in g++-13 g++-12 g++; do
    if have "$x"; then CXX_BIN=$(command -v "$x"); break; fi
  done
  if [ -n "$CXX_BIN" ]; then
    export CXX="$CXX_BIN"
    # 対応する gcc を推定
    local CC_BIN=""
    if [[ "$CXX_BIN" =~ g\+\+-13$ ]] && have gcc-13; then CC_BIN=$(command -v gcc-13); fi
    if [[ -z "$CC_BIN" && "$CXX_BIN" =~ g\+\+-12$ ]] && have gcc-12; then CC_BIN=$(command -v gcc-12); fi
    if [ -z "$CC_BIN" ]; then CC_BIN=$(command -v gcc 2>/dev/null || echo "/usr/bin/cc"); fi
    export CC="$CC_BIN"

    export CMAKE_CUDA_HOST_COMPILER="$CXX"
    export CUDAHOSTCXX="$CXX"

    # g++ 14+ の場合は非推奨のフラグで回避（代替が無い場合のみ）
    local CXX_MAJOR
    CXX_MAJOR=$("$CXX" -dumpfullversion -dumpversion 2>/dev/null | cut -d. -f1 || true)
    if [ -n "$CXX_MAJOR" ] && [ "$CXX_MAJOR" -ge 14 ]; then
      if ! have g++-13 && ! have g++-12; then
        export NVCC_PREPEND_FLAGS="--allow-unsupported-compiler"
        echo "⚠ g++ $CXX_MAJOR 検出。互換性のため NVCC_PREPEND_FLAGS=--allow-unsupported-compiler を設定しました"
      fi
    fi
    echo "✓ Host compiler: CC=$CC, CXX=$CXX"
  fi
}

if [ "$BUILD_TYPE" = "gpu" ]; then
  echo "🚀 GPU対応ビルドを開始します..."
  echo "必要な環境:"
  echo "  - CUDA Toolkit (cuBLAS含む)"
  echo "  - 適切なGPUドライバー"
  echo

  # CUDA環境の確認
  if have nvcc; then
    CUDA_VERSION=$(nvcc --version | grep "release" | sed 's/.*release //' | sed 's/,.*//')
    echo "✓ CUDA Toolkit が見つかりました: $CUDA_VERSION"
  else
    echo "⚠ CUDA Toolkit が見つかりません"
    echo "  CUDAツールキットをインストールしてください: https://developer.nvidia.com/cuda-toolkit"
    echo
  fi

  # GPUドライバーの確認
  if have nvidia-smi; then
    echo "✓ NVIDIA GPU ドライバーが見つかりました"
    nvidia-smi -L || true
  else
    echo "⚠ NVIDIA GPU ドライバーが見つかりません"
    echo
  fi

  # CUDAパスの設定（標準的な場所を確認）
  if [ -d "/usr/local/cuda" ]; then
    export CUDA_PATH="/usr/local/cuda"
    export PATH="/usr/local/cuda/bin:$PATH"
    export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
    echo "✓ CUDA_PATH設定: $CUDA_PATH"
  fi

  # 追加セットアップ
  setup_host_compiler
  load_build_config
  setup_bindgen_clang_args

  echo "📦 ビルド前にクリーンします（構成切替の取りこぼし防止）..."
  cargo clean

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
  echo "🖥️ CPU専用ビルドを開始します..."

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
echo
echo "🔧 最初の実行前に以下を確認してください:"
echo "  1. モデルファイルのダウンロード"
echo "  2. config.tomlの設定確認"
echo "  3. GPU使用の場合: /gpu-status エンドポイントでGPU状態確認"
