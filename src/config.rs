use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    /// The address the cache server listens on.
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// The address the metrics for the cache server listens on.
    #[serde(default = "default_bind_metrics_address")]
    pub bind_metrics_address: String,

    /// The upstream cache store this cache server sits in front of.
    pub upstream_store_url: url::Url,

    /// Configuration for the caching behavior.
    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheConfig {
    /// Directory used to store temporary cache files (files are unlinked after
    /// creation, so this is not used for persistence). Defaults to the system's
    /// temporary directory, e.g. `/tmp`.
    pub dir: Option<PathBuf>,

    /// Max size of all temporary cache files, in bytes.
    pub max_disk_capacity: Option<bytesize::ByteSize>,

    /// Max project sources to keep in memory.
    pub max_project_sources: Option<usize>,

    /// Max bake outputs to keep in memory.
    pub max_bake_outputs: Option<usize>,
}

fn default_bind_address() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_bind_metrics_address() -> String {
    "0.0.0.0:3001".to_string()
}
