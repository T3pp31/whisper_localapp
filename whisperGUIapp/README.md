# whisperGUIapp

Whisper.cpp (whisper-rs) を用いてローカルで音声を文字起こしするための Windows 向けデスクトップアプリです。GUI には [egui](https://github.com/emilk/egui) / [eframe](https://crates.io/crates/eframe) を採用し、Rust のみで完結するネイティブ UI を構築しています。

## 言語 / 技術スタック

- **言語**: Rust
- **GUI フレームワーク**: eframe + egui
- **音声認識**: [whisper-rs](https://github.com/tazz4843/whisper-rs)（whisper.cpp の Rust バインディング）
- **音声デコード**: [symphonia](https://github.com/pdeljanov/Symphonia)
- **リサンプリング**: [rubato](https://github.com/HEnquist/rubato)
- **録音（準備中）**: [cpal](https://github.com/RustAudio/cpal)

## 入出力

- **Input**: 音声ファイル（WAV / MP3 / FLAC / M4A / OGG など）
  
モデルの配置/ダウンロード
- 既定のモデル保存先（ユーザー毎）
  - Windows: `%LOCALAPPDATA%/whisperGUIapp/models`
  - macOS: `~/Library/Application Support/whisperGUIapp/models`
  - Linux: `~/.local/share/whisperGUIapp/models`
- アプリからのダウンロード
  - 「モデルをダウンロード」ボタンで選択中のモデルを取得
  - 「未DLをまとめてDL」ボタンで未ダウンロードのモデルを一括取得
  - ダウンロード後、自動的に一覧が更新されます
- **Output**: テキスト（アプリ内で表示し、コピーや保存が可能）

## アーキテクチャ概要

```
┌─────────────────────────────┐
│     egui/eframe Frontend (Rust)   │
│ - ファイル選択（ネイティブダイアログ）   │
│ - モデル選択・ダウンロード表示           │
│ - 進捗 / 結果表示・保存                  │
└──────────────┬────────────────┘
               │ チャネルによるメッセージ
┌──────────────▼────────────────┐
│   アプリロジック (Rust)             │
│ - Config 読み込み/保存              │
│ - AudioProcessor で前処理            │
│ - WhisperEngine で推論               │
└──────────────┬────────────────┘
               │ PCM 16kHz
┌──────────────▼────────────────┐
│     whisper-rs / whisper.cpp      │
│ - ggml モデルをロード             │
│ - 文字起こし                      │
└──────────────────────────────────┘
```

## プロジェクト構成

```
whisperGUIapp/
├─ src/                 # Rust コード（egui アプリ）
│  ├─ main.rs
│  ├─ audio.rs
│  ├─ config.rs
│  ├─ models.rs
│  └─ whisper.rs
├─ models/              # ggml モデル配置先
├─ config.toml          # アプリ設定
├─ Cargo.toml
└─ download_models.sh   # モデル取得用スクリプト（Linux/WSL 用）
```

## セットアップ (Windows 11 想定)

1. **必須ツールの導入**
   - Rust (1.75 以上) ＋ `cargo`
   - Microsoft Visual C++ Build Tools（`cl.exe`）

2. **Whisper モデルの配置**
   - `models/` ディレクトリに ggml 形式のモデルを配置します。
   - WSL や Git Bash が使える場合は `./download_models.sh` で `tiny/base/small` などを取得できます。

3. **設定の確認**
   - `config.toml` で既定モデル (`whisper.model_path`) や出力ディレクトリなどを調整できます。

## 実行方法 / 配布

```bash
# デバッグ実行
cargo run

# リリースビルド（高速化）
cargo run --release
```

ビルド済みバイナリは `target/release/whisperGUIapp.exe` に生成されます。必要であればこのファイルと `models/`・`config.toml` を同梱して配布してください。

### インストーラーにモデルを同梱して配布する

1. `models/` ディレクトリに配布したいモデル（例: `ggml-large-v3-turbo-q5_0.bin`）を配置します。
   - 本リポジトリには `models/.gitkeep` を置いています。実際に同梱したい `.bin` を追加してください。
2. インストーラーをビルドします。

   ```bash
   cargo tauri build
   ```

   生成物は `target/release/bundle/` 配下に出力されます（MSI/NSISなど）。

3. 初回起動時、インストーラーに同梱したモデルはユーザー領域へ自動コピーされます。
   - コピー先: Windows は `%LOCALAPPDATA%/whisperGUIapp/models`（他OSは各OSのローカルデータディレクトリ）
   - アプリは以後このディレクトリを参照します。

注意:
- `cargo build --release` だけではインストーラーは作られません（EXEのみ）。インストーラー配布は `cargo tauri build` を使ってください。
- `tauri.conf.json` の `bundle.resources` に `models/**` を指定しています。`models/` が空だとビルドが失敗するため、少なくとも1つの「非隠し」ファイル（例: `models/README.txt`）を置いてください（`.gitkeep` のような隠しファイルは対象外）。

### 設定ファイルの保存先（配布時）

- 開発中はカレント直下の `config.toml` を使う運用でも構いませんが、配布版（リリースビルド）ではユーザー設定ディレクトリに保存・読み込みします。
  - Windows: `%APPDATA%/whisperGUIapp/config.toml`
  - macOS/Linux: `~/.config/whisperGUIapp/config.toml`
  - 実装: `src/config.rs` の `Config::config_file_path()` を参照

## パフォーマンス最適化

高速化のための現実的な手順を重要度順にまとめます。CPUマルチコアは本アプリで既に利用可能で、設定で伸ばせます。

- リリースビルドで実行する
  - デバッグビルドは大幅に遅くなります。`cargo run --release` を使ってください。
  - 本プロジェクトは `Cargo.toml` の `[profile.release]` に `opt-level = 3`, `lto = true`, `codegen-units = 1` を設定済みです。

- スレッド数（CPUコア）の調整
  - 文字起こし時の並列度は `config.toml` の `performance.whisper_threads` で指定します。
  - 目安は「物理コア数（±1）」です。例: 8コアCPUなら `whisper_threads = 8`。
  - 設定例（`whisperGUIapp/config.toml`）:

    ```toml
    [performance]
    audio_threads = 2
    whisper_threads = 8   # ← マシンに合わせて増やす
    use_gpu = false
    ```

  - 実装箇所: whisper-rs に渡すスレッド数は `params.set_n_threads(...)` で設定しています（`whisperGUIapp/src/whisper.rs:21`）。

- CPU命令セットの最適化（AVX2 など）
  - 実行マシンに合わせて最適化するには、プロジェクトルートに `.cargo/config.toml` を作成し、以下を追加します。

    ```toml
    [build]
    rustflags = ["-C", "target-cpu=native"]
    ```

  - これにより利用可能なSIMD命令（AVX2/AVX-512 など）が有効化され、CPU環境での性能が向上する可能性があります。

- モデル選択（速度と精度のトレードオフ）
  - `tiny`/`base`/`small` は `medium`/`large` 系より速いです（精度は下がります）。
  - 量子化モデル（例: `ggml-*-q5_0.bin`, `ggml-large-v3-turbo-q8_0.bin`）はメモリ削減・速度向上が見込めます（精度はやや低下）。
  - モデルの切替は GUI または `config.toml` の `whisper.model_path` を変更してください。

- 入力の前処理を減らす
  - すでに 16kHz モノラルの WAV なら、デコードやリサンプリング負荷が抑えられます。
  - 異なるサンプリングレート/フォーマットはアプリ内で統一します（`whisperGUIapp/src/audio.rs`）。

- 速度優先の推論オプション（任意）
  - whisper-rs のバージョンによっては「高速化モード（例: `speed_up`）」が利用できる場合があります。利用可能な場合は、`whisperGUIapp/src/whisper.rs` のパラメータ設定に該当の API 呼び出しを追加します（品質が少し低下することがあります）。
  - 例（対応している場合のみ）:

    ```rust
    // whisperGUIapp/src/whisper.rs の make_params 内など
    // params.set_speed_up(true);
    ```

### GPU / BLAS 加速について（上級者向け）

- 本アプリは `whisper-rs`（whisper.cpp バインディング）を利用しています。標準構成では CPU 実行で、GPU へのオフロードはバージョン/ビルド構成に依存します。
- GPU を使いたい場合の選択肢:
  - whisper.cpp 本体を CUDA/Metal 等のバックエンドでビルドし、アプリから外部バイナリとして呼び出す（実装変更が必要）。
  - GPU/BLAS を露出する別のラッパー/ブランチへ切替（依存関係の見直しが必要）。
- CPU でも BLAS（OpenBLAS/MKL/Accelerate）を用いると 1.2〜2倍程度向上するケースがありますが、`whisper-rs-sys` のビルドフラグを適切に渡す必要があります。導入は環境依存のため、この README では手順を一般論に留めています。

まずは「リリースビルド」「スレッド数調整」「target-cpu=native」の3点を適用し、必要に応じてモデルを軽量化するのがおすすめです。

## アプリの使い方

1. アプリを起動し、「音声ファイルを選択」でファイルを指定します。
2. 「文字起こし開始」で推論を開始します（ステータス欄に進捗が表示されます）。
3. 完了後、結果テキストが表示されます。コピーおよびテキストファイル保存が可能です。

## 今後の拡張候補

- マイク録音（`AudioProcessor::start_recording` の UI 連携）
- 文字起こし結果の SRT/VTT エクスポート
- 設定 UI の整備（テーマ切り替えなど）
- マルチスレッド最適化やキューによるバッチ処理

---

egui 化により、WebView 依存なしでシングルバイナリの配布が容易になりました。リリースビルド後に生成される EXE をそのまま配布することで、ユーザーは追加のランタイムを入れずに利用できます。
