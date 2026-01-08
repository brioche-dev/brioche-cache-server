use futures::StreamExt as _;

use crate::{
    models::{BakeOutput, HashId, ProjectSource},
    store::{Store, StoreError},
};

pub struct HttpStore {
    reqwest: reqwest::Client,
    url: url::Url,
}

impl HttpStore {
    pub fn new(url: url::Url) -> Self {
        Self {
            reqwest: reqwest::Client::new(),
            url,
        }
    }
}

#[async_trait::async_trait]
impl Store for HttpStore {
    async fn get_chunk_zst(
        &self,
        chunk_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError> {
        let url = self.url.join(&format!("chunks/{chunk_id}.zst"))?;

        let response = self.reqwest.get(url).send().await?;
        if matches!(response.status(), reqwest::StatusCode::NOT_FOUND) {
            return Ok(None);
        }

        let response = response.error_for_status()?;
        Ok(Some(axum_body_from_reqwest_response(response)))
    }

    async fn get_artifact_bar_zst(
        &self,
        artifact_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError> {
        let url = self.url.join(&format!("artifacts/{artifact_id}.bar.zst"))?;

        let response = self.reqwest.get(url).send().await?;
        if matches!(response.status(), reqwest::StatusCode::NOT_FOUND) {
            return Ok(None);
        }

        let response = response.error_for_status()?;
        Ok(Some(axum_body_from_reqwest_response(response)))
    }

    async fn get_project_source(
        &self,
        project_id: HashId,
    ) -> Result<Option<ProjectSource>, StoreError> {
        let url = self
            .url
            .join(&format!("projects/{project_id}/source.json"))?;

        let response = self.reqwest.get(url).send().await?;
        if matches!(response.status(), reqwest::StatusCode::NOT_FOUND) {
            return Ok(None);
        }

        let project_source = response.error_for_status()?.json().await?;
        Ok(project_source)
    }

    async fn get_bake_output(&self, recipe_id: HashId) -> Result<Option<BakeOutput>, StoreError> {
        let url = self.url.join(&format!("bakes/{recipe_id}/output.json"))?;

        let response = self.reqwest.get(url).send().await?;
        if matches!(response.status(), reqwest::StatusCode::NOT_FOUND) {
            return Ok(None);
        }

        let bake_output = response.error_for_status()?.json().await?;
        Ok(bake_output)
    }
}

fn axum_body_from_reqwest_response(response: reqwest::Response) -> axum::body::Body {
    let response_size: Option<u64> = response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|content_length| Some(content_length.to_str().ok()?.parse().ok()?));
    let response_stream = response
        .bytes_stream()
        .map(|bytes| bytes.map_err(StoreError::from));

    let size_hint = response_size.map(http_body::SizeHint::with_exact);
    let body =
        crate::response::BodyWithSize::new(crate::response::StreamBody::new(response_stream))
            .with_size_hint(size_hint);
    axum::body::Body::new(body)
}
