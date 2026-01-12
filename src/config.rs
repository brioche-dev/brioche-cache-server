use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    #[serde(default = "default_bind_metrics_address")]
    pub bind_metrics_address: String,

    pub upstream_store_url: url::Url,

    #[serde(default)]
    pub cache: CacheConfig,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct CacheConfig {
    pub dir: Option<PathBuf>,
    pub max_disk_capacity: Option<bytesize::ByteSize>,
    pub max_project_sources: Option<u64>,
    pub max_bake_outputs: Option<u64>,
}

fn default_bind_address() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_bind_metrics_address() -> String {
    "0.0.0.0:3001".to_string()
}
