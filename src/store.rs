use crate::models::{BakeOutput, HashId, ProjectSource};

pub mod cache;
pub mod http;

#[async_trait::async_trait]
pub trait Store {
    async fn get_chunk_zst(&self, chunk_id: HashId)
    -> Result<Option<axum::body::Body>, StoreError>;

    async fn get_artifact_bar_zst(
        &self,
        artifact_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError>;

    async fn get_project_source(
        &self,
        project_id: HashId,
    ) -> Result<Option<ProjectSource>, StoreError>;

    async fn get_bake_output(&self, recipe_id: HashId) -> Result<Option<BakeOutput>, StoreError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Url(#[from] url::ParseError),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
