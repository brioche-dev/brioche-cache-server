use brioche_cache::{
    models::{BakeOutput, ProjectSource},
    store::Store as _,
};

use crate::test_utils::{body_to_string, cache_config, fake_hash_id, test_context};

mod test_utils;

#[tokio::test]
async fn test_cache_chunk() {
    let ctx = test_context();
    let mut mock_server = mockito::Server::new_async().await;

    let upstream_store =
        brioche_cache::store::http::HttpStore::new(mock_server.url().parse().unwrap());
    let cache =
        brioche_cache::store::cache::CacheStore::new(upstream_store, cache_config(&ctx)).unwrap();

    // Put a chunk in the upstream server
    let chunk_id = fake_hash_id(0x1234);
    let chunk_mock = mock_server
        .mock("GET", &*format!("/chunks/{chunk_id}.zst"))
        .with_body("chunk data")
        .expect(1)
        .create();

    // Validate that we can get the chunk via the cache
    let chunk = cache.get_chunk_zst(chunk_id).await.unwrap().unwrap();
    assert_eq!(body_to_string(chunk).await, "chunk data");

    // Get the same chunk again, but it should be cached and so shouldn't
    // trigger another upstream request.
    let chunk = cache.get_chunk_zst(chunk_id).await.unwrap().unwrap();
    assert_eq!(body_to_string(chunk).await, "chunk data");

    chunk_mock.assert_async().await;
}

#[tokio::test]
async fn test_cache_artifact() {
    let ctx = test_context();
    let mut mock_server = mockito::Server::new_async().await;

    let upstream_store =
        brioche_cache::store::http::HttpStore::new(mock_server.url().parse().unwrap());
    let cache =
        brioche_cache::store::cache::CacheStore::new(upstream_store, cache_config(&ctx)).unwrap();

    // Put an artifact in the upstream server
    let artifact_id = fake_hash_id(0x1111);
    let artifact_mock = mock_server
        .mock("GET", &*format!("/artifacts/{artifact_id}.bar.zst"))
        .with_body("fake artifact")
        .expect(1)
        .create();

    // Validate that we can get the artifact via the cache
    let artifact = cache
        .get_artifact_bar_zst(artifact_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body_to_string(artifact).await, "fake artifact");

    // Get the same artifact again, but it should be cached and so shouldn't
    // trigger another upstream request.
    let artifact = cache
        .get_artifact_bar_zst(artifact_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(body_to_string(artifact).await, "fake artifact");

    artifact_mock.assert_async().await;
}

#[tokio::test]
async fn test_cache_project_source() {
    let ctx = test_context();
    let mut mock_server = mockito::Server::new_async().await;

    let upstream_store =
        brioche_cache::store::http::HttpStore::new(mock_server.url().parse().unwrap());
    let cache =
        brioche_cache::store::cache::CacheStore::new(upstream_store, cache_config(&ctx)).unwrap();

    // Put a project source in the upstream server
    let project_id = fake_hash_id(0x4321);
    let project_artifact_hash = fake_hash_id(0x9999);
    let project_source_str = format!(r#"{{"artifactHash":"{project_artifact_hash}"}}"#);
    let project_source_mock = mock_server
        .mock("GET", &*format!("/projects/{project_id}/source.json"))
        .with_body(project_source_str)
        .expect(1)
        .create();

    // Validate that we can get the project source via the cache
    let project_source = cache.get_project_source(project_id).await.unwrap().unwrap();
    assert_eq!(
        project_source,
        ProjectSource {
            artifact_hash: project_artifact_hash,
        },
    );

    // Get the same artifact again, but it should be cached and so shouldn't
    // trigger another upstream request.
    let project_source = cache.get_project_source(project_id).await.unwrap().unwrap();
    assert_eq!(
        project_source,
        ProjectSource {
            artifact_hash: project_artifact_hash,
        },
    );

    project_source_mock.assert_async().await;
}

#[tokio::test]
async fn test_cache_bake_output() {
    let ctx = test_context();
    let mut mock_server = mockito::Server::new_async().await;

    let upstream_store =
        brioche_cache::store::http::HttpStore::new(mock_server.url().parse().unwrap());
    let cache =
        brioche_cache::store::cache::CacheStore::new(upstream_store, cache_config(&ctx)).unwrap();

    // Put a project source in the upstream server
    let recipe_id = fake_hash_id(0x9876);
    let output_artifact_hash = fake_hash_id(0x8888);
    let bake_output_json_str = format!(r#"{{"outputHash":"{output_artifact_hash}"}}"#);
    let bake_output_mock = mock_server
        .mock("GET", &*format!("/bakes/{recipe_id}/output.json"))
        .with_body(bake_output_json_str)
        .expect(1)
        .create();

    // Validate that we can get the project source via the cache
    let bake_output = cache.get_bake_output(recipe_id).await.unwrap().unwrap();
    assert_eq!(
        bake_output,
        BakeOutput {
            output_hash: output_artifact_hash,
        },
    );

    // Get the same artifact again, but it should be cached and so shouldn't
    // trigger another upstream request.
    let bake_output = cache.get_bake_output(recipe_id).await.unwrap().unwrap();
    assert_eq!(
        bake_output,
        BakeOutput {
            output_hash: output_artifact_hash,
        },
    );

    bake_output_mock.assert_async().await;
}
