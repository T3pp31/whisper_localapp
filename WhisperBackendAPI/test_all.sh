#!/bin/bash

# 全テスト実行スクリプト
# 使用方法: ./test_all.sh [quick|full|unit|integration|coverage]

set -e

TEST_TYPE=${1:-quick}

echo "=== WhisperBackendAPI テストスイート ==="
echo "テストタイプ: $TEST_TYPE"
echo

# GPU関連の環境変数を設定（テスト用）
export WHISPER_CUBLAS=0  # テスト環境ではGPU無効
export RUST_BACKTRACE=1

case $TEST_TYPE in
    "quick")
        echo "🚀 クイックテスト実行..."
        echo "単体テストのみを実行します（統合テスト・GPU・重いテストは除外）"

        # 基本的な単体テストのみ実行
        cargo test --lib -- --nocapture \
            --skip integration_ \
            --skip gpu_ \
            --skip performance_ \
            --skip bench_
        ;;

    "unit")
        echo "📋 単体テスト実行..."
        echo "全ての単体テストを実行します（統合テストは除外）"

        # 全ての単体テスト（統合テスト除外）
        cargo test --lib -- --nocapture \
            --skip integration_
        ;;

    "integration")
        echo "🔗 統合テスト実行..."
        echo "統合テストのみを実行します"

        # 統合テストのみ実行
        cargo test --test integration_test -- --nocapture
        ;;

    "full")
        echo "🔍 フルテスト実行..."
        echo "全てのテスト（重いテストも含む）を実行します"

        # 全てのテスト（ignoreされたテストも含む）
        cargo test -- --nocapture --include-ignored
        ;;

    "coverage")
        echo "📊 カバレッジ付きテスト実行..."

        # cargoのテストカバレッジツールが必要
        if ! command -v cargo-tarpaulin &> /dev/null; then
            echo "cargo-tarpaulinがインストールされていません。インストール中..."
            cargo install cargo-tarpaulin
        fi

        # カバレッジ計測
        cargo tarpaulin --out html --output-dir coverage/ -- --nocapture \
            --skip integration_ \
            --skip gpu_

        echo "カバレッジレポートがcoverage/tarpaulin-report.htmlに生成されました"
        ;;

    "modules")
        echo "📦 モジュール別テスト実行..."
        echo "各モジュールのテストを個別に実行します"

        modules=("config_test" "audio_test" "models_test" "whisper_test" "handlers_test")

        for module in "${modules[@]}"; do
            echo ""
            echo "--- $module テスト ---"
            cargo test --test "$module" -- --nocapture || {
                echo "❌ $module テストが失敗しました"
                continue
            }
            echo "✅ $module テスト完了"
        done

        echo ""
        echo "--- 統合テスト ---"
        cargo test --test integration_test -- --nocapture || {
            echo "❌ 統合テストが失敗しました"
        }
        echo "✅ 統合テスト完了"
        ;;

    "doc")
        echo "📚 ドキュメントテスト実行..."
        echo "ドキュメント内のテストコードを実行します"

        # ドキュメントテスト
        cargo test --doc -- --nocapture
        ;;

    "bench")
        echo "⚡ ベンチマークテスト実行..."

        # ベンチマーク（nightly必要）
        if rustc --version | grep -q "nightly"; then
            cargo bench
        else
            echo "ベンチマークにはRust nightlyが必要です"
            echo "代替として重いテストを実行します"
            cargo test bench_ -- --nocapture --include-ignored
        fi
        ;;

    "ci")
        echo "🔧 CI/CDテスト実行..."
        echo "継続的インテグレーション用のテストセットを実行します"

        # フォーマットチェック
        echo "コードフォーマットチェック..."
        cargo fmt -- --check || {
            echo "❌ コードフォーマットエラー。'cargo fmt' を実行してください"
            exit 1
        }

        # Clippy（リンター）
        echo "Clippyチェック..."
        cargo clippy -- -D warnings || {
            echo "❌ Clippyエラー。警告を修正してください"
            exit 1
        }

        # 基本テスト
        echo "基本テスト実行..."
        cargo test --lib -- --nocapture --skip integration_ --skip gpu_

        # ビルドテスト
        echo "リリースビルドテスト..."
        cargo build --release
        ;;

    "debug")
        echo "🐛 デバッグテスト実行..."
        echo "詳細なデバッグ情報付きでテストを実行します"

        export RUST_LOG=debug
        export RUST_BACKTRACE=full

        cargo test --lib -- --nocapture --test-threads=1
        ;;

    *)
        echo "❌ 不正なテストタイプ: $TEST_TYPE"
        echo
        echo "使用方法: $0 [test_type]"
        echo
        echo "利用可能なテストタイプ:"
        echo "  quick       - クイックテスト（基本的な単体テストのみ）"
        echo "  unit        - 全単体テスト"
        echo "  integration - 統合テストのみ"
        echo "  full        - 全テスト（重いテストも含む）"
        echo "  coverage    - カバレッジ付きテスト"
        echo "  modules     - モジュール別テスト"
        echo "  doc         - ドキュメントテスト"
        echo "  bench       - ベンチマークテスト"
        echo "  ci          - CI/CDテスト（フォーマット、Clippy含む）"
        echo "  debug       - デバッグテスト（詳細ログ付き）"
        exit 1
        ;;
esac

echo
echo "✅ テスト完了: $TEST_TYPE"

# テスト結果のサマリー表示
if [ "$TEST_TYPE" != "coverage" ] && [ "$TEST_TYPE" != "bench" ]; then
    echo
    echo "📊 テスト実行サマリー:"
    echo "実行されたテスト数を確認するには、上記の出力を確認してください"
    echo
    echo "🔧 次のステップ:"
    case $TEST_TYPE in
        "quick")
            echo "  - より詳細なテストを実行: ./test_all.sh full"
            echo "  - 統合テストを実行: ./test_all.sh integration"
            ;;
        "unit")
            echo "  - 統合テストを実行: ./test_all.sh integration"
            echo "  - カバレッジを測定: ./test_all.sh coverage"
            ;;
        "integration")
            echo "  - 全テストを実行: ./test_all.sh full"
            echo "  - カバレッジを測定: ./test_all.sh coverage"
            ;;
        "full")
            echo "  - カバレッジを測定: ./test_all.sh coverage"
            echo "  - GPU環境でテスト: ./test_gpu.sh full"
            ;;
    esac
fi