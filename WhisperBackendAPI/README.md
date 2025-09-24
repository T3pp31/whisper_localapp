# WhisperBackendAPI

Whisper (whisper.cpp / whisper-rs) を使ったローカル音声文字起こしバックエンドです。CPU で動作しますが、GPU を使った高速化にも対応しています（環境・ビルド設定が必要）。

本リポジトリでは GPU を使うためのコード修正が入っており、実行時フラグに加えて「GPU対応でビルド」することで GPU が有効になります。GPU での初期化に失敗した場合は自動で CPU にフォールバックします。

---

## 目次

- 概要
- 前提条件
- クイックスタート
- **GPU問題解決の完全ガイド**
- GPU ビルドと起動（CUDA/OpenCL）
- 設定（config.toml）
- 動作確認のしかた
- パフォーマンスのヒント
- トラブルシュート

---

## 概要

- ランタイム: Rust + Axum
- ASR エンジン: whisper-rs（内部で whisper.cpp を使用）
- モデル形式: ggml/gguf（例: `ggml-large-v3-turbo-q5_0.bin` など）
- GPU 対応: 本リポジトリは CUDA/cuBLAS と OpenCL を統合済み（ビルド時に有効化が必要）。Metal / OpenVINO は未統合です。
- フォールバック: GPU 初期化に失敗すると CPU へ自動切替（ログ出力あり）

---

## 前提条件

- Rust/Cargo が利用できること
- CMake とビルドツールチェーンが利用できること
- GPU を使う場合は、各プラットフォームの前提を満たすこと
  - NVIDIA: CUDA Toolkit + ドライバ（cuBLAS 利用）
  - macOS: Metal（このリポジトリでは未統合）
  - Intel: OpenVINO（このリポジトリでは未統合）

> 重要: GPU を使うかどうかは「実行時の設定」だけでなく「ビルド時に GPU バックエンドを有効化しているか」に依存します。

---

## クイックスタート

1) 依存の用意（必要に応じて）

- CPU のみで試す場合は特別なセットアップは不要です。
- GPU を使う場合は、後述の「GPU ビルドと起動」を参照して環境を整えてください。

2) モデルの配置

- 既定では `models/ggml-large-v3-turbo-q5_0.bin` を参照します。
- 別モデルを使う場合は `config.toml` の `whisper.model_path` を変更してください。

3) 起動（CPU）

- `cargo run` または `cargo run --release`

起動時に以下のようなログが出ます:

- `Whisperモデルを読み込みました: <path> (GPU: enabled|disabled)`
- GPU 初期化に失敗すると `GPU初期化に失敗しました。CPUで再試行します: ...` と表示され CPU で継続します。

---

## GPU問題解決の完全ガイド

### 🚨 GPU使用時の問題と解決策

**問題**: `WHISPER_CUBLAS=1 cargo run --release` でビルドしているのにGPUが使われない

**原因**: ビルド時のGPU設定とランタイム設定の両方が必要

### 📋 完全解決手順（NVIDIA CUDA）

#### 1. 環境確認

```bash
# GPU確認
nvidia-smi

# CUDA確認
nvcc --version

# CUDAライブラリ確認
ls -la /usr/local/cuda/lib64/libcudart.so
ls -la /usr/local/cuda/lib64/libcublas.so
```

#### 2. 完全リビルド（重要）

```bash
# 既存ビルド成果物を完全削除
cargo clean

# GPU対応ビルド
WHISPER_CUBLAS=1 cargo build --release --features cuda,opencl
```

#### 3. GPU設定確認

`config.toml` で以下が `true` に設定されていることを確認：

```toml
[whisper]
enable_gpu = true
```

#### 4. GPU対応で起動

```bash
# 環境変数を設定して起動
WHISPER_CUBLAS=1 ./target/release/WhisperBackendAPI
```

#### 5. GPU使用確認

起動後、ブラウザまたはcurlで以下にアクセス：

```bash
# 詳細なGPU状態を確認
curl http://localhost:8080/gpu-status | jq

# 簡易確認
curl http://localhost:8080/health | jq
```

#### 6. 実際に音声でテスト

文字起こし実行時のログで以下が表示されることを確認：

```
🚀 GPU使用で文字起こしを開始します...
⏱️  推論処理時間: 1234.56ms (GPU)
```

### 🛠️ 便利スクリプト利用

上記手順を自動化したスクリプトを提供しています：

