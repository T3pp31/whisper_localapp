WhisperRealtimeAPI – HTTP/SSE API ドキュメント
==============================================

このドキュメントは、低スペックなクライアントを想定した「分割HTTP POST + SSE」によるリアルタイム文字起こしAPIの利用方法をまとめたものです。サーバ側で音声前処理（フレーム化・リサンプル・正規化）とASRへのストリーミング送出を実施します。

対象リリース: 現在のリポジトリの実装（`src/http_api/`, `src/ingest/`）

---

概要
----
- 送信（クライアント→サーバ）: 短いPCMチャンクをHTTPでPOST
- 受信（サーバ→クライアント）: 部分/確定文字起こしをSSEで配信
- WebSocket/WebRTCは廃止し、HTTP/SSEに一本化

運用メリット:
- 単純なHTTPとSSEのみで成立（プロキシやFW越えが容易）
- 双方向制御不要（送信と受信で接続を分離）

---

ベースURLと設定
----------------
- HTTPインジェスト/SSE サーバ: `http://{http_bind_addr}`
  - `config/server.yaml` の `http_bind_addr`（例: `127.0.0.1:8080`）

音声処理パラメータ（入力/ターゲット/フレーム長など）：
- `config/audio_processing.yaml`
  - `input.sample_rate_hz`, `input.channels`（クライアントが送るPCMの仕様）
  - `target.sample_rate_hz`, `target.channels`（ASR入力に揃えるサーバ側の最終仕様）
  - `frame_assembler.frame_duration_ms`（ASRへ送るフレーム長の基準）

---

音声チャンク仕様（HTTP POST）
---------------------------
- 形式: PCM S16LE（符号付き16bit リトルエンディアン）、インターリーブ、`input.channels`
- サンプルレート: `input.sample_rate_hz`
- チャンク長: 数10ms〜数百msを推奨（例: 20〜200ms）。小さめの方がレイテンシは低い。
  - 例: 48kHz / 2ch / 20ms → サンプル数 48,000 × 0.02 = 960、i16が2chで計 960 × 2 サンプル（バイト数は×2）
- サーバ側ではフレーム再構成・リサンプル・正規化を行い、ターゲット仕様（例: 16kHz/mono）に整形してASRへ連携します。

---

HTTP エンドポイント
------------------

1) チャンク送信
^^^^^^^^^^^^^^^^
- `POST /http/v1/sessions/{session_id}/chunk`
  - ヘッダ: `Content-Type: application/octet-stream`
  - ボディ: PCM S16LE インターリーブバイト列
  - 挙動: セッションが未作成なら自動作成してASRのセッションを開始。受領したチャンクをフレーム化して順次ASRへ送出。
  - レスポンス: `204 No Content`
  - エラー:
    - `400 Bad Request` ボディが読み取れない/不正
    - `500 Internal Server Error` サーバ内エラー

例（curl、ダミーデータ送信）：

```
# 16bitリトルエンディアンのダミー0データを200msぶん送る例（48kHz, mono 前提）
# 実運用ではアプリ側でPCM S16LEバッファを構成してください。
BYTES=$((48000 * 2 / 5)) # 200ms * 2byte
head -c ${BYTES} </dev/zero \
  | curl -sS -X POST \
      -H 'Content-Type: application/octet-stream' \
      --data-binary @- \
      http://127.0.0.1:8080/http/v1/sessions/sess-1/chunk
```

2) セッション終了
^^^^^^^^^^^^^^^^^
- `POST /http/v1/sessions/{session_id}/finish`
  - 挙動: 残りバッファをフラッシュしてASRを終了。SSE側には最終文字起こしを送って接続をクローズします。
  - レスポンス: `204 No Content`
  - エラー: `500 Internal Server Error`（セッション未存在/サーバ内エラー）

