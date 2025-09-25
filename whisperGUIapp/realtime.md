# リアルタイム文字おこし（ローカル専用）設計と実装手順

目的: 既存のオフライン一括文字起こしとは別に、マイク入力をリアルタイムでローカル認識し、UI に逐次反映する機能を、ネットワーク非依存（ローカルのみ）で実装する。

---

## 方針（推奨）
- Rust 側でマイクを直接キャプチャして処理（`cpal` 利用）。
  - 理由: 高頻度の音声チャンクを Tauri IPC でフロント→Rust に渡す方式はオーバーヘッドが大きい。Rust で直接録音すれば低遅延・安定。
- Whisper は既存の `whisper.rs`（whisper-rs 依存）を再利用し、短いウィンドウ（例: 1–5 秒）で繰り返し認識。
- セグメント化は簡易 VAD（`webrtc-vad`）または固定ウィンドウ＋オーバーラップのどちらか。初期は固定ウィンドウで実装し、必要に応じて VAD を追加。
- UI は別ウィンドウ（疑似“別タブ”）として用意（`realtime.html`）。`WebviewWindow` で起動／独立表示。
- 完全ローカル: 外部ネットワークは使用しない（サーバ送信なし）。

---

## 画面要素（`dist/realtime.html`）
- ボタン
  - `開始`（録音＋リアルタイム認識開始）
  - `停止`（録音停止）
  - （任意）デバイス選択セレクト（入力デバイス選択）
- インジケータ
  - 入力レベル（VU メーター: peak / RMS）
  - ステータス（初期化中／録音中／待機など）
- テキストエリア
  - `partial`（途中結果）
  - `final`（確定結果の追記）

---

## Rust 側構成（新規: `src/realtime.rs` 追加）
主要コンポーネント:
- `RealtimeManager`
  - シングルトン。録音スレッド・認識スレッドのライフサイクルを管理。
  - Tauri コマンド `realtime_start`, `realtime_stop`, `realtime_status` を実装。
- 録音スレッド（`cpal`）
  - 取得フォーマット例: 48kHz, f32/mono（環境によっては stereo/f32/i16 → f32 へ変換）。
  - 内部リングバッファ（例: 30 秒）へ追記、同時に短い分析用バッファへも供給。
- 認識スレッド
  - タイマーまたはサンプル閾値（例: 1 秒ごと）で 1–5 秒のウィンドウを抽出。
  - `rubato` で 16kHz へリサンプリング（`audio.rs` のロジックを再利用）。
  - `whisper.rs::WhisperEngine` を使って `transcribe()` を実行し、結果を `partial/final` として Tauri イベントで送信。

イベント設計（フロントへ送る）:
- `realtime-status` { phase: 'starting'|'running'|'stopped'|'error', message?: string }
- `realtime-level` { peak: f32, rms: f32 }
- `realtime-text` { kind: 'partial'|'final', text: string, startMs?: u64, endMs?: u64 }

Tauri コマンド（ローカルのみ・ネットワークなし）:
- `realtime_start(device: Option<String>, language: Option<String>) -> Result<()>`
- `realtime_stop() -> Result<()>`
- `realtime_status() -> Result<RealtimeStatus>`

補足:
- Whisper コンテキスト（モデル）は初回開始時にロードして使い回し（再起動コストを避ける）。
- 競合回避: オフライン一括処理と同時実行は避け、UI で開始ボタンを無効化または警告。

---

## データフロー
1) UI（realtime.html）で「開始」→ `invoke('realtime_start', { device, language })`。
2) Rust 側 `RealtimeManager`:
   - `cpal` で入力ストリーム開始。
   - 認識スレッドを起動（リングバッファから周期的にウィンドウ切り出し → 16kHz 化 → Whisper 認識）。
   - `app.emit('realtime-status', { phase: 'running' })`。
3) UI は `listen('realtime-level')` で VU 表示、`listen('realtime-text')` で partial を上書き、final は下に追記。
4) UI の「停止」→ `invoke('realtime_stop')` でストリーム停止、スレッド join、status を `stopped` へ。

---

## 具体的な実装手順

1. 依存追加（`Cargo.toml`）
   - `cpal = "0.15"`
   - `webrtc-vad = "0.4"`（任意、後からでも可。まずは固定ウィンドウで開始可）
   - `crossbeam-channel = "0.5"`（スレッド間で PCM 受け渡し）
   - `anyhow`, `thiserror`（既存に合わせる）
   - 既存の `rubato`, `whisper-rs` は流用

