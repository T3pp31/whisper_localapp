#!/usr/bin/env bash
set -Eeuo pipefail

# WhisperRealtimeAPI launcher
# - 設定は YAML を基本とし、必要に応じて環境変数で上書き（run.sh側でオーバーレイ作成）
# - ASR gRPC サーバと HTTP(SSE) バックエンドを起動
# - ログ: logs/*.log

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ROOT_DIR="$SCRIPT_DIR"

# 引数処理（現状は --dry-run のみ対応）
DRY_RUN=0
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
  esac
done

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

port_in_use() {
  local host="$1" port="$2"
  (echo > "/dev/tcp/${host}/${port}") >/dev/null 2>&1
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
HTTP_ADDR="$(parse_yaml_value "$CONFIG_DIR/server.yaml" http_bind_addr)"
MODEL_PATH="$(parse_yaml_value "$CONFIG_DIR/whisper_model.yaml" model_path)"

[[ -n "$ASR_ADDR" ]] || die "asr_grpc_bind_addr not found in server.yaml"
[[ -n "$HTTP_ADDR" ]] || die "http_bind_addr not found in server.yaml"
[[ -n "$MODEL_PATH" ]] || warn "model_path not found in whisper_model.yaml"

# 環境変数による上書き（READMEに記載の通り）
if [[ -n "${ASR_GRPC_BIND_ADDR:-}" ]]; then
  ASR_ADDR="$ASR_GRPC_BIND_ADDR"
fi
if [[ -n "${HTTP_BIND_ADDR:-}" ]]; then
  HTTP_ADDR="$HTTP_BIND_ADDR"
fi

# dry-run の場合は計算結果だけ出力して終了（ビルドや起動はしない）
if (( DRY_RUN == 1 )); then
  echo "ASR_ADDR=${ASR_ADDR}"
  echo "HTTP_ADDR=${HTTP_ADDR}"
  echo "CONFIG_DIR=${CONFIG_DIR}"
  echo "RUN_PROFILE=${RUN_PROFILE}"
  echo "RUST_LOG=${RUST_LOG}"
  exit 0
fi

info "Using config dir: $CONFIG_DIR"
info "ASR gRPC bind: $ASR_ADDR"
info "HTTP bind: $HTTP_ADDR"
info "Run profile: $RUN_PROFILE"

# 起動前にポート占有チェック（すでにLISTENしているなら中断）
ASR_HOST="${ASR_ADDR%:*}"; ASR_PORT="${ASR_ADDR##*:}"
HTTP_HOST="${HTTP_ADDR%:*}"; HTTP_PORT="${HTTP_ADDR##*:}"
ASR_HOST_CONN="$(normalize_host_for_connect "$ASR_HOST")"
HTTP_HOST_CONN="$(normalize_host_for_connect "$HTTP_HOST")"

if port_in_use "$ASR_HOST_CONN" "$ASR_PORT"; then
  die "ASR port already in use: ${ASR_HOST}:${ASR_PORT}"
fi
if port_in_use "$HTTP_HOST_CONN" "$HTTP_PORT"; then
  die "HTTP port already in use: ${HTTP_HOST}:${HTTP_PORT}"
fi

# 設定オーバーレイ（環境変数で上書きされた場合、server.yaml と asr_pipeline.yaml を一時ディレクトリで差し替え）
LAUNCH_CONFIG_DIR="$CONFIG_DIR"
OVERLAY_DIR=""
if [[ -n "${ASR_GRPC_BIND_ADDR:-}" || -n "${HTTP_BIND_ADDR:-}" ]]; then
  OVERLAY_DIR="$LOG_DIR/run_cfg_overlay_$(date +%s%N)"
  mkdir -p "$OVERLAY_DIR"
  cp -a "$CONFIG_DIR"/*.yaml "$OVERLAY_DIR"/
  # server.yaml の置換
  if [[ -n "${ASR_GRPC_BIND_ADDR:-}" ]]; then
    awk -v v="$ASR_ADDR" 'BEGIN{p=0} { if($1=="asr_grpc_bind_addr:") { print "asr_grpc_bind_addr: \""v"\""; p=1 } else print } END{ if(p==0) print "asr_grpc_bind_addr: \""v"\"" }' "$OVERLAY_DIR/server.yaml" > "$OVERLAY_DIR/server.yaml.tmp" && mv "$OVERLAY_DIR/server.yaml.tmp" "$OVERLAY_DIR/server.yaml"
    # asr_pipeline.yaml の service.endpoint も合わせる（httpスキーム前提）
    awk -v v="$ASR_ADDR" 'BEGIN{p=0} { if($1=="endpoint:") { print "endpoint: \"http://"v"\""; p=1 } else print } END{ if(p==0) print "endpoint: \"http://"v"\"" }' "$OVERLAY_DIR/asr_pipeline.yaml" > "$OVERLAY_DIR/asr_pipeline.yaml.tmp" && mv "$OVERLAY_DIR/asr_pipeline.yaml.tmp" "$OVERLAY_DIR/asr_pipeline.yaml"
  fi
  if [[ -n "${HTTP_BIND_ADDR:-}" ]]; then
    awk -v v="$HTTP_ADDR" 'BEGIN{p=0} { if($1=="http_bind_addr:") { print "http_bind_addr: \""v"\""; p=1 } else print } END{ if(p==0) print "http_bind_addr: \""v"\"" }' "$OVERLAY_DIR/server.yaml" > "$OVERLAY_DIR/server.yaml.tmp" && mv "$OVERLAY_DIR/server.yaml.tmp" "$OVERLAY_DIR/server.yaml"
  fi
  LAUNCH_CONFIG_DIR="$OVERLAY_DIR"
fi
export WHISPER_REALTIME_CONFIG_DIR="$LAUNCH_CONFIG_DIR"

# ビルド
info "Building project (cargo build $CARGO_PROFILE_FLAG)"
cargo build $CARGO_PROFILE_FLAG >/dev/null 2>&1 || die "cargo build failed"

# クリーンアップ（子プロセス停止 + オーバーレイ削除）
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
  if [[ -n "$OVERLAY_DIR" ]] && [[ -d "$OVERLAY_DIR" ]]; then
    rm -rf "$OVERLAY_DIR" || true
  fi
  exit "$ec"
}
trap cleanup INT TERM EXIT

# ASR gRPC サーバ起動
info "Starting ASR gRPC server ..."
(
  cd "$ROOT_DIR"
  RUST_LOG="$RUST_LOG" WHISPER_REALTIME_CONFIG_DIR="$LAUNCH_CONFIG_DIR" cargo run $CARGO_PROFILE_FLAG --bin asr_server >> "$LOG_DIR/asr_server.log" 2>&1
) &
ASR_PID=$!

if ! wait_for_port "$ASR_HOST_CONN" "$ASR_PORT" 30; then
  warn "ASR gRPC server did not open ${ASR_HOST}:${ASR_PORT} within timeout"
else
  info "ASR gRPC server is listening on ${ASR_ADDR}"
fi

# バックエンド（HTTP ingest + SSE）起動
info "Starting backend (HTTP ingest + SSE) ..."
(
  cd "$ROOT_DIR"
  # メインのバックエンドは `whisper_realtime_api` バイナリ
  RUST_LOG="$RUST_LOG" WHISPER_REALTIME_CONFIG_DIR="$LAUNCH_CONFIG_DIR" cargo run $CARGO_PROFILE_FLAG --bin whisper_realtime_api >> "$LOG_DIR/backend.log" 2>&1
) &
API_PID=$!

if ! wait_for_port "$HTTP_HOST_CONN" "$HTTP_PORT" 30; then
  warn "HTTP server did not open ${HTTP_HOST}:${HTTP_PORT} within timeout"
else
  info "HTTP server is listening on ${HTTP_ADDR}"
fi

echo ""

echo "============================================================"
echo "  WhisperRealtimeAPI is up"
echo "  - ASR gRPC:   ${ASR_ADDR}"
echo "  - HTTP ingest:  POST http://${HTTP_ADDR}/http/v1/sessions/<id>/chunk"
echo "  - SSE events:   GET  http://${HTTP_ADDR}/http/v1/sessions/<id>/events"
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

