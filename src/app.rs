use std::sync::Arc;

use axum::{
    Json,
    body::Body,
    extract::{Path, State},
};
use reqwest::StatusCode;

use crate::{
    models::{BakeOutput, HashId, ProjectSource},
    store::HttpStore,
};

pub fn router(state: AppState) -> axum::Router {
    axum::Router::new()
        .route("/", axum::routing::get(|| async { "Hello world!" }))
        .route("/chunks/{chunk_file}", axum::routing::get(get_chunk_zst))
        .route(
            "/artifacts/{artifact_filename}",
            axum::routing::get(get_artifact_bar_zst),
        )
        .route(
            "/projects/{project_id}/source.json",
            axum::routing::get(get_project_source),
        )
        .route(
            "/bakes/{recipe_id}/output.json",
            axum::routing::get(get_bake_output),
        )
        .with_state(state)
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<HttpStore>,
}

async fn get_chunk_zst(
    State(state): State<AppState>,
    Path(filename): Path<ChunkFilename>,
) -> Result<Body, AppError> {
    let ChunkFilename::Zst { chunk_id } = filename;
    let body = state
        .store
        .get_chunk_zst(chunk_id)
        .await?
        .ok_or_else(|| ResourceNotFound::Chunk { chunk_id })?;
    Ok(body)
}

async fn get_artifact_bar_zst(
    State(state): State<AppState>,
    Path(filename): Path<ArtifactFilename>,
) -> Result<Body, AppError> {
    let ArtifactFilename::BarZst { artifact_id } = filename;
    let body = state
        .store
        .get_artifact_bar_zst(artifact_id)
        .await?
        .ok_or_else(|| ResourceNotFound::Artifact { artifact_id })?;
    Ok(body)
}

async fn get_project_source(
    State(state): State<AppState>,
    Path(project_id): Path<HashId>,
) -> Result<Json<ProjectSource>, AppError> {
    let project_source = state
        .store
        .get_project_source(project_id)
        .await?
        .ok_or_else(|| ResourceNotFound::ProjectSource { project_id })?;
    Ok(Json(project_source))
}

async fn get_bake_output(
    State(state): State<AppState>,
    Path(recipe_id): Path<HashId>,
) -> Result<Json<BakeOutput>, AppError> {
    let bake_output = state
        .store
        .get_bake_output(recipe_id)
        .await?
        .ok_or_else(|| ResourceNotFound::BakeOutput { recipe_id })?;
    Ok(Json(bake_output))
}

#[derive(Debug, Clone)]
enum ChunkFilename {
    Zst { chunk_id: HashId },
}

impl std::fmt::Display for ChunkFilename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zst { chunk_id } => write!(f, "{chunk_id}.zst"),
        }
    }
}

impl std::str::FromStr for ChunkFilename {
    type Err = miette::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(chunk_id) = s.strip_suffix(".zst") {
            let chunk_id = chunk_id.parse()?;
            Ok(Self::Zst { chunk_id })
        } else {
            miette::bail!("invalid chunk filename: {s}");
        }
    }
}

impl serde::Serialize for ChunkFilename {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.to_string();
        s.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ChunkFilename {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone)]
enum ArtifactFilename {
    BarZst { artifact_id: HashId },
}

impl std::fmt::Display for ArtifactFilename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BarZst { artifact_id } => write!(f, "{artifact_id}.bar.zst"),
        }
    }
}

impl std::str::FromStr for ArtifactFilename {
    type Err = miette::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(artifact_id) = s.strip_suffix(".bar.zst") {
            let artifact_id = artifact_id.parse()?;
            Ok(Self::BarZst { artifact_id })
        } else {
            miette::bail!("invalid artifact filename: {s}");
        }
    }
}

impl serde::Serialize for ArtifactFilename {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = self.to_string();
        s.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for ArtifactFilename {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
enum AppError {
    #[error("error from store")]
    Store(#[from] crate::store::StoreError),

    #[error(transparent)]
    NotFound(#[from] ResourceNotFound),
}

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::Store(error) => {
                tracing::warn!("store error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("store error: {error}"),
                )
            }
            Self::NotFound(not_found) => {
                tracing::info!("{not_found}");
                (StatusCode::NOT_FOUND, not_found.to_string())
            }
        };

        (status, Json(JsonError { message })).into_response()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct JsonError {
    message: String,
}

#[derive(Debug, thiserror::Error)]
enum ResourceNotFound {
    #[error("chunk not found in store: {chunk_id}")]
    Chunk { chunk_id: HashId },

    #[error("artifact not found in store: {artifact_id}")]
    Artifact { artifact_id: HashId },

    #[error("project source not found in store: {project_id}")]
    ProjectSource { project_id: HashId },

    #[error("recipe bake output not found in store: {recipe_id}")]
    BakeOutput { recipe_id: HashId },
}
