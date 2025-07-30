use axum::{
    body::Bytes,
    extract::{Json, State},
    http::{Method, StatusCode, header},
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use serde::Serialize;
use std::sync::Arc;

mod storage;
use crate::storage::TimestampingService;

#[derive(Debug, Serialize)]
struct AddResponse {
    success: bool,
    message: String,
    total_hashes: usize,
    new_hashes: usize,
    existing_hashes: usize,
}

#[derive(Debug, Serialize)]
struct CheckHashResponse {
    success: bool,
    message: &'static str,
    exists: bool,
    merkle_proof: Option<Vec<(Vec<u8>, Vec<u8>)>>,
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
    merkle_tree_root: Option<Vec<u8>>,
    last_tree_update: Option<u64>,
}

const INDEX_SIZE: usize = 28;
const PREFIX_SIZE: usize = 0;
const NUM_THREADS: usize = 8; // Number of threads for hash distribution

// Pre-allocated response messages
const MSG_HASH_FOUND: &str = "Hash found in store";
const MSG_HASH_NOT_FOUND: &str = "Hash not found in store";
const MSG_INVALID_LENGTH: &str = "Invalid hash length - must be exactly 64 bytes";
const MSG_INVALID_BATCH_SIZE: &str = "Invalid batch size - must be multiple of 64 bytes";

#[tokio::main]
async fn main() {
    let timestamping_service = Arc::new(TimestampingService::<INDEX_SIZE, PREFIX_SIZE>::with_threads(NUM_THREADS));

    let cors = CorsLayer::new()
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE])
        .allow_origin(Any);

    let app = Router::new()
        .route("/add", post(add))
        .route("/check", post(check))
        .route("/update-tree", post(update_tree))
        .route("/stats", get(get_stats))
        .layer(cors)
        .with_state(timestamping_service);

    println!("Server starting on http://127.0.0.1:3427");
    println!("POST /add - Add multiple 512-bit hashes (raw bytes, multiple of 64 bytes)");
    println!("POST /check - Check if hash exists and get merkle proof (raw bytes, 64 bytes)");
    println!("POST /update-tree - Update the merkle tree");
    println!("GET /stats - Get storage statistics");
    println!("Using {} threads for hash distribution", NUM_THREADS);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3427")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn add(
    State(service): State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
    bytes: Bytes,
) -> (StatusCode, Json<AddResponse>) {
    // Check that the total size is a multiple of 64 bytes
    if bytes.len() % 64 != 0 {
        return (
            StatusCode::BAD_REQUEST,
            Json(AddResponse {
                success: false,
                message: MSG_INVALID_BATCH_SIZE.to_string(),
                total_hashes: 0,
                new_hashes: 0,
                existing_hashes: 0,
            }),
        );
    }

    let total_hashes = bytes.len() / 64;
    let mut new_hashes = 0;
    let mut existing_hashes = 0;

    // Process hashes in chunks of 64 bytes
    for i in 0..total_hashes {
        let start = i * 64;
        let end = start + 64;
        let hash_bytes = bytes[start..end].to_vec();

        let is_new = service.hash_store.add_hash(hash_bytes);
        if is_new {
            new_hashes += 1;
        } else {
            existing_hashes += 1;
        }
    }

    let message = format!(
        "Batch processed: {} total, {} new, {} existing",
        total_hashes, new_hashes, existing_hashes
    );

    (
        StatusCode::OK,
        Json(AddResponse {
            success: true,
            message,
            total_hashes,
            new_hashes,
            existing_hashes,
        }),
    )
}

async fn check(
    State(service): State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
    bytes: Bytes,
) -> (StatusCode, Json<CheckHashResponse>) {
    // Check the length of the raw bytes
    if bytes.len() != 64 {
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

    let exists = service.hash_store.contains(&bytes);
    let merkle_proof = if exists {
        service.get_merkle_proof(&bytes.to_vec())
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
    State(service): State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
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
    State(service): State<Arc<TimestampingService<INDEX_SIZE, PREFIX_SIZE>>>,
) -> (StatusCode, Json<GetStatsResponse>) {
    let stats = GetStatsResponse {
        count: service.hash_store.len(),
        slots: service.hash_store.occupied_slots(),
        total_slots: 1 << INDEX_SIZE,
        merkle_tree_size: service.get_merkle_tree_size(),
        merkle_tree_root: service.get_merkle_tree_root_bytes(),
        last_tree_update: service.get_last_update_timestamp(),
    };
    (StatusCode::OK, Json(stats))
}
