# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## プロジェクト概要

WhisperGUIappは、whisper-rs (whisper.cppのRustバインディング) を用いてローカルで音声を文字起こしするためのWindows向けデスクトップアプリケーションです。Tauri 1.xフレームワークを使用し、HTML/CSS/JavaScriptのフロントエンドとRustのバックエンドを連携させています。

## 主要な開発コマンド

```bash
# プロジェクトディレクトリに移動
cd whisperGUIapp

# 開発モード（ホットリロードなしの簡易起動）
cargo tauri dev

# リリースビルド
cargo tauri build

# Cargoテスト実行
cargo test

# Whisperモデルのダウンロード（WSL/Git Bash環境）
./download_models.sh
```

## アーキテクチャとコード構成

### 全体アーキテクチャ
- **フロントエンド**: `dist/` - 静的HTML/CSS/JavaScript (Tauri WebView2)
- **バックエンド**: `src/` - Rust（Tauri API）
- **設定管理**: `config.toml` - アプリケーション設定
- **モデル**: `models/` - ggml形式のWhisperモデル

### Rustバックエンドモジュール構成
- `main.rs`: Tauriアプリケーションのエントリーポイント、AppState管理
- `config.rs`: 設定ファイル（config.toml）の読み込み・管理
- `audio.rs`: 音声ファイルの前処理（デコード、リサンプリング）
- `whisper.rs`: Whisper推論エンジンの管理
- `models.rs`: モデル情報の管理・ダウンロード機能

### データフロー
1. フロントエンド → Tauri invoke → バックエンドコマンド
2. `AudioProcessor` → 音声前処理（16kHz PCM変換）
3. `WhisperEngine` → 文字起こし実行
4. 進捗イベント → フロントエンドへemit
5. 結果テキスト → フロントエンドで表示・保存

## 設定ファイル

### config.toml
アプリケーションの設定は`config.toml`で管理：
- Whisperモデルパス・言語設定
- 音声処理パラメータ
- GUI設定
- パフォーマンス設定
- 出力フォーマット設定

### tauri.conf.json
Tauriフレームワークの設定：
- バンドル設定（MSI生成）
- ウィンドウ設定
- API許可設定（dialog, fs, pathなど）

## 開発時の注意点

### モデル配置
- `models/`ディレクトリにggml形式のWhisperモデルが必要
- 開発時は`./download_models.sh`でtiny/base/smallモデルを取得可能
- `config.toml`の`model_path`設定と一致させる

### ビルド要件
- Rust 1.75以上
- Microsoft Visual C++ Build Tools
- WebView2 Runtime（Windows 11に標準搭載）
- 推奨: `cargo install tauri-cli --version ^1.5`

### 配布形式
- デフォルト: MSI形式（`target/release/bundle/msi/`）
- NSIS使用時: EXE形式（`target/release/bundle/nsis/`）
- `models/`と`config.toml`は自動的にバンドルされる

## 主要な依存関係

- **tauri**: デスクトップアプリフレームワーク
- **whisper-rs**: Whisper音声認識ライブラリ
- **symphonia**: 音声ファイルデコード
- **rubato**: 音声リサンプリング
- **cpal**: 音声録音（将来機能）
- **serde/toml**: 設定ファイル管理

## 今後の拡張予定

- マイク録音機能（`AudioProcessor::start_recording`のTauriコマンド化）
- SRT/VTTエクスポート機能
- モデル選択・ダウンロードUI
- 非同期処理の最適化