use std::{
    os::unix::fs::FileExt as _,
    path::PathBuf,
    sync::{Arc, atomic::AtomicU64},
};

use anyhow::Context as _;
use futures::TryStreamExt as _;
use lru::LruCache;
use tokio::sync::{Mutex, OnceCell};

use crate::{
    config::CacheConfig,
    models::{BakeOutput, HashId, ProjectSource},
    store::{Store, StoreError},
};

pub struct CacheStore<S> {
    store: S,
    cache_dir: Arc<PathBuf>,
    capacity_pool: CacheCapacityPool,
    cached_resources: Mutex<LruCache<CacheResourceId, Arc<OnceCell<Arc<CacheFile>>>>>,
    cached_project_sources: Mutex<LruCache<HashId, Arc<OnceCell<(ProjectSource, uuid::Uuid)>>>>,
    cached_bake_outputs: Mutex<LruCache<HashId, Arc<OnceCell<(BakeOutput, uuid::Uuid)>>>>,
}

impl<S> CacheStore<S> {
    pub fn new(store: S, config: CacheConfig) -> anyhow::Result<Self> {
        let (original_soft_limit, hard_limit) =
            rlimit::getrlimit(rlimit::Resource::NOFILE).context("failed to get rlimit")?;

        let min_soft_limit = config
            .min_non_cache_files
            .checked_add(config.min_cache_files.max(1))
            .unwrap();

        anyhow::ensure!(
            min_soft_limit <= hard_limit,
            "NOFILE hard rlimit {hard_limit} is less than the minimum limit of {min_soft_limit} in the config"
        );

        let soft_limit = if original_soft_limit < min_soft_limit {
            tracing::info!(
                original_soft_limit,
                new_soft_limit = min_soft_limit,
                hard_limit,
                min_non_cache_files = config.min_non_cache_files,
                min_cache_files = config.min_cache_files,
                "increasing NOFILE soft rlimit"
            );
            rlimit::increase_nofile_limit(min_soft_limit)
                .context("failed to increase NOFILE rlimit")?
        } else {
            original_soft_limit
        };

        let max_cache_files = soft_limit.checked_sub(config.min_non_cache_files).unwrap();
        let max_cache_files: usize = max_cache_files.try_into().unwrap();

        tracing::info!(
            cache_dir = %config.dir.display(),
            max_disk_capacity_bytes = config.max_disk_capacity.as_u64(),
            max_cache_files,
            min_non_cache_files = config.min_non_cache_files,
            min_cache_files = config.min_cache_files,
            "creating new cache store",
        );

        metrics::gauge!("cache_disk_max_bytes").set(config.max_disk_capacity.as_u64() as f64);
        metrics::gauge!("cache_disk_max_files").set(max_cache_files as f64);

        Ok(Self {
            store,
            capacity_pool: CacheCapacityPool::new(config.max_disk_capacity.as_u64()),
            cache_dir: Arc::new(config.dir),
            cached_resources: Mutex::new(LruCache::new(max_cache_files.try_into().unwrap())),
            cached_project_sources: Mutex::new(LruCache::new(
                config
                    .max_project_sources
                    .try_into()
                    .expect("max_project_sources should not be zero"),
            )),
            cached_bake_outputs: Mutex::new(LruCache::new(
                config
                    .max_bake_outputs
                    .try_into()
                    .expect("max_bake_outputs should not be zero"),
            )),
        })
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
        let init_id = uuid::Uuid::new_v4();
        let cache_key = CacheResourceId::Chunk { chunk_id };
        let resource = cache_key.variant_label();

        let cell = {
            let mut cached_resources = self.cached_resources.lock().await;
            cached_resources
                .get_or_insert(cache_key, || Arc::new(OnceCell::new()))
                .clone()
        };
        let result = cell
            .get_or_try_init(async || {
                let content = match self.store.get_chunk_zst(chunk_id).await {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        return Err(None);
                    }
                    Err(err) => {
                        return Err(Some(err));
                    }
                };
                let content = content.into_data_stream().map_err(std::io::Error::other);
                let content = tokio_util::io::StreamReader::new(content);
                let file = match create_cache_file(init_id, cache_key, self, content).await {
                    Ok(file) => file,
                    Err(err) => {
                        return Err(Some(err));
                    }
                };
                Ok(Arc::new(file))
            })
            .await;

