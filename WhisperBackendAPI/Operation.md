# Operation

## WhisperBackendAPI を systemd で常駐運用する

このドキュメントは、WhisperBackendAPI を Linux 上で systemd の管理下に常駐実行し、クラッシュ時の自動復旧・再起動時の自動起動・ログ集約を行うための運用手順をまとめたものです。

---

## 目的（Why）

- 起動自動化（OS 起動時に自動起動）
- クラッシュ時の自動復帰（Restart）
- ログの一元管理（journalctl）
- 権限最小化・サービス分離（専用ユーザー、Sandbox オプション）

---

## 前提（Prerequisites）

- Rust リリースビルド済み（GPU 利用時は GPU バックエンド有効化ビルド）
  - CPU: `cargo build --release`
  - GPU: `NVCC_PREPEND_FLAGS="--std=c++14 -Wno-deprecated-gpu-targets -U_GNU_SOURCE" CMAKE_CUDA_STANDARD=14 ./build.sh gpu`

- NVIDIA 環境で GPU を使う場合
  - ドライバ導入済み（`nvidia-smi` が動作）
  - 任意: `nvidia-persistenced` の常駐（後述）

---

## 配置（Layout）

1) 作業ディレクトリと専用ユーザー

```bash
sudo useradd -r -s /usr/sbin/nologin whisper || true
sudo mkdir -p /opt/whisper/{models,temp,uploads}
sudo chown -R whisper:whisper /opt/whisper
```

2) バイナリ・設定・モデルの配置（例）

```bash
# ビルド成果物とスクリプト、設定を配置
sudo cp target/release/WhisperBackendAPI /opt/whisper/
sudo cp run.sh config.toml /opt/whisper/

# モデルファイル（例: ggml-large-v3-turbo-q5_0.bin）
sudo cp models/ggml-large-v3-turbo-q5_0.bin /opt/whisper/models/

sudo chown -R whisper:whisper /opt/whisper
```

注意:

- 本アプリは `WorkingDirectory` 配下の `config.toml` を参照します。
- `whisper.model_path` が実在すること（例: `models/ggml-large-v3-turbo-q5_0.bin`）。

---

## systemd ユニットファイル

`/etc/systemd/system/whisper-backend.service` を作成します。

```
[Unit]
Description=WhisperBackendAPI (Axum + whisper-rs)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=whisper
Group=whisper
WorkingDirectory=/opt/whisper

# ログと GPU 設定（必要に応じて追記）
Environment=RUST_LOG=info
Environment=WHISPER_CUBLAS=1
# Environment=CUDA_PATH=/usr/local/cuda
# Environment=LD_LIBRARY_PATH=/usr/local/cuda/lib64:%s

ExecStart=/opt/whisper/run.sh gpu release

Restart=always
RestartSec=3

# セキュリティ強化（必要に応じ調整）
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ReadWritePaths=/opt/whisper/models /opt/whisper/temp /opt/whisper/uploads

[Install]
WantedBy=multi-user.target
```

反映と起動:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now whisper-backend.service
```

状態/ログ確認:

```bash
systemctl status whisper-backend.service
journalctl -u whisper-backend -f
```

---

## run.sh の改善（任意だが推奨）

systemd から正しく本体プロセスを監視させるため、`run.sh` の最後（リリース実行部分）を `exec` で置き換えることを推奨します。

変更前（抜粋）:

```
./target/release/WhisperBackendAPI
```

変更後（推奨）:

```
exec ./target/release/WhisperBackendAPI
```

`exec` によりシェルプロセスではなくアプリ本体が PID を継承するため、systemd の停止/再起動/状態遷移が確実になります。

---

## NVIDIA ドライバ常駐（GPU 運用時の推奨）

GPU の初期化オーバーヘッドを抑えるため、ドライバに同梱される永続化デーモンを有効化します。

```bash
sudo systemctl enable --now nvidia-persistenced
```

---

## 運用コマンド早見表

```bash
# 起動/停止/再起動
sudo systemctl start whisper-backend
sudo systemctl stop whisper-backend
sudo systemctl restart whisper-backend

# 自動起動設定
sudo systemctl enable whisper-backend
sudo systemctl disable whisper-backend

# ログ追跡
journalctl -u whisper-backend -f

# ヘルス確認
curl -s http://127.0.0.1:8080/health | jq .
```

---

## トラブルシューティング

- サービスが即時終了する → `journalctl -u whisper-backend -n 200` を確認
- モデル未検出エラー → `config.toml` の `whisper.model_path` と実ファイルの整合性を確認
- 大容量アップロードで 413 → フロントのプロキシ（nginx 等）の `client_max_body_size` を `server.max_request_size` と合わせる
- GPU が使われない → GPU ビルドと `WHISPER_CUBLAS=1`、CUDA ライブラリパス（`LD_LIBRARY_PATH`）を確認
