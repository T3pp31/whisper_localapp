#!/bin/bash

# WhisperBackendAPI実行スクリプト
# 使用方法: ./run.sh [gpu|cpu] [dev|release]

set -e

MODE=${1:-gpu}      # gpu or cpu
BUILD=${2:-release} # dev or release

echo "=== WhisperBackendAPI 起動スクリプト ==="
echo "モード: $MODE"
echo "ビルド: $BUILD"
echo

# 環境変数の設定
if [ "$MODE" = "gpu" ]; then
    echo "🚀 GPU モードで起動します..."
    export WHISPER_CUBLAS=1

    # CUDAパスの設定
    if [ -d "/usr/local/cuda" ]; then
        export CUDA_PATH="/usr/local/cuda"
        export PATH="/usr/local/cuda/bin:$PATH"
        export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
        echo "CUDA_PATH: $CUDA_PATH"
    fi

    # GPU情報表示
    if command -v nvidia-smi &> /dev/null; then
        echo "GPU 情報:"
        nvidia-smi --query-gpu=name,memory.total,memory.free --format=csv,noheader,nounits
        echo
    fi
elif [ "$MODE" = "cpu" ]; then
    echo "🖥️  CPU モードで起動します..."
else
    echo "❌ 不正なモード: $MODE"
    echo "使用方法: $0 [gpu|cpu] [dev|release]"
    exit 1
fi

# 実行
if [ "$BUILD" = "dev" ]; then
    echo "🔨 開発モードで実行..."
    cargo run
else
    if [ ! -f "./target/release/WhisperBackendAPI" ]; then
        echo "❌ リリースビルドが見つかりません"
        echo "先に ./build.sh $MODE を実行してください"
        exit 1
    fi

    echo "🚀 リリースビルドを実行..."
    ./target/release/WhisperBackendAPI
fi