        let cache_file = match result {
            Ok(cache_file) => {
                if cache_file.id == init_id {
                    // File ID matches the ID we just generated, so this was
                    // a cache miss that we've now filled in
                    metrics::counter!("cache_misses", "resource" => resource).increment(1);
                } else {
                    // File ID does not match our ID, so the file was already
                    // populated meaning this was a cache hit
                    metrics::counter!("cache_hits", "resource" => resource).increment(1);
                }
                cache_file.clone()
            }
            Err(err) => {
                match err {
                    Some(err) => {
                        metrics::counter!(
                            "cache_errors",
                            "resource" => resource,
                        )
                        .increment(1);
                        return Err(err);
                    }
                    None => {
                        metrics::counter!(
                            "cache_not_found_accesses",
                            "resource" => resource,
                        )
                        .increment(1);
                        return Ok(None);
                    }
                };
            }
        };
        let body = body_from_cache_file(&cache_file).await?;
        Ok(Some(axum::body::Body::new(body)))
    }

    async fn get_artifact_bar_zst(
        &self,
        artifact_id: HashId,
    ) -> Result<Option<axum::body::Body>, StoreError> {
        let init_id = uuid::Uuid::new_v4();
        let cache_key = CacheResourceId::Artifact { artifact_id };
        let resource = cache_key.variant_label();

        let cell = {
            let mut cached_resources = self.cached_resources.lock().await;
            cached_resources
                .get_or_insert(cache_key, || Arc::new(OnceCell::new()))
                .clone()
        };

        let result = cell
            .get_or_try_init(async || {
                let content = match self.store.get_artifact_bar_zst(artifact_id).await {
                    Ok(Some(content)) => content,
                    Ok(None) => {
                        return Err(None);
                    }
                    Err(err) => {
                        return Err(Some(err));
                    }
                };
                let content = content.into_data_stream().map_err(std::io::Error::other);
                let content = tokio_util::io::StreamReader::new(content);
                let file = match create_cache_file(init_id, cache_key, self, content).await {
                    Ok(file) => file,
                    Err(err) => {
                        return Err(Some(err));
                    }
                };
                Ok(Arc::new(file))
            })
            .await;
        let cache_file = match result {
            Ok(cache_file) => {
                if cache_file.id == init_id {
                    // File ID matches the ID we just generated, so this was
                    // a cache miss that we've now filled in
                    metrics::counter!("cache_misses", "resource" => resource).increment(1);
                } else {
                    // File ID does not match our ID, so the file was already
                    // populated meaning this was a cache hit
                    metrics::counter!("cache_hits", "resource" => resource).increment(1);
                }
                cache_file.clone()
            }
            Err(err) => {
                match err {
                    Some(err) => {
                        metrics::counter!(
                            "cache_errors",
                            "resource" => resource,
                        )
                        .increment(1);
                        return Err(err);
                    }
                    None => {
                        metrics::counter!(
                            "cache_not_found_accesses",
                            "resource" => resource,
                        )
                        .increment(1);
                        return Ok(None);
                    }
                };
            }
        };
        let body = body_from_cache_file(&cache_file).await?;
        Ok(Some(body))
    }

    async fn get_project_source(
        &self,
        project_id: HashId,
    ) -> Result<Option<ProjectSource>, StoreError> {
        let init_id = uuid::Uuid::new_v4();
        let resource = "project_source";

        let cell = {
            let mut cached_project_sources = self.cached_project_sources.lock().await;
            cached_project_sources
                .get_or_insert(project_id, || Arc::new(OnceCell::new()))
                .clone()
        };
        let result = cell
            .get_or_try_init(async || {
                let project_source = self.store.get_project_source(project_id).await;
                match project_source {
                    Ok(Some(project_source)) => Ok((project_source, init_id)),
                    Ok(None) => Err(None),
                    Err(err) => Err(Some(err)),
                }
            })
            .await;
        match result {
            Ok((project_source, id)) => {
                if *id == init_id {
                    // File ID matches the ID we just generated, so this was
                    // a cache miss that we've now filled in
                    metrics::counter!("cache_misses", "resource" => resource).increment(1);
                } else {
                    // File ID does not match our ID, so the file was already
                    // populated meaning this was a cache hit
                    metrics::counter!("cache_hits", "resource" => resource).increment(1);
                }
                Ok(Some(project_source.clone()))
            }
            Err(err) => match err {
                Some(err) => {
                    metrics::counter!(
                        "cache_errors",
                        "resource" => resource,
                    )
                    .increment(1);
                    Err(err)
                }
                None => {
                    metrics::counter!(
                        "cache_not_found_accesses",
                        "resource" => resource,
                    )
                    .increment(1);
                    Ok(None)
                }
            },
        }
    }

    async fn get_bake_output(&self, recipe_id: HashId) -> Result<Option<BakeOutput>, StoreError> {
        let init_id = uuid::Uuid::new_v4();
        let resource = "bake_output";

        let cell = {
            let mut cached_bake_outputs = self.cached_bake_outputs.lock().await;
            cached_bake_outputs
                .get_or_insert(recipe_id, || Arc::new(OnceCell::new()))
                .clone()
        };
        let result = cell
            .get_or_try_init(async || {
                let bake_output = self.store.get_bake_output(recipe_id).await;
                match bake_output {
                    Ok(Some(bake_output)) => Ok((bake_output, init_id)),
                    Ok(None) => Err(None),
                    Err(err) => Err(Some(err)),
                }
            })
            .await;
        match result {
            Ok((bake_result, id)) => {
                if *id == init_id {
                    // File ID matches the ID we just generated, so this was
                    // a cache miss that we've now filled in
                    metrics::counter!("cache_misses", "resource" => resource).increment(1);
                } else {
                    // File ID does not match our ID, so the file was already
                    // populated meaning this was a cache hit
                    metrics::counter!("cache_hits", "resource" => resource).increment(1);
                }
                Ok(Some(bake_result.clone()))
            }
            Err(err) => match err {
                Some(err) => {
                    metrics::counter!(
                        "cache_errors",
                        "resource" => resource,
                    )
                    .increment(1);
                    Err(err)
                }
                None => {
                    metrics::counter!(
                        "cache_not_found_accesses",
                        "resource" => resource,
                    )
                    .increment(1);
                    Ok(None)
                }
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CacheResourceId {
    Chunk { chunk_id: HashId },
    Artifact { artifact_id: HashId },
}

impl CacheResourceId {
    fn variant_label(&self) -> &'static str {
        match self {
            Self::Chunk { .. } => "chunk",
            Self::Artifact { .. } => "artifact",
        }
    }
}

async fn create_cache_file<S>(
    id: uuid::Uuid,
    resource_id: CacheResourceId,
    store: &CacheStore<S>,
    mut data: impl tokio::io::AsyncBufRead + Unpin,
) -> Result<CacheFile, StoreError> {
    let cache_dir = store.cache_dir.clone();
    let mut file = tokio::task::spawn_blocking(move || {
        let file = tempfile::tempfile_in(&*cache_dir)?;
        std::io::Result::Ok(tokio::fs::File::from_std(file))
    })
    .await
    .unwrap()?;

    let size = tokio::io::copy(&mut data, &mut file).await?;

    let reservation = {
        let mut cached_resources_lock = None;
        loop {
            // Try and reserve enough space from the pool for the file
            let reservation = store.capacity_pool.reserve(size);
            if let Some(reservation) = reservation {
                // ...okay, we reserved the space
                break reservation;
            }

            // We couldn't reserve enough space, so make some space by
            // clearing the cache
            let cached_resources = match cached_resources_lock.as_mut() {
                None => cached_resources_lock.insert(store.cached_resources.lock().await),
                Some(cached_resources) => cached_resources,
            };

            let evicted = cached_resources.pop_lru();
            let Some((evicted_key, _)) = evicted else {
                // Cache is empty but there still wasn't enough space last
                // we checked. Well, we already wrote the file, so reserve
                // as much space as we can from the pool and continue onward

                let reservation = store.capacity_pool.reserve_up_to(size);

                if reservation.reserved < size {
                    tracing::warn!(
                        size,
                        reserved = reservation.reserved,
                        "nothing left to evict from cache but failed to reserve enough space for file, resource may be too big for cache or there might be a lot of requests in flight?"
                    );
                }

                break reservation;
            };

            metrics::counter!(
                "cache_evictions",
                "resource" => evicted_key.variant_label(),
            )
            .increment(1);

            // We just evicted something from the cache, so we're ready to
            // try again
        }
    };

    let resource = resource_id.variant_label();
    metrics::gauge!("cache_disk_bytes", "resource" => resource).increment(size as f64);
    metrics::gauge!("cache_disk_files", "resource" => resource).increment(1);

    Ok(CacheFile {
        id,
        resource_id,
        file,
        size,
        _reservation: reservation,
    })
}

async fn body_from_cache_file(cache_file: &CacheFile) -> tokio::io::Result<axum::body::Body> {
    let (body_tx, body_rx) = tokio::sync::mpsc::channel(1);
    let file = cache_file.file.try_clone().await?.into_std().await;

    tokio::task::spawn_blocking(move || {
        let mut offset: u64 = 0;
        let mut buf = [0; 16_384];
        loop {
            let result = file.read_at(&mut buf, offset);
            let send_result = match result {
                Ok(0) => {
                    break;
                }
                Ok(len) => {
                    let len_u64: u64 = len.try_into().unwrap();

                    let bytes = bytes::Bytes::copy_from_slice(&buf[..len]);
                    offset += len_u64;

                    body_tx.blocking_send(Ok(bytes))
                }
                Err(error) => body_tx.blocking_send(Err(error)),
            };
            if send_result.is_err() {
                break;
            }
        }
    });

    let body_stream = tokio_stream::wrappers::ReceiverStream::new(body_rx);

    let size_hint = http_body::SizeHint::with_exact(cache_file.size);
    let body = crate::response::BodyWithSize::new(crate::response::StreamBody::new(body_stream))
        .with_size_hint(Some(size_hint));
    Ok(axum::body::Body::new(body))
}

struct CacheFile {
    id: uuid::Uuid,
    resource_id: CacheResourceId,
    file: tokio::fs::File,
    size: u64,
    _reservation: CacheCapacityReservation,
}

impl Drop for CacheFile {
    fn drop(&mut self) {
        let resource = self.resource_id.variant_label();
        metrics::gauge!("cache_disk_bytes", "resource" => resource).decrement(self.size as f64);
        metrics::gauge!("cache_disk_files", "resource" => resource).decrement(1);
    }
}

#[derive(Debug, Clone)]
#[repr(transparent)]
struct CacheCapacityPool {
    available_capacity: Arc<AtomicU64>,
}

impl CacheCapacityPool {
    fn new(capacity: u64) -> Self {
        Self {
            available_capacity: Arc::new(AtomicU64::new(capacity)),
        }
    }

    fn reserve(&self, size: u64) -> Option<CacheCapacityReservation> {
        if size == 0 {
            return Some(CacheCapacityReservation {
                pool: self.clone(),
                reserved: 0,
            });
        }

        loop {
            let available_capacity = self
                .available_capacity
                .load(std::sync::atomic::Ordering::Acquire);
            let remaining_capacity = available_capacity.checked_sub(size)?;

            let result = self.available_capacity.compare_exchange(
                available_capacity,
                remaining_capacity,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            );

            if result.is_ok() {
                return Some(CacheCapacityReservation {
                    pool: self.clone(),
                    reserved: size,
                });
            }
        }
    }

    fn reserve_up_to(&self, size: u64) -> CacheCapacityReservation {
        if size == 0 {
            return CacheCapacityReservation {
                pool: self.clone(),
                reserved: size,
            };
        }

        loop {
            let available_capacity = self
                .available_capacity
                .load(std::sync::atomic::Ordering::Acquire);
            let remaining_capacity = available_capacity.saturating_sub(size);

            let result = self.available_capacity.compare_exchange(
                available_capacity,
                remaining_capacity,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            );

            if result.is_ok() {
                let reserved = available_capacity.checked_sub(remaining_capacity).unwrap();
                return CacheCapacityReservation {
                    pool: self.clone(),
                    reserved,
                };
            }
        }
    }
}

#[derive(Debug)]
struct CacheCapacityReservation {
    pool: CacheCapacityPool,
    reserved: u64,
}

impl CacheCapacityReservation {
    fn return_to_pool(&mut self) {
        if self.reserved == 0 {
            return;
        }

        loop {
            let current_capacity = self
                .pool
                .available_capacity
                .load(std::sync::atomic::Ordering::Acquire);
            let new_capacity = current_capacity
                .checked_add(self.reserved)
                .expect("disk pool capacity overflowed");

            let result = self.pool.available_capacity.compare_exchange(
                current_capacity,
                new_capacity,
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
            );

            if result.is_ok() {
                self.reserved = 0;
                return;
            }
        }
    }
}

impl Drop for CacheCapacityReservation {
    fn drop(&mut self) {
        self.return_to_pool();
    }
}
