# WhisperRealtimeAPI

WebRTC + QUICを使用したリアルタイム文字起こしバックエンド

## 概要

このプロジェクトは、ブラウザおよびモバイルクライアントからのリアルタイム音声入力を最小遅延で文字起こしするためのバックエンドシステムです。

## アーキテクチャ

- **WebRTCトランスポート**: ブラウザから音声ストリームを受信
- **QUICストリーム**: 文字起こし結果の双方向ストリーミング
- **Opusデコーダー**: WebRTCからの音声をデコード
- **音声処理パイプライン**: リサンプリング、正規化
- **ASR gRPCクライアント**: Whisper推論サービスとの通信

## セットアップ

### 1. 依存関係のインストール

```bash
cd /root/github/whisper_localapp/WhisperRealtimeAPI
cargo build
```

### 2. 設定ファイル

`config/` ディレクトリに以下の設定ファイルがあります：

- `system_requirements.yaml`: システム要件、ICEサーバー設定
- `audio_processing.yaml`: 音声処理パラメータ
- `asr_pipeline.yaml`: ASR設定
- `monitoring.yaml`: モニタリング設定
- `server.yaml`: サーバのバインド先（例: `ws_bind_addr: "127.0.0.1:8080"`）

### 3. ASR gRPCサーバーの起動（必須）

本バックエンドは起動時に `GrpcAsrClient` を使用し、外部ASRサーバへ接続します。GPUを使用する場合は、ASRサーバ（例：FasterWhisper/CTranslate2 CUDA対応）をGPU有効で起動してください。

接続先は `config/asr_pipeline.yaml` の `service.endpoint` を使用します（既定: `http://localhost:50051`）。

```bash
# ASRサービスの起動例（別プロジェクト・参考）
# python -m faster_whisper_server --port 50051  # venvは環境に合わせて選択
```

### 4. バックエンド起動（WebSocketシグナリングサーバ）

```bash
RUST_LOG=info cargo run
```

起動後、`config/server.yaml` の `ws_bind_addr` で指定されたアドレスにWebSocketサーバがバインドされます。
クライアントは `ws://<host>/ws?session_id=<id>` で接続し、`SignalingMessage` をJSONで送受信します。

## テスト

```bash
# ユニットテスト
cargo test

# 統合テスト
cargo test --test integration
```

## フロントエンドとの統合

whisperWEBuiプロジェクトと連携して動作します：

1. whisperWEBuiを起動
2. ブラウザでリアルタイムタブを開く
3. セッションを開始してWebRTC接続確立
4. マイクから音声入力→リアルタイム文字起こし

## 実装状況

### 完了
- ✅ WebRTCトランスポート層
- ✅ QUICストリームハンドラ
- ✅ Opusデコーダー統合
- ✅ ASR gRPCクライアント
- ✅ WebSocketシグナリング
- ✅ 基本的な統合テスト

### TODO
- [ ] WebRTCとASRパイプラインの完全統合
- [ ] QUICストリームの実装完了（送受信ロジック）
- [ ] Prometheusメトリクス収集
- [ ] ネットワーク遅延・損失対応
- [ ] 負荷テスト
- [ ] 本番環境用証明書設定

## 技術スタック

- **Rust**: 高性能・低遅延処理
- **WebRTC**: リアルタイム音声通信
- **QUIC**: 高速双方向ストリーミング
- **Opus**: 音声コーデック
- **gRPC**: ASRサービス通信
- **tokio**: 非同期ランタイム

## ライセンス

[ライセンス情報]
