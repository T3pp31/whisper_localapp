//! WhisperRealtimeAPI クレート
//!
//! このクレートは、HTTP(SSE) を用いた簡易インジェスト、gRPC 経由のASR(自動音声認識)
//! クライアント/サーバ、音声前処理（フレーム化・リサンプル・正規化）および
//! 設定読み込みをまとめたライブラリです。
//!
//! 主なモジュール:
//! - `asr`: ストリーミングASRクライアント、管理、サーバ実装
//! - `audio_pipeline`: 音声フレーム化、リサンプル、正規化
//! - `config`: YAML設定の読み込みと型定義
//! - `ingest`: PCM(S16LE)のインジェストとASRへの橋渡し
//! - `http_api`: HTTPインタフェース（チャンク受付＋SSE配信）
//!
pub mod asr;
pub mod audio_pipeline;
pub mod config;
pub mod ingest;
pub mod http_api;

pub use config::ConfigSet;
