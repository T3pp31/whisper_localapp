# WhisperBackendAPI

Whisper (whisper.cpp / whisper-rs) を使ったローカル音声文字起こしバックエンドです。CPU で動作しますが、GPU を使った高速化にも対応しています（環境・ビルド設定が必要）。

本リポジトリでは GPU を使うためのコード修正が入っており、実行時フラグに加えて「GPU対応でビルド」することで GPU が有効になります。GPU での初期化に失敗した場合は自動で CPU にフォールバックします。

---

## 目次
- 概要
- 前提条件
- クイックスタート
- GPU ビルドと起動（NVIDIA/Mac/Intel）
- 設定（config.toml）
- 動作確認のしかた
- パフォーマンスのヒント
- トラブルシュート

---

## 概要
- ランタイム: Rust + Axum
- ASR エンジン: whisper-rs（内部で whisper.cpp を使用）
- モデル形式: ggml/gguf（例: `ggml-large-v3-turbo-q5_0.bin` など）
- GPU 対応: whisper.cpp のバックエンド（CUDA/cuBLAS、Metal、OpenVINO など）をビルド時に有効化する必要があります
- フォールバック: GPU 初期化に失敗すると CPU へ自動切替（ログ出力あり）

---

## 前提条件
- Rust/Cargo が利用できること
- CMake とビルドツールチェーンが利用できること
- GPU を使う場合は、各プラットフォームの前提を満たすこと
  - NVIDIA: CUDA Toolkit + ドライバ（cuBLAS 利用）
  - macOS: Metal が利用可能（Apple Silicon/対応GPU）
  - Intel: OpenVINO ランタイム

> 重要: GPU を使うかどうかは「実行時の設定」だけでなく「ビルド時に GPU バックエンドを有効化しているか」に依存します。

---

## クイックスタート
1) 依存の用意（必要に応じて）
- CPU のみで試す場合は特別なセットアップは不要です。
- GPU を使う場合は、後述の「GPU ビルドと起動」を参照して環境を整えてください。

2) モデルの配置
- 既定では `models/ggml-large-v3-turbo-q5_0.bin` を参照します。
- 別モデルを使う場合は `config.toml` の `whisper.model_path` を変更してください。

3) 起動（CPU）
- `cargo run` または `cargo run --release`

起動時に以下のようなログが出ます:
- `Whisperモデルを読み込みました: <path> (GPU: enabled|disabled)`
- GPU 初期化に失敗すると `GPU初期化に失敗しました。CPUで再試行します: ...` と表示され CPU で継続します。

---

## GPU ビルドと起動（NVIDIA/Mac/Intel）
GPU を使うには、whisper.cpp の GPU バックエンドを有効にしてビルドする必要があります。whisper-rs-sys は以下の環境変数でバックエンドを切り替えます。

- NVIDIA (CUDA / cuBLAS)
  - コマンド: `WHISPER_CUBLAS=1 cargo run --release`
  - 備考: CUDA Toolkit と対応ドライバが必要です。切替後は `cargo clean` すると確実です。

- macOS (Metal)
  - コマンド: `WHISPER_METAL=1 cargo run --release`

- Intel (OpenVINO)
  - コマンド: `WHISPER_OPENVINO=1 cargo run --release`
  - 備考: OpenVINO ランタイムのセットアップが必要です。

> 注: 上記は代表例です。環境変数の有効値やサポート状況は whisper-rs / whisper.cpp のバージョンに依存します。

---

## 設定（config.toml）
`config.toml`（初回起動時は自動生成）

- `whisper.model_path`: モデルファイルのパス（例: `models/ggml-large-v3-turbo-q5_0.bin`）
- `whisper.language`: 既定言語（`auto` で自動検出）
- `whisper.enable_gpu`: 実行時に GPU を使うかの希望フラグ（true/false）
  - true でも「GPU バックエンド未ビルド」の場合は CPU にフォールバックします
- `performance.whisper_threads`: Whisper のスレッド数（CPU 側の並列度）

例: `config.toml:9-17` と `config.toml:21-33` を参照

---

## 動作確認のしかた
- 起動ログで GPU の有効/無効が分かります。
  - `Whisperモデルを読み込みました: ... (GPU: enabled)` と出力されれば実行時設定としては有効化されています。
  - 併せて GPU バックエンドでビルドしていれば、内部でGPU実行が走ります。
- GPU 初期化が失敗した場合は警告を出して CPU に切り替えます。

---

## パフォーマンスのヒント
- 量子化モデル（例: `*-q5_0.bin`）でも GPU で一部オフロードされますが、FP16 モデルより速度向上が限定的なケースがあります。十分な VRAM がある場合は FP16 モデルも検討してください。
- `performance.whisper_threads` は CPU スレッド数です。GPU 利用時も前処理や一部計算で意味がありますが、CPU単体時ほど支配的ではありません。
- `--release` ビルドでの実行を推奨します。

---

## トラブルシュート
- GPU ビルドが効かない / 速度が出ない
  - ビルド時にバックエンドが有効化されているか再確認（例: `WHISPER_CUBLAS=1`）。
  - 切り替えた後は `cargo clean` → 再ビルドを推奨。
  - NVIDIA: CUDA Toolkit/ドライバの導入、`nvidia-smi` で認識確認。
  - コンテナでの実行時は `--gpus all` や nvidia-container-toolkit の設定を確認。

- モデルが見つからない
  - ログ/エラーに出る wget コマンド例で `models/` にダウンロードしてください。

---

## 実装メモ（本リポジトリでの対応）
- `src/whisper.rs` で GPU 利用フラグを `WhisperContextParameters` に反映
  - `ctx_params.use_gpu = config.whisper.enable_gpu;`（src/whisper.rs:48）
- GPU 初期化失敗時は CPU で再試行（フォールバック）
  - エラーログ出力後、`use_gpu=false` で再初期化（src/whisper.rs:50）
- 起動時ログに GPU 有効/無効を出力
  - `Whisperモデルを読み込みました: <path> (GPU: enabled|disabled)`（src/whisper.rs:78）

---

## ライセンス
- 本リポジトリのライセンスに従います。
