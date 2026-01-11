use std::{
    os::unix::fs::FileExt as _,
    path::{Path, PathBuf},
    sync::Arc,
    task::Poll,
};

use futures::TryStreamExt as _;

/// The dividend for calculating the weight from the bytes for cache entries.
///
/// This is required because `moka` currently requires the weigher for cache
/// entry to return a `u32` value. If we used the number of bytes as the weight
/// directly, this would limit the max file size to 4 GiB. So we divide the
/// bytes by a factor so we can represent larger files. Also, this can help
/// approximate filesystem overhead.
const BYTES_PER_CACHE_WEIGHT: u64 = 4096;

use crate::{
    models::{BakeOutput, HashId, ProjectSource},
    store::{Store, StoreError},
};

pub struct CacheStore<S> {
    store: S,
    cache_dir: Arc<PathBuf>,
    cached_resources: moka::future::Cache<CacheResourceId, Arc<CacheFile>>,
    cached_project_sources: moka::future::Cache<HashId, ProjectSource>,
    cached_bake_outputs: moka::future::Cache<HashId, BakeOutput>,
}

impl<S> CacheStore<S> {
    pub fn new(store: S, cache_dir: &Path) -> Self {
        Self {
            store,
            cache_dir: Arc::new(cache_dir.to_path_buf()),
            cached_resources: moka::future::Cache::builder()
                .weigher(|_, file: &Arc<CacheFile>| {
                    // Divide the file size by `BYTES_PER_CACHE_WEIGHT` so we
                    // can measure larger file sizes in a `u32`
                    u32::try_from(file.size.div_ceil(BYTES_PER_CACHE_WEIGHT))
                        .expect("file size too large to weigh in cache")
                })
                .max_capacity(
                    (1024 * 1024 * 1024) / BYTES_PER_CACHE_WEIGHT, /* 1 GiB */
                )
                .build(),
            cached_project_sources: moka::future::Cache::builder().max_capacity(10_000).build(),
            cached_bake_outputs: moka::future::Cache::builder().max_capacity(10_000).build(),
        }
    }
}

