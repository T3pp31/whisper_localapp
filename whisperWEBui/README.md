# Whisper WebUI

WhisperBackendAPIを利用する音声文字起こしWebインターフェースです。

## 概要

このWebUIは、RustとAxumで構築されたモダンなWebアプリケーションで、音声ファイルのアップロードと文字起こし結果の表示を提供します。

## 機能

### 主要機能
- **音声ファイルアップロード**: ドラッグ&ドロップまたはクリックでファイル選択
- **リアルタイム進捗表示**: アップロード・処理状況のリアルタイム表示
- **結果表示**: プレーンテキストまたはタイムスタンプ付きセグメント表示
- **言語選択**: サポートされている言語の自動検出または手動選択
- **高度なオプション**: 温度設定、無音閾値調整
- **エクスポート機能**: テキスト/JSONファイルのダウンロード

### ユーザーインターフェース
- **レスポンシブデザイン**: デスクトップとモバイルに対応
- **リアルタイムステータス**: バックエンドサーバーとGPUの状態表示
- **統計情報**: サーバーの稼働状況とリクエスト統計
- **通知システム**: 操作結果のリアルタイム通知

## 技術仕様

### バックエンド
- **言語**: Rust
- **フレームワーク**: Axum 0.8
- **HTTPクライアント**: reqwest
- **設定管理**: TOML

### フロントエンド
- **HTML/CSS/JavaScript**: Vanilla技術スタック
- **スタイリング**: カスタムCSS with Flexbox/Grid
- **インタラクション**: モダンな非同期JavaScript

### サポートファイル形式
- **音声**: WAV, MP3, M4A, FLAC, OGG
- **動画**: MP4, MOV, AVI, MKV

## セットアップ

### 前提条件
- Rust 1.70以上
- WhisperBackendAPIが稼働中（デフォルト: http://127.0.0.1:8081）

### インストール

1. **依存関係のインストール**
   ```bash
   cd whisperWEBui
   cargo build --release
   ```

2. **設定ファイルの編集** (必要に応じて)
   ```toml
   [server]
   host = "127.0.0.1"
   port = 3001

   [backend]
   base_url = "http://127.0.0.1:8081"
   timeout_seconds = 300

   [webui]
   title = "Whisper WebUI"
   max_file_size_mb = 100
   allowed_extensions = ["wav", "mp3", "m4a", "flac", "ogg", "mp4", "mov", "avi", "mkv"]
   ```

3. **WebUIサーバーの起動**
   ```bash
   cargo run
   ```

## 使用方法

1. **WebUIにアクセス**
   - ブラウザで `http://127.0.0.1:3001` を開く

2. **音声ファイルのアップロード**
   - ファイルをドラッグ&ドロップするか、クリックしてファイルを選択
   - 必要に応じて言語や処理オプションを設定
   - ファイルが自動的にアップロード・処理される

3. **結果の確認**
   - 文字起こし結果が表示される
   - タイムスタンプ付きの場合、セグメント単位での表示
   - 処理時間、音声長、検出言語などの詳細情報

4. **エクスポート**
   - テキストまたはJSONファイルとしてダウンロード
   - クリップボードへのコピー

## API仕様

### エンドポイント

#### WebUI
- `GET /` - メイン画面
- `GET /static/*` - 静的ファイル（CSS/JS）

#### API
- `POST /api/upload` - ファイルアップロード・文字起こし
- `GET /api/health` - バックエンドのヘルスチェック
- `GET /api/stats` - バックエンドの統計情報
- `GET /api/models` - 利用可能なモデル一覧
- `GET /api/languages` - サポート言語一覧
- `GET /api/gpu-status` - GPU使用状態

### アップロードパラメータ

```javascript
FormData {
  file: File,                    // 音声ファイル（必須）
  language: string,              // 言語コード（オプション）
  with_timestamps: boolean,      // タイムスタンプ付き（オプション）
  temperature: number,           // 温度 0.0-1.0（オプション）
  no_speech_threshold: number    // 無音閾値 0.0-1.0（オプション）
}
```

## 開発

### テストの実行
```bash
cargo test
```

### デバッグビルド
```bash
cargo run
```

### リリースビルド
```bash
cargo build --release
```

## ライセンス

このプロジェクトは適切なライセンスの下で提供されています。
