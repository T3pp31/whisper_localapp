# リアルタイム文字起こし統合ガイド

whisperWEBuiとWhisperRealtimeAPIを連携させ、リアルタイム文字起こしを実現しました。

## アーキテクチャ

```
[ブラウザ]
    |
    | WebRTC + WebSocket
    v
[whisperWEBui] (Port 3001)
    | WebSocket Proxy
    v
[WhisperRealtimeAPI] (Port 8081)
    |
    | gRPC
    v
[ASR Engine]
```

whisperWEBuiがWebSocketプロキシとして機能し、ブラウザからのWebRTC/WebSocketシグナリングをWhisperRealtimeAPIへ転送します。

## 設定

### 1. whisperWEBui/config.toml

```toml
[realtime]
enabled = true
config_dir = "../WhisperRealtimeAPI/config"
backend_ws_url = "ws://127.0.0.1:8081"
connection_timeout_seconds = 10
default_client_type = "browser"
default_client_name = "Chrome"
default_client_version = "130"
default_token_subject = "web-demo"
heartbeat_interval_ms = 30000
```

- `backend_ws_url`: WhisperRealtimeAPIのWebSocketエンドポイント
- `connection_timeout_seconds`: バックエンド接続タイムアウト（秒）

### 2. WhisperRealtimeAPI/config/server.yaml

```yaml
ws_bind_addr: "127.0.0.1:8081"
asr_grpc_bind_addr: "127.0.0.1:50051"
```

## 起動手順

### 1. WhisperRealtimeAPIを起動

```bash
cd WhisperRealtimeAPI
RUST_LOG=info cargo run --release
```

起動ログ例:
```
INFO signaling service initialized max_sessions=32
INFO starting websocket signaling server addr=127.0.0.1:8081
INFO WebSocket signaling server listening addr=127.0.0.1:8081
```

### 2. whisperWEBuiを起動

```bash
cd whisperWEBui
RUST_LOG=info cargo run --release
```

起動ログ例:
```
設定ファイルを読み込みました
WebUIサーバーアドレス: 0.0.0.0:3001
リアルタイムバックエンド: 有効 (../WhisperRealtimeAPI/config)
WebUIサーバーを起動します: http://0.0.0.0:3001
```

### 3. ブラウザでアクセス

http://localhost:3001 にアクセスし、「リアルタイム文字起こし」タブを開きます。

## 動作フロー

1. **セッション開始**
   - ブラウザから `/api/realtime/session` へPOSTリクエスト
   - whisperWEBuiがWhisperRealtimeAPIのシグナリングサービスを呼び出し
   - セッションIDとICEサーバー情報を取得

2. **WebSocket接続**
   - ブラウザが `/ws/realtime/{session_id}` へWebSocket接続
   - whisperWEBuiがバックエンド `ws://127.0.0.1:8081/ws?session_id={session_id}` へ接続
   - 双方向プロキシ転送開始

3. **WebRTCシグナリング**
   - ブラウザ → whisperWEBui → WhisperRealtimeAPI へOffer SDPを送信
   - WhisperRealtimeAPI → whisperWEBui → ブラウザ へAnswer SDPを返信
   - ICE Candidateを交換

4. **音声ストリーミング**
   - WebRTCでブラウザ→WhisperRealtimeAPIへ音声を送信
   - WhisperRealtimeAPIがOpusデコード→リサンプリング→ASR処理

5. **文字起こし結果配信**
   - WhisperRealtimeAPI → whisperWEBui → ブラウザ へ文字起こし結果を送信
   - `partial_transcript`: 部分結果（認識中）
   - `final_transcript`: 確定結果

## テスト

### 設定テスト

```bash
cd whisperWEBui
cargo test --test realtime_proxy test_config_loading
```

### 統合テスト（手動）

WhisperRealtimeAPIとwhisperWEBuiの両方を起動してから:

```bash
cd whisperWEBui
cargo test --test realtime_proxy test_websocket_proxy_connection -- --ignored
```

## トラブルシューティング

### WebSocket接続失敗

```
ERROR バックエンドWebSocket接続失敗: Connection refused
```

**原因**: WhisperRealtimeAPIが起動していない、またはポートが異なる

**対処**:
1. WhisperRealtimeAPIが起動しているか確認
2. `config.toml`の`backend_ws_url`とWhisperRealtimeAPIの`ws_bind_addr`が一致しているか確認

### タイムアウトエラー

```
ERROR バックエンドWebSocket接続タイムアウト
```

**原因**: バックエンドへの接続に10秒以上かかっている

**対処**:
1. `config.toml`の`connection_timeout_seconds`を増やす
2. ネットワーク設定を確認

### リアルタイム機能が無効

```
ERROR リアルタイムバックエンドが無効です
```

**原因**: `config.toml`で`enabled = false`になっている

**対処**:
```toml
[realtime]
enabled = true
```

## 実装詳細

### プロキシ方式の利点

1. **疎結合**: 各サービスが独立して開発・デプロイ可能
2. **スケーラビリティ**: 負荷に応じてバックエンドをスケール可能
3. **デバッグ容易**: 各サービスのログを分離して確認可能
4. **既存機能への影響なし**: バッチ処理と分離されている

### メッセージフロー

```
クライアント→バックエンド転送:
1. ブラウザ → axum::ws::Message
2. whisperWEBui → tokio_tungstenite::Message に変換
3. バックエンド → 受信

バックエンド→クライアント転送:
1. バックエンド → tokio_tungstenite::Message
2. whisperWEBui → axum::ws::Message に変換
3. ブラウザ → 受信
```

## 参考ファイル

- `whisperWEBui/src/config.rs`: 設定構造体
- `whisperWEBui/src/handlers.rs`: WebSocketプロキシ実装
- `whisperWEBui/static/js/realtime-webrtc.js`: WebRTCクライアント
- `WhisperRealtimeAPI/src/signaling/websocket.rs`: シグナリングハンドラ
- `WhisperRealtimeAPI/src/realtime/mod.rs`: オーケストレータ

## 今後の改善案

1. **認証・認可**: セッションにトークンベース認証を追加
2. **メトリクス収集**: 接続数、転送量、エラー率の監視
3. **自動再接続**: バックエンドとの接続が切れた場合の自動再接続
4. **ロードバランシング**: 複数のバックエンドインスタンスへの分散
5. **TLS/WSS対応**: 本番環境での暗号化通信