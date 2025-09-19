# whisperGUIapp

Whisper.cpp (whisper-rs) を用いてローカルで音声を文字起こしするための Windows 向けデスクトップアプリです。GUI フレームワークを egui から **Tauri** に換装し、HTML/CSS/JavaScript で構築したフロントエンドと Rust 製バックエンドを連携させています。

## 言語 / 技術スタック

- **言語**: Rust (バックエンド) / HTML + CSS + JavaScript (フロントエンド)
- **アプリケーションフレームワーク**: [Tauri 1.x](https://tauri.app/) （Windows 11 WebView2 を利用）
- **音声認識**: [whisper-rs](https://github.com/tazz4843/whisper-rs)（whisper.cpp の Rust バインディング）
- **音声デコード**: [symphonia](https://github.com/pdeljanov/Symphonia)
- **リサンプリング**: [rubato](https://github.com/HEnquist/rubato)
- **録音**: [cpal](https://github.com/RustAudio/cpal)（今後拡張予定）

## 入出力

- **Input**: 音声ファイル（WAV / MP3 / FLAC / M4A / OGG など）
- **Output**: テキスト（結果はアプリ上で表示し、保存・コピーが可能）

## アーキテクチャ概要

```
┌─────────────────────────────┐
│       Tauri Frontend (dist/)      │
│ - ファイル選択 (dialog API)        │
│ - 進捗表示・結果表示               │
│ - テキスト保存・コピー             │
└──────────────┬────────────────┘
               │ invoke/event
┌──────────────▼────────────────┐
│   Tauri Backend (Rust)           │
│ - Config 読み込み                │
│ - AudioProcessor で前処理        │
│ - WhisperEngine で推論           │
│ - 進捗イベントを emit            │
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
├─ dist/                # Tauri フロントエンド (静的 HTML/CSS/JS)
│  ├─ index.html
│  ├─ main.js
│  └─ styles.css
├─ src/                 # Rust バックエンド（tauri::Builder）
│  ├─ main.rs
│  ├─ audio.rs
│  ├─ config.rs
│  └─ whisper.rs
├─ models/              # ggml モデル配置先（バンドル対象）
├─ config.toml          # アプリ設定
├─ tauri.conf.json      # Tauri 設定（ビルド/バンドル）
├─ Cargo.toml
└─ download_models.sh   # モデル取得用スクリプト（Linux/WSL 用）
```

## セットアップ (Windows 11 想定)

1. **必須ツールの導入**
   - Rust (1.75 以上) ＋ `cargo`
   - Microsoft Visual C++ Build Tools（`cl.exe`）
   - [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（ほとんどの Windows 11 には同梱）
   - （推奨）`cargo install tauri-cli --version ^1.5` で `cargo tauri` を利用可能にする

2. **Whisper モデルの配置**
   - `models/` ディレクトリに ggml 形式のモデルを配置します。
   - WSL や Git Bash が使える場合は `./download_models.sh` で `tiny/base/small` などを取得できます。
   - バンドル時に `models/` と `config.toml` はインストーラに同梱されます。

3. **設定の確認**
   - `config.toml` で既定モデル (`whisper.model_path`) や出力関連の設定を調整できます。

## 開発/実行方法

```bash
# 開発モード（ホットリロードなしの簡易起動）
cargo tauri dev

# リリースビルド
cargo tauri build
```

- `cargo tauri dev` は WebView2 上でフロントエンドを読み込み、Rust バックエンドと連携します。
- `cargo tauri build` 実行後、`target/release/bundle/msi/` に Windows インストーラ (`.msi`) が生成されます。

### `.exe` 形式で配布したい場合

Tauri は既定で MSI を生成しますが、NSIS がインストールされていれば `.exe`（NSIS インストーラ）も作成できます。

1. [NSIS](https://nsis.sourceforge.io/Main_Page) を Windows にインストールし、`makensis.exe` を `PATH` に追加。
2. `tauri.conf.json` の `tauri.bundle.targets` に `"nsis"` を追加（例: `["msi", "nsis"]`）。
3. `cargo tauri build` を再度実行すると `target/release/bundle/nsis/` 配下に `.exe` が生成されます。

生成された MSI/EXE に `models/` と `config.toml` が同梱されるため、別途手動コピーせず配布可能です（モデルが大きい場合はダウンロード手順を別途案内する方が配布サイズを抑えられます）。

## アプリの使い方

1. アプリを起動し、「音声ファイルを選択」でファイルを指定します。
2. 「文字起こし開始」で推論を開始します（進捗は画面下部に表示）。
3. 完了後、結果テキストが表示されます。コピーやテキストファイル保存が可能です。

## 今後の拡張候補

- マイク録音（`AudioProcessor::start_recording` を Tauri コマンド化）
- 文字起こし結果の SRT/VTT エクスポート
- モデル選択 UI・ダウンロード UI のフロントエンド化
- 非同期キューによる同時処理の最適化

---

Tauri 化により、Windows ネイティブな配布（MSI / EXE）が容易になりました。上記手順でビルドした成果物をそのまま配布可能です。モデルを同梱するか、初回起動時にダウンロードさせるかは運用方針に合わせて調整してください。
