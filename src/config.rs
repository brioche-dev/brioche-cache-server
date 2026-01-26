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
    #[serde(default = "default_cache_dir")]
    pub dir: PathBuf,

    /// Max size of all temporary cache files, in bytes.
    #[serde(default = "default_max_disk_capacity")]
    pub max_disk_capacity: bytesize::ByteSize,

    /// Max project sources to keep in memory.
    #[serde(default = "default_max_project_sources")]
    pub max_project_sources: usize,

    /// Max bake outputs to keep in memory.
    #[serde(default = "default_max_bake_outputs")]
    pub max_bake_outputs: usize,
}

fn default_bind_address() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_bind_metrics_address() -> String {
    "0.0.0.0:3001".to_string()
}

fn default_cache_dir() -> PathBuf {
    std::env::temp_dir()
}

fn default_max_disk_capacity() -> bytesize::ByteSize {
    bytesize::ByteSize::gb(1)
}

fn default_max_project_sources() -> usize {
    10_000
}

fn default_max_bake_outputs() -> usize {
    10_000
}
