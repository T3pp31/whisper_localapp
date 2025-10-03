//! システム要件・対応クライアント・ネットワーク要件などの設定
use serde::Deserialize;
use version_compare::Version;

#[derive(Debug, Clone, Deserialize)]
pub struct SystemRequirements {
    pub supported_clients: SupportedClients,
    pub network: NetworkRequirements,
    pub signaling: SignalingParameters,
    pub resources: ResourceRequirements,
    pub token: TokenConfig,
}

impl SystemRequirements {
    /// ブラウザ名/バージョンが最小要件を満たすか
    pub fn is_browser_supported(&self, name: &str, version: &str) -> bool {
        self.supported_clients
            .browsers
            .iter()
            .filter(|client| client.name.eq_ignore_ascii_case(name))
            .any(|client| version_meets(&client.min_version, version))
    }

    /// モバイルOS/バージョンが最小要件を満たすか
    pub fn is_mobile_supported(&self, os: &str, version: &str) -> bool {
        self.supported_clients
            .mobile
            .iter()
            .filter(|client| client.os.eq_ignore_ascii_case(os))
            .any(|client| version_meets(&client.min_version, version))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SupportedClients {
    pub browsers: Vec<BrowserClient>,
    pub mobile: Vec<MobileClient>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrowserClient {
    pub name: String,
    pub min_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MobileClient {
    pub os: String,
    pub min_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NetworkRequirements {
    pub max_bandwidth_mbps: u32,
    pub preferred_codecs: CodecPreferences,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CodecPreferences {
    pub audio: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SignalingParameters {
    pub default_bitrate_kbps: u32,
    pub ice_servers: Vec<IceServerConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IceServerConfig {
    pub urls: Vec<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceRequirements {
    pub max_concurrent_sessions: u32,
    pub session_timeout_s: u64,
    pub gpu: GpuRequirement,
    pub cpu: CpuRequirement,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GpuRequirement {
    pub model: String,
    pub count: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CpuRequirement {
    pub cores: u32,
    pub min_clock_ghz: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenConfig {
    pub issuer: String,
    pub jwks_url: String,
    pub audience: String,
}

/// バージョン文字列が最小要件以上か（`version_compare` が解釈できない場合は文字列比較）
fn version_meets(min_required: &str, current: &str) -> bool {
    match (Version::from(min_required), Version::from(current)) {
        (Some(min_v), Some(cur_v)) => cur_v >= min_v,
        _ => current >= min_required,
    }
}