```bash
# 1. GPU対応ビルド
./build.sh gpu

# 2. GPU対応起動
./run.sh gpu

# 3. GPU状態テスト
./test_gpu.sh quick

# 4. APIテスト（別ターミナル）
./test_gpu.sh api
```

### 🔍 トラブルシューティングツール

#### GPU診断コマンド

```bash
# 環境診断
./test_gpu.sh quick

# 詳細テスト（モデルファイルが必要）
./test_gpu.sh full

# 性能比較（CPU vs GPU）
./test_gpu.sh bench
```

#### 期待される出力例

正常なGPU使用時の起動ログ：

```
=== GPU設定情報 ===
設定でGPU有効化: true
WhisperContextParameters.use_gpu: true
WHISPER_CUBLAS環境変数: 1
CUDA feature is enabled
✓ GPU対応のWhisperコンテキストの初期化に成功しました
✓ GPUアクセラレーションが有効です
✓ Whisperモデルを読み込みました: models/ggml-large-v3-turbo-q5_0.bin (GPU: 設定有効 -> 実際: 有効)
==================
```

### 🚫 よくある失敗例

1. **環境変数なしでの実行**

   ```bash
   # ❌ 間違い（環境変数なし）
   ./target/release/WhisperBackendAPI

   # ✅ 正しい（環境変数あり）
   WHISPER_CUBLAS=1 ./target/release/WhisperBackendAPI
   ```

2. **cargo cleanを忘れる**

   ```bash
   # ❌ 間違い（古いビルドが残る）
   WHISPER_CUBLAS=1 cargo build --release

   # ✅ 正しい（完全リビルド）
   cargo clean
   WHISPER_CUBLAS=1 cargo build --release --features cuda,opencl
   ```

3. **設定ファイルでGPU無効**

   ```toml
   # ❌ 間違い
   [whisper]
   enable_gpu = false

   # ✅ 正しい
   [whisper]
   enable_gpu = true
   ```

---

## GPU ビルドと起動（CUDA/OpenCL）

GPU を使うには、whisper.cpp の GPU バックエンドを有効にしてビルドする必要があります。

### 便利なスクリプトを用意しました

#### ビルド

```bash
# GPU対応ビルド（推奨）
./build.sh gpu

# CPU専用ビルド
./build.sh cpu
```

#### 実行

```bash
# GPU モードで実行
./run.sh gpu

# CPU モードで実行
./run.sh cpu

# 開発モード（cargo run）
./run.sh gpu dev
```

#### GPU状態のテスト

```bash
# 基本テスト
./test_gpu.sh quick

# 性能比較テスト（モデルファイルが必要）
./test_gpu.sh full

# APIエンドポイントテスト（サーバー起動中）
./test_gpu.sh api
```

### 手動でのビルドと起動

- NVIDIA (CUDA / cuBLAS)
  - ビルド: `WHISPER_CUBLAS=1 cargo build --release --features cuda,opencl`
  - 実行: `WHISPER_CUBLAS=1 ./target/release/WhisperBackendAPI`
  - 備考: CUDA Toolkit と対応ドライバが必要です。切替後は `cargo clean` すると確実です。

- OpenCL（実験的）
  - ビルド: `WHISPER_OPENCL=1 cargo build --release --features opencl`
  - 実行: `WHISPER_OPENCL=1 ./target/release/WhisperBackendAPI`
  - 備考: OpenCL ランタイム（ICD）と対応ドライバが必要です。性能・安定性は環境に依存します。

### GPU状態の確認方法

起動後、以下のエンドポイントでGPU使用状態を確認できます：

```bash
# GPU状態確認
curl http://localhost:8080/gpu-status | jq

# ヘルスチェック
curl http://localhost:8080/health | jq
```

> 注: 上記は代表例です。環境変数の有効値やサポート状況は whisper-rs / whisper.cpp のバージョンに依存します。また、GPUライブラリの自動検出は Linux を対象に実装されています。

---

## 設定（config.toml）

`config.toml`（初回起動時は自動生成）

- `whisper.model_path`: モデルファイルのパス（例: `models/ggml-large-v3-turbo-q5_0.bin`）
- `whisper.language`: 既定言語（`auto` で自動検出）
- `whisper.enable_gpu`: 実行時に GPU を使うかの希望フラグ（true/false）
  - true でも「GPU バックエンド未ビルド」の場合は CPU にフォールバックします
- `performance.whisper_threads`: Whisper のスレッド数（CPU 側の並列度）

例: `config.toml:9-17` と `config.toml:21-33` を参照

---

## 動作確認のしかた

