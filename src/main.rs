use axum::{
    extract::Json,
    http::{Method, StatusCode, header},
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

mod storage;
use crate::storage::TimestampingService;

#[derive(Debug, Deserialize, Serialize)]
struct AddHashRequest {
    hash: String, // base64 encoded bytes
}

#[derive(Debug, Serialize)]
struct AddHashResponse {
    success: bool,
    message: &'static str,
    is_new: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct AddBatchRequest {
    hashes: Vec<String>, // base64 encoded bytes
}

#[derive(Debug, Serialize)]
struct AddBatchResponse {
    success: bool,
    message: String,
    total_hashes: usize,
    new_hashes: usize,
    existing_hashes: usize,
    results: Vec<BatchHashResult>,
}

#[derive(Debug, Serialize)]
struct BatchHashResult {
    hash: String, // base64 encoded bytes
    is_new: bool,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CheckHashRequest {
    hash: String, // base64 encoded bytes
}

#[derive(Debug, Serialize)]
struct CheckHashResponse {
    success: bool,
    message: &'static str,
    exists: bool,
    merkle_proof: Option<Vec<(String, String)>>, // base64 encoded bytes
}

#[derive(Debug, Serialize)]
struct UpdateTreeResponse {
    success: bool,
    message: String,
    tree_size: usize,
    hash_count: usize,
}

#[derive(Debug, Serialize)]
struct GetStatsResponse {
    count: usize,
    slots: usize,
    total_slots: usize,
    merkle_tree_size: usize,
    merkle_tree_root: Option<String>, // base64 encoded bytes
    last_tree_update: Option<u64>,
}

const INDEX_SIZE: usize = 28;
const PREFIX_SIZE: usize = 0;
const NUM_THREADS: usize = 8; // Number of threads for hash distribution

// Pre-allocated response messages
const MSG_HASH_ADDED: &str = "Hash added successfully";
const MSG_HASH_EXISTS: &str = "Hash already exists";
const MSG_HASH_FOUND: &str = "Hash found in store";
const MSG_HASH_NOT_FOUND: &str = "Hash not found in store";
const MSG_INVALID_LENGTH: &str = "Invalid hash length - must be 64 bytes";
const MSG_INVALID_BASE64: &str = "Invalid base64 format";

#[tokio::main]
async fn main() {
    let timestamping_service = Arc::new(TimestampingService::<INDEX_SIZE, PREFIX_SIZE>::with_threads(NUM_THREADS));

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
        .allow_origin(Any);

    let app = Router::new()
        .route("/add", post(add))
        .route("/add-batch", post(add_batch))
        .route("/check", post(check))
        .route("/update-tree", post(update_tree))
        .route("/stats", get(get_stats))
        .layer(cors)
        .with_state(timestamping_service);

    println!("Server starting on http://127.0.0.1:3427");
    println!("POST /add - Add a 512-bit hash");
    println!("POST /add-batch - Add multiple 512-bit hashes");
    println!("POST /check - Check if hash exists and get merkle proof");
    println!("POST /update-tree - Update the merkle tree");
    println!("GET /stats - Get storage statistics");
    println!("Using {} threads for hash distribution", NUM_THREADS);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3427")
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
        let is_new1 = store.add_hash(hash1);
        assert!(is_new1);
        assert_eq!(store.len(), 1);
        assert!(store.contains(&hash1));

        let is_new2 = store.add_hash(hash2);
        assert!(is_new2);
        assert_eq!(store.len(), 2);
        assert!(store.contains(&hash2));

        // Test duplicate hash
        let is_new_duplicate = store.add_hash(hash1);
        assert!(!is_new_duplicate);
        assert_eq!(store.len(), 2);

        // Test to_array
        let array = store.to_array();
        assert_eq!(array.data.len(), 2);
        assert!(array.data.contains(&hash1));
        assert!(array.data.contains(&hash2));
    }
}

async fn add(
    axum::extract::State(service): axum::extract::State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
    Json(payload): Json<AddHashRequest>,
) -> (StatusCode, Json<AddHashResponse>) {
    // Decode base64 hash
    let hash_bytes = match BASE64.decode(&payload.hash) {
        Ok(bytes) => match bytes.try_into() {
            Ok(hash_array) => hash_array,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AddHashResponse {
                        success: false,
                        message: MSG_INVALID_LENGTH,
                        is_new: false,
                    }),
                );
            }
        },
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(AddHashResponse {
                    success: false,
                    message: MSG_INVALID_BASE64,
                    is_new: false,
                }),
            );
        }
    };

    let is_new = service.hash_store.add_hash(hash_bytes);

    (
        StatusCode::OK,
        Json(AddHashResponse {
            success: true,
            message: if is_new { MSG_HASH_ADDED } else { MSG_HASH_EXISTS },
            is_new,
        }),
    )
}

