use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct ConnectionProfile {
    pub max_bitrate_kbps: u32,
    pub latency_budget_ms: u32,
    pub stream_kinds: HashSet<StreamKind>,
}

impl ConnectionProfile {
    pub fn new(
        max_bitrate_kbps: u32,
        latency_budget_ms: u32,
        stream_kinds: impl IntoIterator<Item = StreamKind>,
    ) -> Self {
        Self {
            max_bitrate_kbps,
            latency_budget_ms,
            stream_kinds: stream_kinds.into_iter().collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKind {
    Audio,
    PartialTranscript,
    FinalTranscript,
    Control,
}