### 起動時ログの確認

- GPU の有効/無効が詳細に表示されます：

```
=== GPU設定情報 ===
設定でGPU有効化: true
WhisperContextParameters.use_gpu: true
WHISPER_CUBLAS環境変数: 1
CUDA feature is enabled
✓ GPU対応のWhisperコンテキストの初期化に成功しました
✓ GPUアクセラレーションが有効です
✓ Whisperモデルを読み込みました: models/ggml-large-v3-turbo-q5_0.bin (GPU: 設定有効 -> 実際: 有効)
```

### APIエンドポイントでの確認

サーバー起動後、以下のエンドポイントが利用可能です：

- `GET /gpu-status` - GPU使用状態の詳細確認
- `GET /health` - ヘルスチェック（GPU状態含む）
- `GET /stats` - サーバー統計情報
- `GET /models` - 利用可能なモデル一覧
- `GET /languages` - サポートされている言語一覧

### 文字起こし実行時のログ

実際に文字起こしを行うと、GPUまたはCPU使用が表示されます：

```
🚀 GPU使用で文字起こしを開始します...
⏱️  推論処理時間: 1234.56ms (GPU)
```

---

## パフォーマンスのヒント

- 量子化モデル（例: `*-q5_0.bin`）でも GPU で一部オフロードされますが、FP16 モデルより速度向上が限定的なケースがあります。十分な VRAM がある場合は FP16 モデルも検討してください。
- `performance.whisper_threads` は CPU スレッド数です。GPU 利用時も前処理や一部計算で意味がありますが、CPU単体時ほど支配的ではありません。
- `--release` ビルドでの実行を推奨します。

---

## トラブルシュート

### GPU関連の問題

#### 1. GPUが使われない（最重要）

**症状**: `WHISPER_CUBLAS=1 cargo run --release` で起動してもCPUで実行される

**解決手順**:

```bash
# Step 1: 完全リビルド
cargo clean
WHISPER_CUBLAS=1 cargo build --release --features cuda,opencl

# Step 2: 設定確認
grep "enable_gpu" config.toml  # true になっているか確認

# Step 3: GPU対応起動
WHISPER_CUBLAS=1 ./target/release/WhisperBackendAPI

# Step 4: 状態確認
curl http://localhost:8080/gpu-status | jq '.gpu_actually_enabled'
```

**または便利スクリプトを使用**:

```bash
./build.sh gpu
./run.sh gpu
./test_gpu.sh quick
```

#### 2. CUDA環境の問題

**症状**: `CUDA Runtime library not found` または初期化エラー

**確認コマンド**:

```bash
# GPU認識確認
nvidia-smi

# CUDA確認
nvcc --version

# ライブラリ存在確認
ls -la /usr/local/cuda/lib64/libcudart.so
ls -la /usr/local/cuda/lib64/libcublas.so

# パス設定確認
echo $CUDA_PATH
echo $LD_LIBRARY_PATH
```

**解決方法**:

```bash
# 環境変数設定（~/.bashrcに追加）
export CUDA_PATH=/usr/local/cuda
export PATH=$CUDA_PATH/bin:$PATH
export LD_LIBRARY_PATH=$CUDA_PATH/lib64:$LD_LIBRARY_PATH
```

#### 3. 性能が期待値より遅い

**診断方法**:

```bash
# 性能比較テスト
./test_gpu.sh bench

# GPU使用率監視（別ターミナル）
watch -n 1 nvidia-smi
```

**改善策**:

- より大きなモデル（FP16）を使用
- `config.toml` の `whisper_threads` を調整
- GPU VRAMに十分な空きがあることを確認

#### 4. API経由での詳細診断

```bash
# GPU状態詳細確認
curl -s http://localhost:8080/gpu-status | jq '.recommendations[]'

# システム全体確認
curl -s http://localhost:8080/health | jq
```

### その他の問題

#### モデルファイルが見つからない

**症状**: `Whisperモデルファイルが見つかりません`

**解決方法**:

```bash
# モデルダウンロード（例）
mkdir -p models
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin -P models/

# 設定ファイル確認
grep "model_path" config.toml
```

#### メモリ不足

**症状**: OOM (Out of Memory) エラー

**対策**:

- より小さいモデルを使用（base, small など）
- `config.toml` の `max_file_size_mb` を削減
- `whisper_threads` を削減

#### ポート競合

**症状**: `Address already in use`

**解決方法**:

```bash
# ポート使用状況確認
lsof -i :8080

# 設定変更
sed -i 's/port = 8080/port = 8081/' config.toml
```

