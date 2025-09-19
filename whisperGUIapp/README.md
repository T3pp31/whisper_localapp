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

## 実行方法

```bash
# デバッグ実行
cargo run

# リリースビルド（高速化）
cargo run --release
```

ビルド済みバイナリは `target/release/whisperGUIapp.exe` に生成されます。必要であればこのファイルと `models/`・`config.toml` を同梱して配布してください。

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