3) SSE 受信
^^^^^^^^^^^
- `GET /http/v1/sessions/{session_id}/events`
  - レスポンスヘッダ：
    - `Content-Type: text/event-stream`
    - `Cache-Control: no-cache`
    - `Connection: keep-alive`
  - イベント種別：
    - `event: partial` `data: {"text": string, "confidence": number}`
    - `event: final`   `data: {"text": string}`（送信後に接続はクローズ）
  - 備考: SSE接続は「先に開いておく」ことを推奨（`chunk`より先に開いてもOK）。

例（curlでSSEを受信しつつ、別ターミナルでchunk/finishを送信）：

```
curl -N http://127.0.0.1:8080/http/v1/sessions/sess-1/events
```

SSEメッセージ例：

```
id: 1
event: partial
data: {"text":"hello wor", "confidence":0.87}

id: 2
event: final
data: {"text":"hello world"}
```

---

ブラウザからの送信例（簡易）
----------------------------
ブラウザではAudioWorklet等でPCM Float32を取り出し、S16LEへ量子化してPOSTします。

注意: 以下は考え方の参考です。実運用では適切なバッファリング/エラーハンドリングを追加してください。

```
// Float32Array [-1.0, 1.0] → Int16LE へ量子化
function f32ToS16leBytes(float32) {
  const out = new Int16Array(float32.length);
  for (let i = 0; i < float32.length; i++) {
    const s = Math.max(-1, Math.min(1, float32[i]));
    out[i] = s < 0 ? s * 0x8000 : s * 0x7fff;
  }
  return new Uint8Array(out.buffer);
}

async function postChunk(sessionId, pcmBytes) {
  await fetch(`/http/v1/sessions/${sessionId}/chunk`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/octet-stream' },
    body: pcmBytes,
    keepalive: true,
  });
}

// SSE受信
function subscribe(sessionId, onPartial, onFinal) {
  const es = new EventSource(`/http/v1/sessions/${sessionId}/events`);
  es.addEventListener('partial', (ev) => {
    const payload = JSON.parse(ev.data);
    onPartial(payload.text, payload.confidence);
  });
  es.addEventListener('final', (ev) => {
    const payload = JSON.parse(ev.data);
    onFinal(payload.text);
    es.close();
  });
}
```

---

推奨チャンク設計とレイテンシ
-----------------------------
- チャンク間隔は 20〜200ms を目安に調整してください。短いほど部分結果が速く出ますが、HTTPリクエスト数は増えます。
- `config/asr_pipeline.yaml` の `streaming.max_pending_requests` にも依存します。大量のチャンクを過度に先行投入しないよう注意してください。

---

ステータスコードとエラー
----------------------
- 2xx
  - `204` chunk/finish 正常受理
  - `200` SSE接続確立
- 4xx
  - `400` 無効なボディ
- 5xx
  - 内部エラー（セッション未存在/ASR連携失敗など）

---

セキュリティ
------------
- 現状、HTTPインジェスト/SSEにアプリケーションレベルの認証は未導入です。運用では以下を推奨：
  - リバースプロキシでの認証/認可（トークンヘッダ等）
  - 内部ネットワーク限定/MTLS
  - レート制限

---

既知の制限
----------
- SSEは一方向・1接続前提。複数クライアントへの同報は必要に応じて拡張してください。
- `finish` 後、SSEは `final` を送ってクローズします。続けて同じ `session_id` を再利用する場合は新規セッションとして扱ってください。

---

テスト
-----
- `tests/ingest.rs`：PCMインジェストの部分/最終到達確認
- `tests/http_api.rs`：HTTPエンドポイントの基本応答（chunk/finish/SSEヘッダ）
- 実行にはRust依存の取得が必要です（ネットワーク許可が必要）。

---

変更履歴（このブランチ）
-----------------------
- HTTP API（chunk/finish/events）を追加
- PCMインジェスト層（サーバ側処理）を追加
- `config/server.yaml` に `http_bind_addr` を追加
- WebSocket/Signalingを削除し、HTTP/SSEに一本化