### デバッグモード

問題特定のため、詳細ログを有効化：

```bash
# 環境変数でログレベル設定
RUST_LOG=debug ./run.sh gpu dev
```

---

## 実装メモ（本リポジトリでの対応）

### 📁 ファイル構成とGPU対応修正

本リポジトリでは、GPU使用問題を解決するため以下の修正を実装しています：

#### 🔧 コア修正

- **`Cargo.toml`**: whisper-rs は optional。Cargo フィーチャーで CUDA/OpenCL を伝播

  ```toml
  [dependencies]
  whisper-rs = { version = "0.11", optional = true }

  [features]
  default = ["whisper"]
  whisper = ["whisper-rs"]
  cuda = ["whisper-rs/cuda"]
  opencl = ["whisper-rs/opencl"]
  ```

- **`build.rs`**: CUDA環境変数とライブラリパスの自動設定
  - CUDA_PATH検出と環境変数設定
  - WHISPER_CUBLAS=1時の自動ライブラリリンク

- **`src/whisper.rs`**: 詳細なGPU初期化ログと状態管理
  - GPU設定情報の詳細出力（src/whisper.rs:50-85）
  - GPU初期化成功/失敗の明確化（src/whisper.rs:88-117）
  - 推論時のGPU使用状態表示（src/whisper.rs:208-224）

#### 🌐 API拡張

- **`src/handlers.rs`**: `/gpu-status` エンドポイント追加
  - GPU使用状態の詳細情報を提供
  - 環境変数・ライブラリ検出・推奨事項を表示

- **`src/main.rs`**: GPU状態確認エンドポイントをルーティングに追加

#### 🛠️ 便利ツール

- **`build.sh`**: GPU/CPU対応の自動ビルドスクリプト
- **`run.sh`**: GPU/CPU対応の自動実行スクリプト
- **`test_gpu.sh`**: GPU機能テストスイート
- **`tests/gpu_test.rs`**: 単体テスト・ベンチマーク・環境診断

### 🚀 主な改善点

#### 1. 詳細なGPU状態ログ

起動時に以下が表示されます：

```rust
// src/whisper.rs:50-85 での実装
println!("=== GPU設定情報 ===");
println!("設定でGPU有効化: {}", config.whisper.enable_gpu);
println!("WhisperContextParameters.use_gpu: {}", ctx_params.use_gpu);
// 環境変数・フィーチャーフラグ確認
```

#### 2. 自動フォールバック機能

```rust
// src/whisper.rs:88-117 での実装
let (context, gpu_actually_enabled) = match WhisperContext::new_with_params(model_path, ctx_params) {
    Ok(ctx) => {
        println!("✓ GPU対応のWhisperコンテキストの初期化に成功しました");
        (ctx, config.whisper.enable_gpu)
    },
    Err(e) => {
        eprintln!("⚠ GPU初期化に失敗しました。CPUで再試行します: {}", e);
        // CPU mode fallback
    }
};
```

#### 3. リアルタイム推論状態表示

```rust
// src/whisper.rs:208-224 での実装
if self.enable_gpu {
    println!("🚀 GPU使用で文字起こしを開始します...");
} else {
    println!("🖥️  CPU使用で文字起こしを開始します...");
}
// 処理時間とGPU/CPU使用状況を表示
```

#### 4. API経由での診断機能

```rust
// src/handlers.rs:445-485 での実装
pub async fn get_gpu_status(State(state): State<AppState>) -> ApiResult<Json<GpuStatusResponse>> {
    // GPU実際の使用状況
    // 環境変数確認
    // ライブラリ検出
    // 推奨事項生成
}
```

### 🔄 従来との違い

| 項目 | 従来 | 修正後 |
|------|------|---------|
| GPU設定確認 | 不明確 | 詳細ログ出力 |
| 初期化失敗 | 不明確なエラー | 自動フォールバック |
| 推論時状態 | 表示なし | GPU/CPU使用明示 |
| 診断機能 | なし | API・テストツール提供 |
| ビルド支援 | 手動設定のみ | 自動化スクリプト |

この修正により、GPU使用に関する問題の特定と解決が大幅に容易になりました。

---

## ライセンス

- 本リポジトリのライセンスに従います。

```
NVCC_PREPEND_FLAGS="--std=c++14 -Wno-deprecated-gpu-targets -U_GNU_SOURCE" CMAKE_CUDA_STANDARD=14 ./build.sh gpu
```
