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
- `whisper_model.yaml`: whisper.cppモデルのパス・スレッド数・言語設定

### 3. ASR gRPCサーバーの起動（同一プロジェクト内の実装を追加）

本プロジェクト内にASR gRPCサーバを実装しました。既定ではwhisper-rs（whisper.cpp）で実推論（CPU）を行います。

- バインド先は `config/server.yaml` の `asr_grpc_bind_addr`（既定: `127.0.0.1:50051`）。
- モデル設定は `config/whisper_model.yaml` を使用します（例: `model_path: models/ggml-large-v3-turbo-q5_0.bin`）。
- 起動コマンド:

```bash
RUST_LOG=info cargo run --bin asr_server
```

バックエンド本体は `GrpcAsrClient` で `config/asr_pipeline.yaml` の `service.endpoint` に接続します。ローカルASRサーバを使う場合、`service.endpoint` を `http://127.0.0.1:50051` に設定してください。
注意: WebRTC → AudioPipeline → ASRへの完全な配線は段階的に実装中です（TODO参照）。

### 4. バックエンド起動（WebSocketシグナリングサーバ）

```bash
RUST_LOG=info cargo run
```

起動後、`config/server.yaml` の `ws_bind_addr` で指定されたアドレスにWebSocketサーバがバインドされます。
クライアントは `ws://<host>/ws?session_id=<id>` で接続し、`SignalingMessage` をJSONで送受信します。
送受信するメッセージ例:
- 送信: `{ "type": "offer", "session_id": "<id>", "sdp": "..." }`
- 受信: `{ "type": "answer", "session_id": "<id>", "sdp": "..." }`
- 送信: `{ "type": "ice_candidate", "session_id": "<id>", "candidate": "..." }`
- 受信(部分結果): `{ "type": "partial_transcript", "session_id": "<id>", "text": "...", "confidence": 0.92 }`
- 受信(最終結果): `{ "type": "final_transcript", "session_id": "<id>", "text": "..." }`

### 5. run.sh での起動とポート競合対策

同梱の `run.sh` は ASR gRPC サーバとバックエンド（WebSocket）を同時に起動します。

```bash
# 既定の設定ディレクトリ(config/)を使う
./run.sh

# 別ディレクトリの設定を使う
WHISPER_REALTIME_CONFIG_DIR=/path/to/config ./run.sh

# ポートを一時的に上書き（YAMLより優先）
ASR_GRPC_BIND_ADDR=127.0.0.1:50052 WS_BIND_ADDR=127.0.0.1:8082 ./run.sh
```

run.sh は起動前にポートの使用状況をチェックし、既に使われている場合は起動を中止してエラーメッセージを表示します。
別サーバや他プロセスと競合する場合は、上記の環境変数でポートを切り替えるか、`config/server.yaml` を変更してください。

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
