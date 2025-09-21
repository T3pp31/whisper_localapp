# whisperGUIapp

Whisper.cpp（whisper-rs）を用いてローカルで音声を文字起こしする Windows 向けデスクトップアプリです。GUI は **Tauri 1.x** を採用し、静的な HTML/CSS/JavaScript（`dist/`）で構築したフロントエンドと、Rust 製バックエンドが `invoke/event` で連携します。

## 言語 / 技術スタック

- 言語: Rust（バックエンド）/ HTML + CSS + JavaScript（フロントエンド）
- フレームワーク: [Tauri 1.x](https://tauri.app/)（Windows 11 WebView2）
- 音声認識: [whisper-rs](https://github.com/tazz4843/whisper-rs)（whisper.cpp の Rust バインディング）
- デコード: [symphonia](https://github.com/pdeljanov/Symphonia)
- リサンプリング: [rubato](https://github.com/HEnquist/rubato)
- 録音: [cpal](https://github.com/RustAudio/cpal)（今後対応予定）

## 主な機能

- 音声ファイル入力: WAV / MP3 / FLAC / M4A / OGG / MP4 等
- プレビュー再生: 16kHz モノラルの WAV を一時生成して再生（再生失敗時はフォールバック）
- 言語設定: 自動検出/日本語/英語/中国語/韓国語を切替（UI から反映）
- 英語翻訳: 英語に翻訳して書き起こすトグルあり
- モデル管理: カタログから選択・切替、1件 or 未ダウンロード一括のダウンロード（進捗表示）
- 解析結果: タイムスタンプ付き表示、クリック再生、クリップボードへコピー

## アーキテクチャ概要

```
┌─────────────────────────────┐
│    Tauri Frontend (whisperGUIapp/dist) │
│ - ファイル/モデル選択・DL・言語設定         │
│ - 再生・クリック再生・結果表示/コピー      │
└──────────────┬────────────────┘
               │ invoke/event
┌──────────────▼────────────────┐
│      Tauri Backend (Rust)        │
│ - Config 読み込み/保存            │
│ - AudioProcessor で前処理         │
│ - WhisperEngine で推論            │
│ - DL進捗を emit（download-progress）│
└──────────────┬────────────────┘
               │ PCM 16kHz
┌──────────────▼────────────────┐
│   whisper-rs / whisper.cpp       │
│ - ggml モデルをロード             │
│ - 文字起こし                      │
└──────────────────────────────────┘
```

## プロジェクト構成

```
whisperGUIapp/
├─ dist/                 # Tauri フロントエンド（静的アセット）
│  ├─ index.html
│  ├─ main.js
│  └─ styles.css
├─ src/                  # Rust バックエンド
│  ├─ main.rs            # tauri::Builder / コマンド定義
│  ├─ audio.rs           # 読み込み・デコード・16kHz化
│  ├─ config.rs          # 設定の読み書き（ユーザー領域）
│  ├─ models.rs          # モデルカタログ/情報
│  └─ whisper.rs         # whisper-rs 呼び出し
├─ models/               # モデル配置（開発用・同梱用）
├─ icons/                # アプリアイコン
├─ config.toml           # 既定設定（初回の参考）
├─ tauri.conf.json       # Tauri 設定（バンドル/許可API）
├─ Cargo.toml
└─ download_models.sh    # モデル取得スクリプト（WSL/Git Bash 用）
```

## データ/設定の保存先（初回起動時に移行）

- 設定ファイル: Windows は `%APPDATA%/whisperGUIapp/config.toml`
- モデル保存先: Windows は `%LOCALAPPDATA%/whisperGUIapp/models`
- macOS/Linux は各 OS の一般的な設定/ローカルデータディレクトリ配下に作成されます。

`models/` に同梱したモデルは、初回起動時にユーザー領域へ自動コピーされます（存在しない場合のみ）。

## セットアップ（Windows 11 想定）

1) 前提ツール
- Rust（1.75+）/ `cargo`
- Microsoft Visual C++ Build Tools（`cl.exe`）
- WebView2 Runtime（多くの Windows 11 に同梱）
- 推奨: `cargo install tauri-cli --version ^1.5`

2) モデルの用意
- `whisperGUIapp/models/` に ggml 形式の `.bin` を配置、またはアプリの「モデルをダウンロード/未DLをまとめてDL」を利用
- WSL/Git Bash が使える場合は `whisperGUIapp/download_models.sh` で `tiny/base/small/medium/large-v3-turbo(-q5_0)` を取得可能

3) 設定の確認
- `whisperGUIapp/config.toml` で既定モデルやスレッド数を調整（実際の保存はユーザー領域に行われます）

## 実行/ビルド

```bash
cd whisperGUIapp

# 開発モード（静的 dist を読み込む）
cargo tauri dev

# リリースビルド（インストーラ含む）
cargo tauri build
```

- 既定では `tauri.conf.json` の `bundle.targets` が `nsis` のため、Windows 用 `.exe`（NSIS インストーラ）を生成します。
- MSI が必要な場合は、`tauri.conf.json` の `tauri.bundle.targets` に `"msi"` を追加してください。
- 出力先: `whisperGUIapp/target/release/bundle/nsis/`（NSIS）/ `.../msi/`（MSI）
- NSIS を使う場合は [NSIS](https://nsis.sourceforge.io/Main_Page) をインストールし、`makensis.exe` を `PATH` に通してください。

モデルをインストーラへ同梱する場合は、`tauri.conf.json` の `tauri.bundle.resources` に対象ファイルを追加してください（例: `models/ggml-large-v3-turbo-q5_0.bin`）。

## 使い方

- 「音声ファイルを選択」→「読み込み」でプレビューを準備して再生できます。
- 「言語（自動/ja/en/zh/ko）」や「英語に翻訳」を設定します。
- 「カタログから選択」→「モデル切替」でモデルを切り替えられます。未取得のモデルは「モデルをダウンロード」または「未DLをまとめてDL」で取得できます（進捗表示あり）。
- 「文字起こし開始」で推論を実行。結果はタイムスタンプ付きで表示され、「クリック再生」ON で該当行から再生が可能です。
- 「解析結果をコピー」で結果全文をクリップボードへコピーできます。

## パフォーマンスのヒント

- リリースビルド（`cargo tauri build`）を利用する
- `whisperGUIapp/config.toml` の `performance.whisper_threads` を CPU コア数に合わせて調整
- `~/.cargo/config.toml` 等に `-C target-cpu=native` を設定すると SIMD 最適化が有効になる場合があります
- 軽量モデル（`tiny/base/small` や量子化版）を選択すると高速です

## 今後の拡張

- マイク録音（UI 連携）
- SRT/VTT へのエクスポート
- 範囲指定の書き起こし（UI は先行実装済み）
- バッチ/キュー処理による同時実行最適化

Tauri 化により、Windows ネイティブ配布（EXE/MSI）が容易です。モデルを同梱するか、初回起動時にダウンロードさせるかは配布ポリシーに応じて選択してください。

---

# WhisperBackendAPI

**whisperGUIapp** と並行して開発された、**Rust + whisper-rs** によるHTTPバックエンドAPIサーバーです。GPUを活用した高速な文字起こしサービスを提供し、whisperGUIappやその他のクライアントからHTTP経由で利用できます。

## 概要

WhisperBackendAPIは、ローカルでWhisperモデルを実行するRESTful APIサーバーです。Axumフレームワークを使用し、マルチパート形式でのファイルアップロード、非同期処理、CORS対応を実装しています。

### 主要な特徴

- **高性能**: whisper-rs + GPU対応による高速処理
- **マルチフォーマット対応**: WAV, MP3, M4A, FLAC, OGG
- **RESTful API**: 標準的なHTTPエンドポイント
- **並行処理**: tokio + spawn_blockingによる効率的なリソース利用
- **セキュリティ**: ファイルサイズ・音声長制限、入力検証
- **CORS対応**: クロスオリジンリクエスト対応
- **設定可能**: TOMLファイルによる柔軟な設定管理

## アーキテクチャ

```
┌─────────────────────────────────┐
│     HTTP Client (whisperGUIapp等)   │
│  - ファイルアップロード               │
│  - レスポンス受信・結果表示           │
└──────────────┬──────────────────┘
               │ HTTP/REST API
┌──────────────▼──────────────────┐
│     WhisperBackendAPI (Axum)       │
│  - Multipart ファイル受信           │
│  - AudioProcessor で前処理          │
│  - spawn_blocking で並行処理        │
│  - CORS・エラーハンドリング          │
└──────────────┬──────────────────┘
               │ PCM 16kHz
┌──────────────▼──────────────────┐
│    whisper-rs / whisper.cpp        │
│  - ggml モデルをロード (Arc共有)     │
│  - GPU対応文字起こし                │
└─────────────────────────────────┘
```

## APIエンドポイント

### 文字起こし

| エンドポイント | メソッド | 説明 |
|---------------|---------|------|
| `/transcribe` | POST | 基本的な文字起こし |
| `/transcribe-with-timestamps` | POST | タイムスタンプ付き文字起こし |

#### リクエスト形式

```bash
# 基本的な文字起こし
curl -F "file=@audio.wav" http://localhost:8080/transcribe

# タイムスタンプ付き（言語指定・翻訳）
curl -F "file=@audio.wav" \
     -F "language=ja" \
     -F "translate_to_english=false" \
     http://localhost:8080/transcribe-with-timestamps
```

#### レスポンス形式

```json
{
  "text": "こんにちは、世界！",
  "language": "ja",
  "duration_ms": 2350,
  "segments": [
    {
      "text": "こんにちは、",
      "start_time_ms": 0,
      "end_time_ms": 1200
    },
    {
      "text": "世界！",
      "start_time_ms": 1200,
      "end_time_ms": 2350
    }
  ],
  "processing_time_ms": 847
}
```

### 情報取得

| エンドポイント | メソッド | 説明 |
|---------------|---------|------|
| `/models` | GET | 利用可能なモデル一覧 |
| `/languages` | GET | サポート言語一覧 |
| `/health` | GET | ヘルスチェック |
| `/stats` | GET | サーバー統計情報 |

## プロジェクト構成

```
WhisperBackendAPI/
├─ src/
│  ├─ main.rs           # サーバーエントリーポイント・ルーター設定
│  ├─ handlers.rs       # APIハンドラー・リクエスト処理
│  ├─ config.rs         # 設定管理・TOML読み込み
│  ├─ models.rs         # データ構造・モデルカタログ
│  ├─ audio.rs          # 音声処理・フォーマット変換
│  └─ whisper.rs        # Whisperラッパー・推論処理
├─ models/              # Whisperモデル配置
├─ config.toml          # サーバー設定
├─ Cargo.toml
└─ README.md            # このファイル
```

## セットアップ

### 必要なシステム依存関係

```bash
# Ubuntu/WSL
sudo apt update
sudo apt install -y cmake build-essential clang llvm libclang-dev pkg-config libssl-dev

# 他のLinuxディストリビューション
# 適切なパッケージマネージャーで同等パッケージをインストール
```

### モデルの準備

```bash
cd WhisperBackendAPI
mkdir -p models

# 基本モデル（軽量・テスト用）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -P models/

# 高性能モデル（設定ファイルのデフォルト）
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin -P models/
```

### 設定ファイル

`config.toml` で各種設定をカスタマイズできます：

```toml
[server]
host = "0.0.0.0"
port = 8080
max_request_size = 104857600  # 100MB

[whisper]
model_path = "models/ggml-large-v3-turbo-q5_0.bin"
language = "auto"
enable_gpu = true

[performance]
whisper_threads = 14
max_concurrent_requests = 10
request_timeout_seconds = 300

[limits]
max_file_size_mb = 50
max_audio_duration_minutes = 30
```

## 実行

### 開発モード

```bash
cd WhisperBackendAPI
cargo run
```

### リリースビルド

```bash
cargo build --release
./target/release/WhisperBackendAPI
```

## 使用例

### 基本的な使用

```bash
# サーバー起動
cd WhisperBackendAPI
cargo run

# 別ターミナルでテスト
curl -F "file=@test.wav" http://localhost:8080/transcribe
```

### whisperGUIappとの連携

whisperGUIapp側でAPIサーバーを指定することで、ローカル処理の代わりにHTTP経由で文字起こしを実行できます（将来的な拡張として検討中）。

### 他言語クライアント

```python
# Python例
import requests

with open('audio.wav', 'rb') as f:
    response = requests.post(
        'http://localhost:8080/transcribe-with-timestamps',
        files={'file': f},
        data={'language': 'ja', 'translate_to_english': 'false'}
    )

result = response.json()
print(f"Text: {result['text']}")
for segment in result.get('segments', []):
    print(f"{segment['start_time_ms']}ms-{segment['end_time_ms']}ms: {segment['text']}")
```

## パフォーマンス最適化

### GPU対応

```toml
# config.toml
[whisper]
enable_gpu = true
```

whisper.cppがCUDA/Metal/Vulkan対応でビルドされている場合、GPU加速が利用されます。

### 並行処理

サーバーは複数のリクエストを同時処理できます：

```toml
[performance]
max_concurrent_requests = 10  # 同時処理数
whisper_threads = 14          # Whisper内部スレッド数
```

### メモリ使用量

- Whisperモデルは起動時に1回だけ読み込まれ、`Arc`で共有されます
- リクエストごとに軽量な`WhisperState`を作成して処理します
- 大型モデル（large-v3）使用時は573MB程度のメモリを消費します

## 監視・運用

### ヘルスチェック

```bash
curl http://localhost:8080/health
```

### 統計情報

```bash
curl http://localhost:8080/stats
```

### ログ

環境変数でログレベルを調整できます：

```bash
RUST_LOG=debug cargo run
```

## 今後の拡張

- **ストリーミング対応**: リアルタイム音声処理
- **認証機能**: APIキー・JWT認証
- **レート制限**: リクエスト頻度制御
- **モデル動的切替**: リクエスト単位でのモデル指定
- **バッチ処理**: 複数ファイル同時処理
- **WebSocket対応**: リアルタイム通信
- **Docker化**: コンテナ対応・デプロイ簡素化

## トラブルシューティング

### よくある問題

1. **コンパイルエラー: cmake not found**
   ```bash
   sudo apt install cmake
   ```

2. **コンパイルエラー: libclang not found**
   ```bash
   sudo apt install libclang-dev clang
   ```

3. **起動エラー: モデルファイルが見つからない**
   - `config.toml`のmodel_pathを確認
   - モデルファイルをダウンロード

4. **GPU が認識されない**
   - whisper-rsのGPU対応ビルドが必要
   - 適切なGPUドライバーがインストールされているか確認

WhisperBackendAPIにより、whisper-rsの高性能な文字起こし機能をHTTP API経由で活用し、様々なクライアントアプリケーションから利用することが可能になります。