2. モジュール追加（`src/realtime.rs`）
   - `RealtimeManager`（`OnceCell<Mutex<...>>` などでグローバル管理）
   - `start(device, language)`:
     - WhisperEngine 準備（既存 `whisper.rs` の `WhisperEngine::new()`）
     - cpal 入力ストリーム作成→コールバックで f32 モノに整形してチャンネルに送る
     - 認識スレッド起動（以下の “認識ループ”）
   - `stop()`:
     - ストップフラグ設定、cpal ストリーム停止、認識スレッド join
   - `認識ループ`:
     - 例: 10ms 単位で PCM を受信しつつ、1s ごとに直近 3s–5s のウィンドウを用意
     - 16kHz へ変換（`audio.rs` の `resample_audio` を関数化・再利用）
     - `engine.transcribe(&window_pcm)` を呼ぶ
     - 直近の結果を `partial` として emit、ある程度の静音が続いたらそれまでを `final` として emit

3. 既存コードへの統合（`src/main.rs`）
   - `mod realtime;` を追加
   - Tauri コマンド登録: `realtime_start`, `realtime_stop`, `realtime_status`
   - イベント送信は `app_handle.emit_all(...)` を利用

4. UI（別ウィンドウ）
   - `dist/realtime.html`, `dist/realtime.js` を新規作成
   - `realtime.js`:
     - `const { invoke } = window.__TAURI__.tauri; const event = window.__TAURI__.event;`
     - `start/stop` ボタンで `invoke('realtime_start') / invoke('realtime_stop')`
     - `event.listen('realtime-status'|...'realtime-text'|...'realtime-level', handler)`
   - 既存ウィンドウから起動: `const { WebviewWindow } = window.__TAURI__.window; new WebviewWindow('realtime', { url: 'realtime.html', title: 'リアルタイム文字起こし' });`

5. UI の状態管理
   - 録音中は「開始」無効化、「停止」有効化
   - オフライン一括実行中はリアルタイム開始を抑止（ボタン無効化 or 警告）
   - partial は上段の小さなエリアで上書き、final は下段テキストに追記

6. タイムスタンプ付与（任意）
   - 固定ウィンドウ方式では「ウィンドウ内の相対秒」を元に `startMs/endMs` を算出
   - VAD でセグメント境界が決まる場合は累積時間で管理
   - `realtime-text` イベントに `{ startMs, endMs }` を含める

7. パフォーマンス調整
   - ウィンドウ長: 3–5 秒、ステップ: 0.5–1.0 秒（短いほど低遅延、高負荷）
   - `whisper_threads`: 既存の `config.performance.whisper_threads` を利用
   - モデルは small/base 推奨（大きいほど精度↑/遅延↑）

8. OS 権限とバンドル設定
   - macOS: マイク権限の説明文（`NSMicrophoneUsageDescription`）を Info.plist に追加（Tauri バンドラの設定で挿入）
   - Windows: OS 側のマイク許可が必要（初回ダイアログ）
   - Linux: PulseAudio/ALSA の設定に依存
   - Tauri allowlist: ネットワーク不要。新規コマンドの登録のみで可（`fs` 権限は不要）

---

## 参考スニペット

Tauri コマンド（概略）:
```rust
#[tauri::command]
fn realtime_start(app: tauri::AppHandle, device: Option<String>, language: Option<String>) -> Result<(), String> {
    realtime::manager().start(app, device, language).map_err(|e| e.to_string())
}

#[tauri::command]
fn realtime_stop() -> Result<(), String> {
    realtime::manager().stop().map_err(|e| e.to_string())
}
```

イベント送信（概略）:
```rust
app.emit_all("realtime-text", json!({
  "kind": "partial",
  "text": partial_text,
}))?;
```

UI（起動ボタン; 既存画面から）:
```js
const { WebviewWindow } = window.__TAURI__.window;
document.getElementById('open-realtime').onclick = () => {
  const win = WebviewWindow.getByLabel('realtime') || new WebviewWindow('realtime', {
    url: 'realtime.html', title: 'リアルタイム文字起こし', width: 1000, height: 720
  });
  win.setFocus();
};
```

---

## 実装チェックリスト
- [ ] Cargo.toml に `cpal` 他を追加
- [ ] `src/realtime.rs` 作成（マネージャ、録音、認識スレッド）
- [ ] `src/main.rs` にコマンド登録＋モジュール統合
- [ ] `dist/realtime.html` / `dist/realtime.js` 作成
- [ ] 既存画面に「リアルタイム」起動ボタン追加（任意）
- [ ] イベント購読と UI 更新（partial/final/level/status）
- [ ] 競合制御（オフライン処理との排他）
- [ ] 権限（macOS のマイク使用説明、Windows/Linux 確認）

---

## 今後の拡張
- VAD 導入（`webrtc-vad`）で確定テキストの自然な区切りを改善
- キーワードハイライト、逐次保存（自動保存 .txt/.srt）
- ノイズ低減や AGC（録音段階での軽処理）
- 低遅延チューニング（ウィンドウとステップ幅の最適化）

以上の手順で、ローカル完結のリアルタイム文字おこしを安全に追加できます。

