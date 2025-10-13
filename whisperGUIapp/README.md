# whisperGUIapp

Whisper.cpp（whisper-rs）を用いてローカルで音声を文字起こしする Windows 向けデスクトップアプリです。GUI は Tauri 1.x を採用し、静的な HTML/CSS/JavaScript（`dist/`）のフロントエンドと Rust バックエンドを `invoke/event` で連携させています。

## 言語 / 技術スタック

- 言語: Rust（バックエンド）/ HTML + CSS + JavaScript（フロントエンド）
- フレームワーク: Tauri 1.x（Windows 11 WebView2）
- 音声認識: whisper-rs（whisper.cpp の Rust バインディング）
- デコード: symphonia
- リサンプリング: rubato
- 録音: cpal（将来的に対応予定）

## 主な機能

- 入力: 音声ファイル（WAV / MP3 / FLAC / M4A / OGG / MP4 など）
- プレビュー再生: 16kHz モノラル WAV を一時生成して安定再生（失敗時はフォールバック）
- モデル管理: カタログから選択・切替、1件DL / 未DL一括DL、進捗表示
- 言語設定: 自動検出/ja/en/zh/ko、英語翻訳トグル
- GPU加速: ローカルPCのGPU（CUDA/Metal/Vulkan/hipBLAS）を使用した高速推論に対応
- 結果表示: タイムスタンプ付き、クリック再生、編集モード、クリップボードへコピー

## アーキテクチャ概要

```
┌─────────────────────────────┐
│    Tauri Frontend (dist/)        │
│ - ファイル/モデル選択・DL          │
│ - 言語/翻訳設定・結果表示          │
│ - クリック再生・コピー              │
└──────────────┬────────────────┘
               │ invoke/event
┌──────────────▼────────────────┐
│  Tauri Backend (Rust)            │
│ - Config 読み書き                 │
│ - AudioProcessor 前処理           │
│ - WhisperEngine 推論              │
│ - DL進捗を emit                   │
└──────────────┬────────────────┘
               │ PCM 16kHz
┌──────────────▼────────────────┐
│ whisper-rs / whisper.cpp        │
│ - モデルロード / 文字起こし       │
└────────────────────────────────┘
```

## プロジェクト構成

```
whisperGUIapp/
├─ dist/                 # フロントエンド（静的アセット）
│  ├─ index.html
│  ├─ main.js
│  └─ styles.css
├─ src/                  # Rust バックエンド
│  ├─ main.rs            # tauri::Builder / コマンド
│  ├─ audio.rs           # デコード/モノラル化/16kHz 変換
│  ├─ config.rs          # 設定（ユーザー領域へ保存）
│  ├─ models.rs          # モデルカタログ/情報
│  └─ whisper.rs         # whisper-rs ラッパ
├─ models/               # モデル配置（ビルド同梱の元）
├─ icons/                # アイコン
├─ config.toml           # 初期設定の例（実保存先はユーザー領域）
├─ tauri.conf.json       # Tauri 設定（bundle/allowlist）
├─ Cargo.toml
└─ download_models.sh    # モデル取得スクリプト（WSL/Git Bash）
```

## データ/設定の保存先（初回起動時に移行）

- 設定: `%APPDATA%/whisperGUIapp/config.toml`
- モデル: `%LOCALAPPDATA%/whisperGUIapp/models`
- macOS/Linux は各 OS の一般的な設定/ローカルデータディレクトリ配下

`models/` に同梱したモデルは、初回起動時にユーザー領域へ自動コピー（未存在時）されます。

## セットアップ（Windows 11 想定）

1) 必須ツール
- Rust（1.75 以上）/ `cargo`
- Microsoft Visual C++ Build Tools（`cl.exe`）
- WebView2 Runtime（多くの Windows 11 に同梱）
- 推奨: `cargo install tauri-cli --version ^1.5`

2) モデルの用意
- `models/` に ggml 形式の `.bin` を配置、またはアプリの DL 機能を使用
- `./download_models.sh` で `tiny/base/small/medium/large-v3-turbo(-q5_0)` を一括取得可能

3) 設定
- `config.toml` の `whisper.model_path`・`performance.whisper_threads` などを確認
  - 実際の保存はユーザー領域に行われます（初回に自動作成/移行）

## 実行/ビルド/配布

### CPU版（デフォルト）

```bash
cd whisperGUIapp

# 開発モード（dist をそのまま読み込み）
cargo tauri dev

# リリースビルド（インストーラ生成）
cargo tauri build
```

### GPU対応版

GPU加速を有効化してビルドするには、プラットフォームに応じた機能フラグを指定します：

```bash
# CUDA (NVIDIA GPU)
cargo tauri build --features gpu-cuda

# Metal (Apple Silicon / macOS)
cargo tauri build --features gpu-metal

# Vulkan (クロスプラットフォーム)
cargo tauri build --features gpu-vulkan

# hipBLAS (AMD GPU / Linux)
cargo tauri build --features gpu-hipblas
```

**前提条件:**
- CUDA: CUDA Toolkit（nvcc、cuBLAS）がインストールされ、環境変数が設定されている
- Metal: Xcode Command Line Toolsがインストールされている
- Vulkan: Vulkan SDKがインストールされている
- hipBLAS: ROCmツールチェーンとhipblas開発パッケージ（Linux）

GPU版でビルドした後、アプリの「GPUを利用」トグルをONにすると、ローカルPCのGPUで高速推論が可能です。

### その他

- 既定のターゲットは `nsis`（`.exe` インストーラ）。MSI が必要なら `tauri.conf.json` の `tauri.bundle.targets` に `"msi"` を追加。
- 生成物の例: `target/release/bundle/nsis/`（NSIS）, `.../msi/`（MSI）
- モデル同梱は `tauri.bundle.resources` に `.bin` を列挙してください（例: `models/ggml-large-v3-turbo-q5_0.bin`）。

## 使い方

- 「音声ファイルを選択」→「読み込み」でプレビューを準備・再生が可能
- 「音声の言語」から `自動/ja/en/zh/ko` を選択し、「英語に翻訳」トグルで英訳しながら書き起こし
- 「カタログから選択」→「モデル切替」でモデル変更。未取得は「モデルをダウンロード」または「未DLをまとめてDL」
- **「GPUを利用」トグル**をONにすると、ローカルPCのGPUで高速推論を実行（GPU対応版ビルドが必要）
- 「文字起こし開始」で推論実行。結果はタイムスタンプ付きで表示され、「クリック再生」ON で該当行から再生
- 「解析結果をコピー」で結果全文をクリップボードへ
- 範囲スライダ UI は先行実装（現時点では全体を解析）

## パフォーマンスのヒント

- リリースビルドを使用（`cargo tauri build`）
- **GPU版でビルド**して「GPUを利用」をONにすると、CPU版の数倍〜10倍以上高速化
- CPU版の場合、`performance.whisper_threads` を CPU コア数に合わせて調整（UIの「使用CPU数」で変更可能）
- `-C target-cpu=native` を付与すると SIMD が有効化され高速化する場合あり
- 軽量/量子化モデル（`tiny/base/small` や `*-q5_0`）は高速（精度は低下）

## 今後の拡張

- マイク録音の UI 連携
- SRT/VTT へのエクスポート
- 範囲指定の部分書き起こし
- バッチ/キュー処理の最適化

Tauri 採用により、Windows ネイティブ配布（EXE/MSI）が容易です。モデル同梱と初回ダウンロードは配布ポリシーに応じて選択してください。

