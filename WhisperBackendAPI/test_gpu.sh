#!/bin/bash

# GPU機能テストスクリプト
# 使用方法: ./test_gpu.sh [quick|full|bench]

set -e

TEST_TYPE=${1:-quick}

echo "=== WhisperBackendAPI GPU テストスイート ==="
echo "テストタイプ: $TEST_TYPE"
echo

# 環境変数の設定
export WHISPER_CUBLAS=1
if [ -d "/usr/local/cuda" ]; then
    export CUDA_PATH="/usr/local/cuda"
    export PATH="/usr/local/cuda/bin:$PATH"
    export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"
fi

case $TEST_TYPE in
    "quick")
        echo "🚀 クイックテスト実行..."
        cargo test test_gpu_initialization -- --nocapture
        cargo test test_environment_variables -- --nocapture
        cargo test test_gpu_library_detection -- --nocapture
        ;;

    "full")
        echo "🔍 フルテスト実行（性能比較含む）..."
        cargo test gpu_tests -- --nocapture --include-ignored
        ;;

    "bench")
        echo "⚡ ベンチマークテスト実行..."
        cargo test bench_gpu_transcription -- --nocapture --include-ignored
        ;;

    "api")
        echo "🌐 APIエンドポイントテスト..."
        echo "サーバーが起動していることを確認してください"
        read -p "サーバーのURL (例: http://localhost:8080): " SERVER_URL

        if [ -z "$SERVER_URL" ]; then
            SERVER_URL="http://localhost:8080"
        fi

        echo "GPU状態確認中..."
        curl -s "$SERVER_URL/gpu-status" | python3 -m json.tool || echo "JSONツールが見つかりません"

        echo
        echo "ヘルスチェック..."
        curl -s "$SERVER_URL/health" | python3 -m json.tool || echo "JSONツールが見つかりません"
        ;;

    *)
        echo "❌ 不正なテストタイプ: $TEST_TYPE"
        echo "使用方法: $0 [quick|full|bench|api]"
        echo
        echo "  quick - 基本的なGPU初期化・環境確認テスト"
        echo "  full  - 性能比較を含む全テスト（モデルファイルが必要）"
        echo "  bench - ベンチマークテスト（モデルファイルが必要）"
        echo "  api   - APIエンドポイントのテスト（サーバー起動が必要）"
        exit 1
        ;;
esac

echo
echo "✅ テスト完了"