# WebRTC+QUICリアルタイム文字起こしバックエンド 実装計画

> 実装は Rust (既存の `src/` ディレクトリ配下の構成) に従って進め、既存モジュールとの整合性を保つ。

## 1. 目的と性能要件
- ブラウザおよびモバイルクライアントからのリアルタイム音声入力を最小遅延で文字起こしする。
- 期待遅延: エンドツーエンドで 150ms 以下（音声取得→推論→字幕反映）。
- 同時接続数・最大帯域・対応ブラウザを要件定義書に明文化し、`config/system_requirements.yaml` に設定パラメータとして集約する。

## 2. アーキテクチャ設計
- クライアント: MediaStream API で音声取得 → `RTCPeerConnection` + `RTCQuicTransport` を確立し、音声メディアストリームと制御用QUICストリームを送受信。
- シグナリング: 既存バックエンドに HTTPS + WebSocket を追加。セッション確立時のSDP交換、トークン認証、リソース割り当てを担当。
- サーバ: Pion WebRTC + quic-go（または libwebrtc + MsQuic）の組合せでQUICストリームを終端。Rust 実装では既存 `src/` に合わせたモジュール分割でハンドラ/サービス層を構築し、音声フレームはストリーミングASRサービスへ即時パイプ。
- ASR: Whisper系ストリーミング推論（例: FasterWhisper）をgRPCサービスとして分離。設定値は `config/asr_pipeline.yaml` で管理。

## 3. 実装手順
1. 要件確定
   - 対応ブラウザ、モバイルOS、証明書運用方針、GPU/CPUリソースを定義。
   - 運用パラメータを `config/system_requirements.yaml` に記述し、読み込みユーティリティを整備。
2. シグナリング実装
   - `src/` 配下に既存スタイルを踏襲したシグナリングモジュールを追加し、セッション開始/終了API、トークン検証、リソース管理ロジックを実装。
   - `tests/signaling/test_session_api.rs` を作成し、正常系・認証失敗・リトライを検証。
3. WebRTC + QUIC トランスポート
   - クライアント側で QUIC データ/制御ストリーム実装、帯域プロファイル調整。
   - サーバ側でICE/DTLS/TLS1.3 over QUICのハンドシェイク実装、TURN over QUIC導入可否を判断し、Rustでトランスポートハンドラを組む。
   - `tests/integration/test_quic_transport.rs` を追加し、接続確立・切断・再接続・帯域制限シナリオを自動化。
4. 音声処理パイプライン
   - 受信フレームの再構成、サンプルレート変換、正規化処理を `src/audio_pipeline` にRustで実装し、設定値は `config/audio_processing.yaml` で管理。
   - `tests/audio/test_frame_reconstructor.rs` と `tests/audio/test_resampler.rs` を追加し、精度とレイテンシを検証。
5. ストリーミングASR統合
   - gRPC で Whisper 推論サービスを呼び出し、部分結果と最終結果を QUIC 双方向ストリームで返却。Rustクライアントは `src/asr` モジュールとして実装。
   - `tests/asr/test_streaming_inference.rs` で部分文字起こしのタイミング・最終文字起こしをテスト。
6. 双方向制御と復旧
   - QUIC ストリームを `audio`, `partial_transcript`, `final_transcript`, `control` に論理分離。
   - パケットロス通知・再送制御・ビットレート調整ロジックを実装し、`tests/integration/test_network_resilience.rs` で遅延/ロス環境を再現。
7. モニタリングと運用
   - 接続統計（RTT, Jitter, Loss, 推論レイテンシ）を収集し Prometheus へエクスポート。閾値を `config/monitoring.yaml` に記載。
   - CI にてユニット/統合/負荷テストを実行し、レポートを保存。

## 4. テスト戦略
- すべての新規テストは `tests/` 配下にまとめ、ユニット（signaling/audio/asr）・統合（quic_connection/network_resilience）・負荷試験を分類。
- `cargo test` を基本としつつ、必要に応じて統合テスト用の `scripts/run_tests.sh` を整備し、ローカルおよびCIでの自動実行を標準化。
- 仮想ネットワーク（tc/netem 等）を使った遅延・損失シナリオを統合テストに組み込み、回帰テストに追加。

## 5. 残タスク & 次アクション
- QUIC 対応 WebRTC スタックの選定（libwebrtc ビルド or Pion + quic-go）。
- Whisper ストリーミング推論用のサービス構成（プロセス分離かライブラリ直結か）を決定。
- 利用する Rust ツールチェーン（stable/betaやMSRV）の確認（ユーザーからの情報待ち）。
- 証明書配備とセキュリティレビューを行い、本番運用手順書を作成。
