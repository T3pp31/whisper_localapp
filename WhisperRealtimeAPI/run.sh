#!/usr/bin/env bash
set -Eeuo pipefail

# WhisperRealtimeAPI launcher
# - 設定は YAML から読み取り（env で上書き可）
# - ASR gRPC サーバと WebSocket シグナリングサーバを起動
# - ログ: logs/*.log

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ROOT_DIR="$SCRIPT_DIR"

# 設定ディレクトリ（デフォルト: ./config）
CONFIG_DIR="${WHISPER_REALTIME_CONFIG_DIR:-${ROOT_DIR}/config}"
export WHISPER_REALTIME_CONFIG_DIR="$CONFIG_DIR"

# ログレベル（tracing-subscriber の EnvFilter 用）
export RUST_LOG="${RUST_LOG:-info}"

# ビルドプロファイル: debug/release（デフォルト: debug）
RUN_PROFILE="${RUN_PROFILE:-debug}"
if [[ "$RUN_PROFILE" == "release" ]]; then
  CARGO_PROFILE_FLAG="--release"
else
  CARGO_PROFILE_FLAG=""
fi

LOG_DIR="$ROOT_DIR/logs"
mkdir -p "$LOG_DIR"

die() { echo "[run.sh] error: $*" >&2; exit 1; }
warn() { echo "[run.sh] warn: $*" >&2; }
info() { echo "[run.sh] info: $*" >&2; }

require_file() { [[ -f "$1" ]] || die "missing file: $1"; }

# YAML の単純キーを抽出（key: "value" または key: value を想定）
parse_yaml_value() {
  local file="$1" key="$2"
  awk -v k="$key" '
    $1 ~ "^"k":" {
      sub("^"k":[ ]*","",$0); gsub("\"","",$0); gsub("\r","",$0); print $0; exit 0
    }
  ' "$file" | xargs || true
}

normalize_host_for_connect() {
  local host="$1"
  case "$host" in
    0.0.0.0) echo "127.0.0.1" ;;
    *) echo "$host" ;;
  esac
}

wait_for_port() {
  local host="$1" port="$2" timeout_sec="${3:-30}" start_ts now_ts
  start_ts=$(date +%s)
  while true; do
    if (echo > "/dev/tcp/${host}/${port}") >/dev/null 2>&1; then
      return 0
    fi
    now_ts=$(date +%s)
    if (( now_ts - start_ts >= timeout_sec )); then
      return 1
    fi
    sleep 0.2
  done
}

# 前提ファイル確認
require_file "$CONFIG_DIR/server.yaml"
require_file "$CONFIG_DIR/asr_pipeline.yaml"
require_file "$CONFIG_DIR/whisper_model.yaml"

ASR_ADDR="$(parse_yaml_value "$CONFIG_DIR/server.yaml" asr_grpc_bind_addr)"
WS_ADDR="$(parse_yaml_value "$CONFIG_DIR/server.yaml" ws_bind_addr)"
MODEL_PATH="$(parse_yaml_value "$CONFIG_DIR/whisper_model.yaml" model_path)"

[[ -n "$ASR_ADDR" ]] || die "asr_grpc_bind_addr not found in server.yaml"
[[ -n "$WS_ADDR" ]] || die "ws_bind_addr not found in server.yaml"
[[ -n "$MODEL_PATH" ]] || warn "model_path not found in whisper_model.yaml"

# モデルファイルの存在チェック（相対/絶対の両方を許容）
if [[ -n "$MODEL_PATH" ]]; then
  if [[ -f "$MODEL_PATH" ]]; then
    :
  elif [[ -f "$ROOT_DIR/$MODEL_PATH" ]]; then
    MODEL_PATH="$ROOT_DIR/$MODEL_PATH"
  else
    warn "model file not found: $MODEL_PATH (ASRはダミーで起動する可能性があります)"
  fi
fi

info "Using config dir: $CONFIG_DIR"
info "ASR gRPC bind: $ASR_ADDR"
info "WS bind: $WS_ADDR"
info "Run profile: $RUN_PROFILE"

# ビルド
info "Building project (cargo build $CARGO_PROFILE_FLAG)"
cargo build $CARGO_PROFILE_FLAG >/dev/null 2>&1 || die "cargo build failed"

# クリーンアップ（子プロセス停止）
ASR_PID=""; API_PID=""
cleanup() {
  local ec=$?
  if [[ -n "$API_PID" ]] && kill -0 "$API_PID" 2>/dev/null; then
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
  fi
  if [[ -n "$ASR_PID" ]] && kill -0 "$ASR_PID" 2>/dev/null; then
    kill "$ASR_PID" 2>/dev/null || true
    wait "$ASR_PID" 2>/dev/null || true
  fi
  exit "$ec"
}
trap cleanup INT TERM EXIT

# アドレス分解（host:port）
ASR_HOST="${ASR_ADDR%:*}"; ASR_PORT="${ASR_ADDR##*:}"
WS_HOST="${WS_ADDR%:*}"; WS_PORT="${WS_ADDR##*:}"
ASR_HOST_CONN="$(normalize_host_for_connect "$ASR_HOST")"
WS_HOST_CONN="$(normalize_host_for_connect "$WS_HOST")"

# ASR gRPC サーバ起動
info "Starting ASR gRPC server ..."
(
  cd "$ROOT_DIR"
  RUST_LOG="$RUST_LOG" cargo run $CARGO_PROFILE_FLAG --bin asr_server >> "$LOG_DIR/asr_server.log" 2>&1
) &
ASR_PID=$!

if ! wait_for_port "$ASR_HOST_CONN" "$ASR_PORT" 30; then
  warn "ASR gRPC server did not open ${ASR_HOST}:${ASR_PORT} within timeout"
else
  info "ASR gRPC server is listening on ${ASR_ADDR}"
fi

# バックエンド（WebSocket シグナリング）起動
info "Starting backend (WebSocket signaling) ..."
(
  cd "$ROOT_DIR"
  # メインのバックエンドは `whisper_realtime_api` バイナリ
  RUST_LOG="$RUST_LOG" cargo run $CARGO_PROFILE_FLAG --bin whisper_realtime_api >> "$LOG_DIR/backend.log" 2>&1
) &
API_PID=$!

if ! wait_for_port "$WS_HOST_CONN" "$WS_PORT" 30; then
  warn "WebSocket server did not open ${WS_HOST}:${WS_PORT} within timeout"
else
  info "WebSocket server is listening on ${WS_ADDR}"
fi

echo ""
echo "============================================================"
echo "  WhisperRealtimeAPI is up"
echo "  - ASR gRPC:   ${ASR_ADDR}"
echo "  - WebSocket:  ws://${WS_ADDR}/ws?session_id=<your-id>"
echo "  Logs:         ${LOG_DIR}/asr_server.log, ${LOG_DIR}/backend.log"
echo "  Stop:         Ctrl-C (both processes will be terminated)"
echo "============================================================"
echo ""

# どちらかが終了したらもう片方も停止
if command -v wait >/dev/null 2>&1; then
  if wait -n "$ASR_PID" "$API_PID" 2>/dev/null; then
    :
  else
    :
  fi
else
  wait "$ASR_PID" "$API_PID" || true
fi

info "A child process exited; shutting down the remaining service"
exit 1
