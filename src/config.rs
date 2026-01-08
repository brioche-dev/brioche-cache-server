use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    pub upstream_store_url: url::Url,

    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
}

fn default_bind_address() -> String {
    "0.0.0.0:3000".to_string()
}

fn default_cache_dir() -> PathBuf {
    std::env::temp_dir()
}
