use brioche_cache::models::HashId;

pub fn fake_hash_id(n: u64) -> HashId {
    let bytes = n.to_be_bytes();
    let mut hash_bytes = [0; HashId::LEN];
    hash_bytes[(HashId::LEN - bytes.len())..].copy_from_slice(&bytes);
    HashId::from_bytes(hash_bytes)
}

pub async fn body_to_string(body: axum::body::Body) -> String {
    let bytes = axum::body::to_bytes(body, 10_000_000).await.unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
}
