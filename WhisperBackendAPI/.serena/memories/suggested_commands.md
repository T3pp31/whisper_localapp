# よく使うコマンド
- 開発サーバー（GPUモード）: `./run.sh gpu dev`
- リリース実行（GPU/CPU 切替）: `./run.sh gpu release` / `./run.sh cpu release`
- ビルド（環境別）: `./build.sh gpu` / `./build.sh cpu`
- 標準テスト一式: `./test_all.sh quick`（単体）、`./test_all.sh integration`、`./test_all.sh full`
- GPU向けテスト: `./test_gpu.sh full`
- 直接コマンド: `cargo run`, `cargo test`, `cargo fmt`, `cargo clippy`
- 設定ファイル生成/確認: 実行時に `config.toml` が作成されるため、編集後に `cargo run` 等で動作確認。