#[async_trait::async_trait]
impl<S> Store for CacheStore<S>
where
    S: Store + Send + Sync,
{
    async fn get_chunk_zst(
        &self,
        chunk_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError> {
        let result = self
            .cached_resources
            .entry(CacheResourceId::Chunk { chunk_id })
            .or_try_insert_with(async {
                let content = match self.store.get_chunk_zst(chunk_id).await {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        return Err(None);
                    }
                    Err(err) => {
                        return Err(Some(Arc::new(err)));
                    }
                };
                let content = content.into_data_stream().map_err(std::io::Error::other);
                let content = tokio_util::io::StreamReader::new(content);
                let file = match create_cache_file(&self.cache_dir, content).await {
                    Ok(file) => file,
                    Err(err) => {
                        return Err(Some(Arc::new(err)));
                    }
                };
                Ok(Arc::new(file))
            })
            .await;
        let cache_file = match result {
            Ok(file) => file.into_value(),
            Err(err) => {
                match &*err {
                    Some(err) => {
                        return Err(StoreError::NestedArc(err.clone()));
                    }
                    None => {
                        return Ok(None);
                    }
                };
            }
        };
        let body = CacheFileBody {
            file: Some(cache_file.file.try_clone().await?.into_std().await),
            offset: 0,
            task: None,
            cache_file,
        };
        Ok(Some(axum::body::Body::new(body)))
    }

    async fn get_artifact_bar_zst(
        &self,
        artifact_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError> {
        let result = self
            .cached_resources
            .entry(CacheResourceId::Artifact { artifact_id })
            .or_try_insert_with(async {
                let content = match self.store.get_artifact_bar_zst(artifact_id).await {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        return Err(None);
                    }
                    Err(err) => {
                        return Err(Some(Arc::new(err)));
                    }
                };
                let content = content.into_data_stream().map_err(std::io::Error::other);
                let content = tokio_util::io::StreamReader::new(content);
                let file = match create_cache_file(&self.cache_dir, content).await {
                    Ok(file) => file,
                    Err(err) => {
                        return Err(Some(Arc::new(err)));
                    }
                };
                Ok(Arc::new(file))
            })
            .await;
        let cache_file = match result {
            Ok(file) => file.into_value(),
            Err(err) => {
                match &*err {
                    Some(err) => {
                        return Err(StoreError::NestedArc(err.clone()));
                    }
                    None => {
                        return Ok(None);
                    }
                };
            }
        };
        let body = CacheFileBody {
            file: Some(cache_file.file.try_clone().await?.into_std().await),
            offset: 0,
            task: None,
            cache_file,
        };
        Ok(Some(axum::body::Body::new(body)))
    }

    async fn get_project_source(
        &self,
        project_id: HashId,
    ) -> Result<Option<ProjectSource>, StoreError> {
        let result = self
            .cached_project_sources
            .entry(project_id)
            .or_try_insert_with(async {
                let project_source = self.store.get_project_source(project_id).await;
                match project_source {
                    Ok(Some(content)) => Ok(content),
                    Ok(None) => Err(None),
                    Err(err) => Err(Some(Arc::new(err))),
                }
            })
            .await;
        match result {
            Ok(project_source) => Ok(Some(project_source.into_value())),
            Err(err) => match &*err {
                Some(err) => Err(StoreError::NestedArc(err.clone())),
                None => Ok(None),
            },
        }
    }

    async fn get_bake_output(&self, recipe_id: HashId) -> Result<Option<BakeOutput>, StoreError> {
        let result = self
            .cached_bake_outputs
            .entry(recipe_id)
            .or_try_insert_with(async {
                let bake_output = self.store.get_bake_output(recipe_id).await;
                match bake_output {
                    Ok(Some(content)) => Ok(content),
                    Ok(None) => Err(None),
                    Err(err) => Err(Some(Arc::new(err))),
                }
            })
            .await;
        match result {
            Ok(bake_output) => Ok(Some(bake_output.into_value())),
            Err(err) => match &*err {
                Some(err) => Err(StoreError::NestedArc(err.clone())),
                None => Ok(None),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CacheResourceId {
    Chunk { chunk_id: HashId },
    Artifact { artifact_id: HashId },
}

async fn create_cache_file(
    cache_dir: &Path,
    mut data: impl tokio::io::AsyncBufRead + Unpin,
) -> Result<CacheFile, StoreError> {
    let id = uuid::Uuid::new_v4();
    let path = cache_dir.join(format!("cache-file-{id}"));

    let mut file = tokio::fs::File::create_new(&path).await?;
    tokio::fs::remove_file(path).await?;

    let size = tokio::io::copy(&mut data, &mut file).await?;

    tracing::info!(%id, size, "created cache file");

    Ok(CacheFile { file, size })
}

struct CacheFile {
    file: tokio::fs::File,
    size: u64,
}

pin_project_lite::pin_project! {
    struct CacheFileBody {
        #[pin]
        file: Option<std::fs::File>,
        offset: u64,
        #[pin]
        task: Option<tokio::task::JoinHandle<anyhow::Result<(bytes::Bytes, std::fs::File)>>>,
        cache_file: Arc<CacheFile>,
    }
}

impl http_body::Body for CacheFileBody {
    type Data = bytes::Bytes;

    type Error = anyhow::Error;

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        let mut this = self.project();
        loop {
            if let Some(task) = this.task.as_mut().as_pin_mut() {
                let result = std::task::ready!(task.poll(cx));
                this.task.as_mut().take();

                match result {
                    Ok(Ok((bytes, file))) => {
                        let read_len: u64 = bytes.len().try_into().unwrap();

                        *this.file = Some(file);
                        *this.offset = this.offset.checked_add(read_len).unwrap();

                        return Poll::Ready(Some(Ok(http_body::Frame::data(bytes))));
                    }
                    Err(error) => {
                        return Poll::Ready(Some(Err(error.into())));
                    }
                    Ok(Err(error)) => {
                        return Poll::Ready(Some(Err(error)));
                    }
                }
            } else {
                let Some(file) = this.file.take() else {
                    return Poll::Ready(Some(Err(anyhow::anyhow!("file handle dropped"))));
                };

                let offset = *this.offset;
                *this.task = Some(tokio::task::spawn_blocking(move || {
                    let mut buf = [0; 65_536];
                    let len = file.read_at(&mut buf, offset)?;
                    let bytes = bytes::Bytes::copy_from_slice(&buf[..len]);

                    anyhow::Ok((bytes, file))
                }));
            }
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        http_body::SizeHint::with_exact(self.cache_file.size)
    }
}
