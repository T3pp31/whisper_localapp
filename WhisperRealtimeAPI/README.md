# WhisperRealtimeAPI

HTTP(SSE) を使用したリアルタイム文字起こしバックエンド

## 概要

このプロジェクトは、ブラウザおよびモバイルクライアントからのリアルタイム音声入力を低遅延で文字起こしするためのバックエンドシステムです。クライアントは短いPCMチャンクをHTTPでPOSTし、サーバはSSEで部分/確定文字起こしを配信します。

## アーキテクチャ

- **HTTPインジェスト**: 短いPCM(S16LE)チャンクをPOSTで受信
- **SSE配信**: 部分/最終文字起こし結果をイベント配信
- **音声処理パイプライン**: リサンプリング、正規化、フレーム化
- **ASR gRPCクライアント**: Whisper推論サービスとの通信

## セットアップ

### 1. 依存関係のインストール

```bash
cd /root/github/whisper_localapp/WhisperRealtimeAPI
cargo build
```

### 2. 設定ファイル

`config/` ディレクトリに以下の設定ファイルがあります：

- `system_requirements.yaml`: システム要件
- `audio_processing.yaml`: 音声処理パラメータ
- `asr_pipeline.yaml`: ASR設定
- `monitoring.yaml`: モニタリング設定
- `server.yaml`: サーバのバインド先（例: `http_bind_addr: "127.0.0.1:8080"`）
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
HTTP → AudioPipeline → ASR で配線されています。

### 4. バックエンド起動（HTTPインジェスト + SSE）

```bash
RUST_LOG=info cargo run
```

起動後、`config/server.yaml` の `http_bind_addr` でHTTPサーバがバインドされます。
クライアントは以下を使用します：
- 送信: `POST /http/v1/sessions/{session_id}/chunk` （Content-Type: application/octet-stream, PCM S16LE）
- 終了: `POST /http/v1/sessions/{session_id}/finish`
- 受信: `GET  /http/v1/sessions/{session_id}/events` （SSE, event: partial/final）

### 5. run.sh での起動とポート競合対策

同梱の `run.sh` は ASR gRPC サーバとバックエンド（HTTP/SSE）を同時に起動します。

```bash
# 既定の設定ディレクトリ(config/)を使う
./run.sh

# 別ディレクトリの設定を使う
WHISPER_REALTIME_CONFIG_DIR=/path/to/config ./run.sh

# ポートを一時的に上書き（YAMLより優先）
ASR_GRPC_BIND_ADDR=127.0.0.1:50052 HTTP_BIND_ADDR=127.0.0.1:8082 ./run.sh
```

run.sh は起動前にポートの使用状況をチェックし、既に使われている場合は起動を中止してエラーメッセージを表示します。
別サーバや他プロセスと競合する場合は、上記の環境変数でポートを切り替えるか、`config/server.yaml` を変更してください。

## テスト

```bash
# ユニットテスト
cargo test

（統合テストはHTTP/SSE中心のため通常の `cargo test` のみで十分です）
```

## フロントエンドとの統合（例）

- AudioWorklet/MediaRecorderなどでPCM S16LEバッファを作成
- 200ms程度ごとに `/chunk` へPOST、イベントはSSEで購読

## 実装状況

### 完了
- ✅ HTTPインジェスト + SSE 配信
- ✅ 音声パイプライン（リサンプル/正規化/フレーム化）
- ✅ ASR gRPCクライアント/サーバ
- ✅ ユニットテスト（インジェスト/HTTP基本応答/設定）

### TODO
- [ ] Prometheusメトリクス収集
- [ ] ネットワーク遅延・損失対応
- [ ] 負荷テスト

## 技術スタック

- **Rust**: 高性能・低遅延処理
- **HTTP/SSE**: シンプルな双方向（片方向×2）のストリーミング
- **Opus**: （必要時）音声コーデック
- **gRPC**: ASRサービス通信
- **tokio**: 非同期ランタイム

## ライセンス

[ライセンス情報]
