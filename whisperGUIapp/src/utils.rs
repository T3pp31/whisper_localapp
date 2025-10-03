//! 共通ユーティリティ関数/定数。
//! - URL 組み立て
//! - 音声拡張子の判定
//! - 小さな純粋関数群（テスト容易性のために分離）

use std::path::Path;

/// リモートアップロードで許可する拡張子（小文字）。
pub const REMOTE_ALLOWED_EXTS: &[&str] = &["wav", "mp3", "m4a", "flac", "ogg"];

/// `base_url` と `endpoint` から完全な URL を生成する。
/// - `endpoint` が `http://` または `https://` で始まる場合はそのまま返す
/// - そうでない場合は `base_url` と連結（スラッシュを重複/欠落なく整形）
pub fn build_full_url(base_url: &str, endpoint: &str) -> String {
    let ep = endpoint.trim();
    if ep.starts_with("http://") || ep.starts_with("https://") {
        return ep.to_string();
    }
    let base = base_url.trim().trim_end_matches('/');
    let ep_norm = if ep.starts_with('/') { ep.to_string() } else { format!("/{}", ep) };
    format!("{}{}", base, ep_norm)
}

/// 拡張子がリモート許可済みかを判定。
pub fn is_allowed_audio_ext<P: AsRef<Path>>(path: P) -> bool {
    let p = path.as_ref();
    p.extension()
        .and_then(|s| s.to_str())
        .map(|s| REMOTE_ALLOWED_EXTS.contains(&s.to_lowercase().as_str()))
        .unwrap_or(false)
}