async fn add_batch(
    axum::extract::State(service): axum::extract::State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
    Json(payload): Json<AddBatchRequest>,
) -> (StatusCode, Json<AddBatchResponse>) {
    let mut results = Vec::new();
    let mut new_hashes = 0;
    let mut existing_hashes = 0;

    for hash_str in payload.hashes {
        // Decode base64 hash
        let hash_bytes = match BASE64.decode(&hash_str) {
            Ok(bytes) => match bytes.try_into() {
                Ok(hash_array) => hash_array,
                Err(_) => {
                    results.push(BatchHashResult {
                        hash: hash_str,
                        is_new: false,
                        error: Some(MSG_INVALID_LENGTH.to_string()),
                    });
                    continue;
                }
            },
            Err(_) => {
                results.push(BatchHashResult {
                    hash: hash_str,
                    is_new: false,
                    error: Some(MSG_INVALID_BASE64.to_string()),
                });
                continue;
            }
        };

        let is_new = service.hash_store.add_hash(hash_bytes);
        if is_new {
            new_hashes += 1;
        } else {
            existing_hashes += 1;
        }

        results.push(BatchHashResult {
            hash: hash_str,
            is_new,
            error: None,
        });
    }

    let total_hashes = results.len();
    let message = format!(
        "Batch processed: {} total, {} new, {} existing",
        total_hashes, new_hashes, existing_hashes
    );

    (
        StatusCode::OK,
        Json(AddBatchResponse {
            success: true,
            message,
            total_hashes,
            new_hashes,
            existing_hashes,
            results,
        }),
    )
}

async fn check(
    axum::extract::State(service): axum::extract::State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
    Json(payload): Json<CheckHashRequest>,
) -> (StatusCode, Json<CheckHashResponse>) {
    // Decode base64 hash
    let hash_bytes = match BASE64.decode(&payload.hash) {
        Ok(bytes) => match bytes.try_into() {
            Ok(hash_array) => hash_array,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(CheckHashResponse {
                        success: false,
                        message: MSG_INVALID_LENGTH,
                        exists: false,
                        merkle_proof: None,
                    }),
                );
            }
        },
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(CheckHashResponse {
                    success: false,
                    message: MSG_INVALID_BASE64,
                    exists: false,
                    merkle_proof: None,
                }),
            );
        }
    };

    let exists = service.hash_store.contains(&hash_bytes);
    let merkle_proof = if exists {
        service.get_merkle_proof(&hash_bytes).map(|proof| {
            proof.into_iter()
                .map(|(left, right)| (BASE64.encode(left), BASE64.encode(right)))
                .collect()
        })
    } else {
        None
    };

    (
        StatusCode::OK,
        Json(CheckHashResponse {
            success: true,
            message: if exists { MSG_HASH_FOUND } else { MSG_HASH_NOT_FOUND },
            exists,
            merkle_proof,
        }),
    )
}

async fn update_tree(
    axum::extract::State(service): axum::extract::State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
) -> (StatusCode, Json<UpdateTreeResponse>) {
    let hash_count = service.hash_store.len();
    service.update_merkle_tree();
    let tree_size = service.get_merkle_tree_size();

    (
        StatusCode::OK,
        Json(UpdateTreeResponse {
            success: true,
            message: format!("Merkle tree updated with {} hashes", hash_count),
            tree_size,
            hash_count,
        }),
    )
}

async fn get_stats(
    axum::extract::State(service): axum::extract::State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
) -> (StatusCode, Json<GetStatsResponse>) {
    let stats = GetStatsResponse {
        count: service.hash_store.len(),
        slots: service.hash_store.occupied_slots(),
        total_slots: 1 << INDEX_SIZE,
        merkle_tree_size: service.get_merkle_tree_size(),
        merkle_tree_root: service.get_merkle_tree_root().map(|root| BASE64.encode(root)),
        last_tree_update: service.get_last_update_timestamp(),
    };
    (StatusCode::OK, Json(stats))
}
