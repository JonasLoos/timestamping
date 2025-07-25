use axum::{
    extract::Json,
    http::{Method, StatusCode, header},
    routing::post,
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod storage;
use crate::storage::HashStore;

#[derive(Debug, Deserialize, Serialize)]
struct AddHashRequest {
    hash: String,
}

#[derive(Debug, Serialize)]
struct AddHashResponse {
    success: bool,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CheckHashRequest {
    hash: String,
}

#[derive(Debug, Serialize)]
struct CheckHashResponse {
    success: bool,
    message: String,
    exists: bool,
}

#[tokio::main]
async fn main() {
    let hash_store = Arc::new(HashStore::<16, 0>::new());

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
        .allow_origin(Any);

    let app = Router::new()
        .route("/add", post(add))
        .route("/check", post(check))
        .layer(cors)
        .with_state(hash_store);

    println!("Server starting on http://127.0.0.1:3000");
    println!("POST /add - Add a 512-bit hash");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use crate::storage::{HashStore, Hash512};

    #[test]
    fn test_hash_store_functionality() {
        let store = HashStore::<8, 0>::new();

        // Create test hashes
        let hash1: Hash512 = [1u8; 64];
        let hash2: Hash512 = [2u8; 64];

        // Test initial state
        assert_eq!(store.len(), 0);
        assert!(!store.contains(&hash1));

        // Add hashes
        store.add_hash(hash1);
        assert_eq!(store.len(), 1);
        assert!(store.contains(&hash1));

        store.add_hash(hash2);
        assert_eq!(store.len(), 2);
        assert!(store.contains(&hash2));

        // Test to_array
        let array = store.to_array();
        assert_eq!(array.len(), 2);
        assert!(array.data.contains(&hash1));
        assert!(array.data.contains(&hash2));
    }
}

async fn add(
    axum::extract::State(hash_store): axum::extract::State<Arc<HashStore<16, 0>>>,
    Json(payload): Json<AddHashRequest>,
) -> (StatusCode, Json<AddHashResponse>) {
    // Validate hash length (512 bits = 64 bytes = 128 hex characters)
    if payload.hash.len() != 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AddHashResponse {
                success: false,
                message: "Hash must be exactly 128 characters (512 bits)".to_string(),
            }),
        );
    }

    // Validate hex format
    if !payload.hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(AddHashResponse {
                success: false,
                message: "Hash must be in hexadecimal format".to_string(),
            }),
        );
    }

    let hash_bytes = hex::decode(payload.hash).unwrap().try_into().unwrap();
    hash_store.add_hash(hash_bytes);

    (
        StatusCode::OK,
        Json(AddHashResponse {
            success: true,
            message: "Hash added successfully".to_string(),
        }),
    )
}

async fn check(
    axum::extract::State(hash_store): axum::extract::State<Arc<HashStore<16, 0>>>,
    Json(payload): Json<CheckHashRequest>,
) -> (StatusCode, Json<CheckHashResponse>) {
    // Validate hash length (512 bits = 64 bytes = 128 hex characters)
    if payload.hash.len() != 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(CheckHashResponse {
                success: false,
                message: "Hash must be exactly 128 characters (512 bits)".to_string(),
                exists: false,
            }),
        );
    }

    // Validate hex format
    if !payload.hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(CheckHashResponse {
                success: false,
                message: "Hash must be in hexadecimal format".to_string(),
                exists: false,
            }),
        );
    }

    let hash_bytes = hex::decode(payload.hash).unwrap().try_into().unwrap();
    let exists = hash_store.contains(&hash_bytes);

    (
        StatusCode::OK,
        Json(CheckHashResponse {
            success: true,
            message: if exists {
                "Hash found in store".to_string()
            } else {
                "Hash not found in store".to_string()
            },
            exists,
        }),
    )
}
