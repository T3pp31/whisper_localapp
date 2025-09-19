#!/bin/bash

# Whisperモデルダウンロードスクリプト

MODEL_DIR="models"
BASE_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"

# モデルディレクトリを作成
mkdir -p $MODEL_DIR

echo "Whisperモデルをダウンロードしています..."

# tiny モデル (~39MB)
echo "tinyモデルをダウンロード中..."
wget -O $MODEL_DIR/ggml-tiny.bin $BASE_URL/ggml-tiny.bin

# base モデル (~74MB) - 推奨
echo "baseモデルをダウンロード中..."
wget -O $MODEL_DIR/ggml-base.bin $BASE_URL/ggml-base.bin

# small モデル (~244MB)
echo "smallモデルをダウンロード中..."
wget -O $MODEL_DIR/ggml-small.bin $BASE_URL/ggml-small.bin

echo "モデルのダウンロードが完了しました。"
echo "ダウンロードされたモデル:"
ls -lh $MODEL_DIR/*.bin