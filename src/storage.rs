use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use sha2::{Digest, Sha512};

pub type Hash512 = [u64; 8];

#[derive(Debug)]
pub enum Hash512Error {
    InvalidLengthError,
}

impl std::fmt::Display for Hash512Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hash512Error::InvalidLengthError => write!(f, "Invalid hash length"),
        }
    }
}

impl std::error::Error for Hash512Error {}

// Trait for Hash512 operations
pub trait Hash512Ops {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Hash512Error> where Self: Sized;
    fn to_bytes(&self) -> Vec<u8>;
    fn to_index(&self, prefix_size: usize, index_size: usize) -> usize;
}

impl Hash512Ops for Hash512 {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Hash512Error> {
        if bytes.len() != 64 {
            return Err(Hash512Error::InvalidLengthError);
        }

        // Convert Vec<u8> to [u64; 8] by reading 8 bytes at a time
        let mut hash_array = [0u64; 8];
        for i in 0..8 {
            let start = i * 8;
            hash_array[i] = u64::from_le_bytes([
                bytes[start], bytes[start + 1], bytes[start + 2], bytes[start + 3],
                bytes[start + 4], bytes[start + 5], bytes[start + 6], bytes[start + 7]
            ]);
        }
        Ok(hash_array)
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.iter().flat_map(|&u64_val| u64_val.to_le_bytes()).collect()
    }

    fn to_index(&self, prefix_size: usize, index_size: usize) -> usize {
        // Extract index_size bits starting from prefix_size, assuming prefix_size + index_size <= 64
        if prefix_size + index_size > 64 { panic!("Prefix size + index size must be less than or equal to 64"); }
        if index_size == 0 { return 0; }

        // Only use the first u64
        ((self[0] << prefix_size) >> (64 - index_size)) as usize
    }
}

#[derive(Debug, Clone)]
pub struct HashLL {
    pub hash: Hash512,
    pub next: Option<Box<HashLL>>,
}

impl HashLL {
    pub fn new(hash: Hash512, next: Option<Box<HashLL>>) -> Self {
        Self { hash, next }
    }
}

fn hash512(a: Hash512, b: Hash512) -> Hash512 {
    let mut hasher = Sha512::new();
    hasher.update(&a.to_bytes());
    hasher.update(&b.to_bytes());
    let result = hasher.finalize();
    Hash512::from_bytes(&result).unwrap()
}

#[derive(Debug)]
pub struct HashStore<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    data: Arc<RwLock<Vec<Option<Box<HashLL>>>>>,
    salt: Hash512,
    num_elements: Arc<RwLock<usize>>,
    buckets_filled: Arc<RwLock<usize>>,
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> HashStore<INDEX_SIZE, PREFIX_SIZE> {
    pub fn new(salt: Hash512) -> Self {
        let total_buckets = 1 << INDEX_SIZE;
        Self {
            data: Arc::new(RwLock::new(vec![None; total_buckets])),
            salt,
            num_elements: Arc::new(RwLock::new(0)),
            buckets_filled: Arc::new(RwLock::new(0)),
        }
    }

    pub fn add_hash(&self, hash: Hash512) -> bool {
        let salted_hash = hash512(hash, self.salt);
        let index = salted_hash.to_index(PREFIX_SIZE, INDEX_SIZE);
        let mut data = self.data.write().unwrap();

        if data[index].is_none() {
            // Add hash to new bucket
            data[index] = Some(Box::new(HashLL::new(salted_hash, None)));
            *self.buckets_filled.write().unwrap() += 1;
            *self.num_elements.write().unwrap() += 1;
            return true;
        }

        // Check if hash already exists and find insertion point
        {
            let bucket = data[index].as_ref().unwrap();
            if salted_hash == bucket.hash {
                return false; // Hash already exists
            }

            if salted_hash < bucket.hash {
                // Insert at the front
                let old_bucket = data[index].take().unwrap();
                data[index] = Some(Box::new(HashLL::new(salted_hash, Some(old_bucket))));
                *self.num_elements.write().unwrap() += 1;
                return true;
            }
        }

        // Traverse the linked list to find the correct insertion point
        let bucket = data[index].as_mut().unwrap();
        let mut current = bucket;

        loop {
            if let Some(next_node) = &current.next {
                if salted_hash == next_node.hash {
                    return false; // Hash already exists
                }
                if salted_hash < next_node.hash {
                    // Insert between current and next
                    let old_next = current.next.take();
                    current.next = Some(Box::new(HashLL::new(salted_hash, old_next)));
                    *self.num_elements.write().unwrap() += 1;
                    return true;
                }
                // Move to next node
                current = current.next.as_mut().unwrap();
            } else {
                // Insert at the end
                current.next = Some(Box::new(HashLL::new(salted_hash, None)));
                *self.num_elements.write().unwrap() += 1;
                return true;
            }
        }
    }

    pub fn len(&self) -> usize {
        *self.num_elements.read().unwrap()
    }

    pub fn occupied_slots(&self) -> usize {
        *self.buckets_filled.read().unwrap()
    }

    pub fn contains(&self, hash: &Hash512) -> bool {
        let salted_hash = hash512(*hash, self.salt);
        let index = salted_hash.to_index(PREFIX_SIZE, INDEX_SIZE);
        let data = self.data.read().unwrap();

        if let Some(node) = &data[index] {
            let mut current = node;
            loop {
                if current.hash == salted_hash {
                    return true;
                }
                match &current.next {
                    Some(next) => current = next,
                    None => break,
                }
            }
        }
        false
    }

    pub fn to_array(&self) -> Vec<Hash512> {
        let mut hashes = Vec::new();
        let data = self.data.read().unwrap();

        for bucket in data.iter() {
            if let Some(node) = bucket {
                let mut current = node;
                loop {
                    hashes.push(current.hash);
                    match &current.next {
                        Some(next) => current = next,
                        None => break,
                    }
                }
            }
        }

        hashes
    }
}

#[derive(Debug)]
pub struct MultiThreadedHashStore<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    threads: Vec<Sender<HashCommand>>,
    salt: Hash512,
}

#[derive(Debug)]
enum HashCommand {
    AddHash(Hash512),
    Contains(Hash512, Sender<bool>),
    GetArray(Sender<Vec<Hash512>>),
    GetLen(Sender<usize>),
    GetOccupiedSlots(Sender<usize>),
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> MultiThreadedHashStore<INDEX_SIZE, PREFIX_SIZE> {
    pub fn new(num_threads: usize, salt: Hash512) -> Self {
        // Ensure num_threads is a power of 2
        if !num_threads.is_power_of_two() {
            panic!("Number of threads must be a power of 2");
        }
        let mut threads = Vec::new();

        for _ in 0..num_threads {
            let (tx, rx) = channel();
            threads.push(tx);

            let store = HashStore::<INDEX_SIZE, PREFIX_SIZE>::new(salt);

            thread::spawn(move || {
                Self::hash_store_worker(store, rx);
            });
        }

        Self {
            threads,
            salt,
        }
    }

    fn hash_store_worker(store: HashStore<INDEX_SIZE, PREFIX_SIZE>, rx: Receiver<HashCommand>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                HashCommand::AddHash(hash) => {
                    let _is_new = store.add_hash(hash);
                }
                HashCommand::Contains(hash, tx) => {
                    let exists = store.contains(&hash);
                    let _ = tx.send(exists);
                }
                HashCommand::GetArray(tx) => {
                    let array = store.to_array();
                    let _ = tx.send(array);
                }
                HashCommand::GetLen(tx) => {
                    let len = store.len();
                    let _ = tx.send(len);
                }
                HashCommand::GetOccupiedSlots(tx) => {
                    let slots = store.occupied_slots();
                    let _ = tx.send(slots);
                }
            }
        }
    }

    pub fn add_hash(&self, hash: Hash512) -> bool {
        let thread_index = hash.to_index(0, (self.threads.len() as f64).log2().ceil() as usize);
        let tx = &self.threads[thread_index];

        let _ = tx.send(HashCommand::AddHash(hash));

        // TODO: return the result of the add_hash operation
        true
    }

    pub fn contains(&self, hash: &Hash512) -> bool {
        let thread_index = hash.to_index(0, (self.threads.len() as f64).log2().ceil() as usize);
        let tx = &self.threads[thread_index];
        let (response_tx, response_rx) = channel();

        let _ = tx.send(HashCommand::Contains(*hash, response_tx));
        response_rx.recv().unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        let mut total = 0;
        for tx in &self.threads {
            let (response_tx, response_rx) = channel();
            let _ = tx.send(HashCommand::GetLen(response_tx));
            total += response_rx.recv().unwrap_or(0);
        }
        total
    }

    pub fn occupied_slots(&self) -> usize {
        let mut total = 0;
        for tx in &self.threads {
            let (response_tx, response_rx) = channel();
            let _ = tx.send(HashCommand::GetOccupiedSlots(response_tx));
            total += response_rx.recv().unwrap_or(0);
        }
        total
    }

    pub fn to_array(&self) -> Vec<Hash512> {
        let mut all_hashes = Vec::new();

        // Collect arrays from all threads
        for tx in &self.threads {
            let (response_tx, response_rx) = channel();
            let _ = tx.send(HashCommand::GetArray(response_tx));
            if let Ok(array) = response_rx.recv() {
                all_hashes.extend(array);
            }
        }

        all_hashes
    }
}

#[derive(Debug, Clone)]
pub struct MerkleTree {
    pub data: Vec<Hash512>,
    pub salt: Hash512,
    pub depth: usize,
    pub leaf_count: usize,
}

impl MerkleTree {
    pub fn new(data: Vec<Hash512>, salt: Hash512) -> Self {
        let n = data.len();
        if n == 0 {
            return Self {
                data: vec![],
                salt,
                depth: 0,
                leaf_count: 0,
            };
        }
        let depth = (n as f64).log2().ceil() as usize;
        let tree_size = (1 << (depth + 1)) - 1;

        let mut tree_data = vec![[0u64; 8]; tree_size];

        // Copy data to leaves (rightmost part of the tree)
        let leaf_start = (1 << depth) - 1;
        tree_data[leaf_start..leaf_start + n].copy_from_slice(&data[..n]);

        // Build tree from bottom up
        for level in (0..depth).rev() {
            let level_start = (1 << level) - 1;
            let child_level_start = (1 << (level + 1)) - 1;

            for i in 0..(1 << level) {
                let parent_idx = level_start + i;
                let left_child_idx = child_level_start + 2 * i;
                let right_child_idx = child_level_start + 2 * i + 1;

                tree_data[parent_idx] = hash512(tree_data[left_child_idx], tree_data[right_child_idx]);
            }
        }
        Self {
            data: tree_data,
            salt,
            depth,
            leaf_count: n,
        }
    }

    pub fn get(&self, hash: &Hash512) -> Option<Vec<(Hash512, Hash512)>> {
        if self.leaf_count == 0 {
            return None;
        }

        let salted_hash = hash512(*hash, self.salt);

        // Find the hash in the leaves
        let leaf_start = (1 << self.depth) - 1;
        let mut hash_idx = None;

        for i in 0..self.leaf_count {
            if self.data[leaf_start + i] == salted_hash {
                hash_idx = Some(i);
                break;
            }
        }

        let hash_idx = hash_idx?;

        // Generate proof path from leaf to root
        let mut proof = Vec::with_capacity(self.depth);
        proof.push((*hash, self.salt.clone()));
        let mut current_idx = hash_idx;

        for level in (0..self.depth).rev() {
            let level_start = (1 << level) - 1;
            let left_child_idx = level_start + (1 << level) + (current_idx & !1);
            let right_child_idx = left_child_idx + 1;

            if left_child_idx < self.data.len() && right_child_idx < self.data.len() {
                proof.push((self.data[left_child_idx], self.data[right_child_idx]));
            }

            current_idx /= 2;
        }

        Some(proof)
    }

    pub fn root(&self) -> Option<Hash512> {
        if self.data.is_empty() {
            None
        } else {
            Some(self.data[0])
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }
}

#[derive(Debug, Clone)]
pub struct TimestampingService<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    pub hash_store: Arc<MultiThreadedHashStore<INDEX_SIZE, PREFIX_SIZE>>,
    pub merkle_tree: Arc<RwLock<Option<MerkleTree>>>,
    pub last_tree_update: Arc<RwLock<Option<SystemTime>>>,
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> TimestampingService<INDEX_SIZE, PREFIX_SIZE> {
    pub fn with_threads(num_threads: usize) -> Self {
        let salt = [rand::random(), rand::random(), rand::random(), rand::random(),
                    rand::random(), rand::random(), rand::random(), rand::random()];
        Self {
            hash_store: Arc::new(MultiThreadedHashStore::new(num_threads, salt)),
            merkle_tree: Arc::new(RwLock::new(None)),
            last_tree_update: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_merkle_tree(&self) {
        let new_tree = MerkleTree::new(self.hash_store.to_array(), self.hash_store.salt);

        *self.merkle_tree.write().unwrap() = Some(new_tree);
        *self.last_tree_update.write().unwrap() = Some(SystemTime::now());
    }

    pub fn get_merkle_proof(&self, hash: &Hash512) -> Option<Vec<(Vec<u8>, Vec<u8>)>> {
        self.merkle_tree
        .read().unwrap().as_ref()?
        .get(hash)
        .map(|proof| {
            proof.into_iter()
                .map(|(left, right)| (left.to_bytes(), right.to_bytes()))
                .collect()
        })
    }

    pub fn get_merkle_tree_root_bytes(&self) -> Option<Vec<u8>> {
        self.get_merkle_tree_root().map(|root| root.to_bytes())
    }

    pub fn get_last_update_timestamp(&self) -> Option<u64> {
        self.last_tree_update
            .read()
            .unwrap()
            .as_ref()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs())
    }

    pub fn get_merkle_tree_size(&self) -> usize {
        self.merkle_tree
            .read()
            .unwrap()
            .as_ref()
            .map(|tree| tree.size())
            .unwrap_or(0)
    }

    pub fn get_merkle_tree_root(&self) -> Option<Hash512> {
        self.merkle_tree
            .read()
            .unwrap()
            .as_ref()
            .and_then(|tree| tree.root())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    static SALT: Hash512 = [0, 0, 0, 0, 0, 0, 0, 0];

    #[test]
    fn test_hash512_byte_conversion() {
        // convert hash to bytes
        let hash = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];
        let bytes = hash.to_bytes();
        assert_eq!(bytes.len(), 64);

        // Convert back and verify
        let reconstructed = Hash512::from_bytes(&bytes).unwrap();
        assert_eq!(reconstructed, hash);

        // Test invalid length
        let invalid_bytes = vec![1u8; 32];
        assert!(Hash512::from_bytes(&invalid_bytes).is_err());
    }

    #[test]
    fn test_hash512_to_index() {
        let hash = [0x1234567890ABCDEFu64, 0, 0, 0, 0, 0, 0, 0];

        assert_eq!(hash.to_index(0, 8), 0x12);
        assert_eq!(hash.to_index(8, 8), 0x34);
        assert_eq!(hash.to_index(0, 1), 0);
        assert_eq!(hash.to_index(0, 64), 0x1234567890ABCDEF);
    }

    #[test]
    #[should_panic(expected = "Prefix size + index size must be less than or equal to 64")]
    fn test_hash512_to_index_panic() {
        let hash = [0u64; 8];
        hash.to_index(32, 33); // This should panic
    }

    #[test]
    fn test_hash_store_basic_operations() {
        let store = HashStore::<8, 0>::new(SALT);

        // Test empty store
        assert_eq!(store.len(), 0);
        assert_eq!(store.occupied_slots(), 0);

        // Create a test hash
        let hash = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];

        // Test adding hash
        assert!(store.add_hash(hash));
        assert_eq!(store.len(), 1);
        assert_eq!(store.occupied_slots(), 1);
        assert!(store.contains(&hash));

        // Test adding duplicate
        assert!(!store.add_hash(hash));
        assert_eq!(store.len(), 1);
        assert_eq!(store.occupied_slots(), 1);

        // Test adding different hash
        let hash2 = [u64::MAX, 10u64, 11u64, 12u64, 13u64, 14u64, 15u64, 16u64];
        assert!(store.add_hash(hash2));
        assert_eq!(store.len(), 2);
        assert_eq!(store.occupied_slots(), 2);
    }

    #[test]
    fn test_hash_store_ordering() {
        let store = HashStore::<8, 0>::new(SALT);

        for i in 0..10 {
            let hash = [i as u64, 0, 0, 0, 0, 0, 0, 0];
            store.add_hash(hash);
        }

        assert_eq!(store.to_array().len(), 10);
        let mut array = store.to_array();
        array.sort_by(|a, b| a[0].cmp(&b[0]));
        assert_eq!(array, store.to_array());
    }

    #[test]
    fn test_multi_threaded_hash_store() {
        let store = MultiThreadedHashStore::<8, 0>::new(4, SALT);

        // Test empty store
        assert_eq!(store.len(), 0);
        assert_eq!(store.occupied_slots(), 0);

        let hash = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];

        // Test adding hash
        store.add_hash(hash);

        // Give some time for the operation to complete
        std::thread::sleep(Duration::from_millis(10));

        assert!(store.contains(&hash));

        // Test adding duplicate
        store.add_hash(hash);
        std::thread::sleep(Duration::from_millis(10));

        // Test adding different hash
        let hash2 = [9u64, 10u64, 11u64, 12u64, 13u64, 14u64, 15u64, 16u64];
        store.add_hash(hash2);
        std::thread::sleep(Duration::from_millis(10));

        assert!(store.contains(&hash2));
    }

    #[test]
    fn test_merkle_tree_basic() {
        let array = vec![
            [1u64, 0, 0, 0, 0, 0, 0, 0],
            [2u64, 0, 0, 0, 0, 0, 0, 0],
            [3u64, 0, 0, 0, 0, 0, 0, 0],
            [4u64, 0, 0, 0, 0, 0, 0, 0],
        ];

        let tree = MerkleTree::new(array, SALT);

        assert_eq!(tree.leaf_count, 4);
        assert_eq!(tree.depth, 2);
        assert!(tree.root().is_some());
        assert_eq!(tree.size(), 7); // 2^3 - 1 = 7 nodes
    }

    #[test]
    fn test_merkle_tree_empty() {
        let array = vec![];
        let tree = MerkleTree::new(array, SALT);

        assert_eq!(tree.leaf_count, 0);
        assert_eq!(tree.depth, 0);
        assert!(tree.root().is_none());
        assert_eq!(tree.size(), 0);
    }

    #[test]
    fn test_merkle_tree_single_element() {
        let hash = [1u64, 0, 0, 0, 0, 0, 0, 0];
        let array = vec![hash];
        let tree = MerkleTree::new(array, SALT);

        assert_eq!(tree.leaf_count, 1);
        assert_eq!(tree.depth, 0);
        assert!(tree.root().is_some());
        assert_eq!(tree.root().unwrap(), hash);
    }

    #[test]
    fn test_merkle_proof() {
        let hashes = vec![
            [1u64, 0, 0, 0, 0, 0, 0, 0],
            [2u64, 0, 0, 0, 0, 0, 0, 0],
            [3u64, 0, 0, 0, 0, 0, 0, 0],
            [4u64, 0, 0, 0, 0, 0, 0, 0],
        ];

        let salted_hashes = hashes.iter().map(|hash| hash512(*hash, SALT)).collect();
        let tree = MerkleTree::new(salted_hashes, SALT);

        // Test proof for first hash
        let proof = tree.get(&hashes[0]);
        assert!(proof.is_some());

        // Test proof for non-existent hash
        let non_existent = [999u64, 0, 0, 0, 0, 0, 0, 0];
        let proof = tree.get(&non_existent);
        assert!(proof.is_none());
    }

    #[test]
    fn test_timestamping_service() {
        let service = TimestampingService::<8, 0>::with_threads(4);

        // Test initial state
        assert_eq!(service.get_merkle_tree_size(), 0);
        assert!(service.get_merkle_tree_root().is_none());
        assert!(service.get_last_update_timestamp().is_none());

        // Add some hashes
        let hash1 = [1u64, 0, 0, 0, 0, 0, 0, 0];
        let hash2 = [2u64, 0, 0, 0, 0, 0, 0, 0];

        service.hash_store.add_hash(hash1);
        service.hash_store.add_hash(hash2);

        // Give time for operations to complete
        std::thread::sleep(Duration::from_millis(10));

        // Update merkle tree
        service.update_merkle_tree();

        // Test updated state
        assert!(service.get_merkle_tree_root().is_some());
        assert!(service.get_last_update_timestamp().is_some());
        assert!(service.get_merkle_tree_size() > 0);

        // Test merkle proof
        let proof = service.get_merkle_proof(&hash1);
        assert!(proof.is_some());

        // Test merkle root bytes
        let root_bytes = service.get_merkle_tree_root_bytes();
        assert!(root_bytes.is_some());
        assert_eq!(root_bytes.unwrap().len(), 64);
    }

    #[test]
    fn test_hash_store_collision_handling() {
        let store = HashStore::<2, 0>::new(SALT); // Only 4 buckets

        // Create hashes where some will collide in the same bucket
        for i in 0..10 {
            let hash = [i as u64, 0, 0, 0, 0, 0, 0, 0];
            store.add_hash(hash);
        }

        assert_eq!(store.len(), 10);
        for i in 0..10 {
            let hash = [i as u64, 0, 0, 0, 0, 0, 0, 0];
            assert!(store.contains(&hash));
        }
    }

    #[test]
    fn test_hash512_equality() {
        let hash1 = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];
        let hash2 = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64, 8u64];
        let hash3 = [9u64, 10u64, 11u64, 12u64, 13u64, 14u64, 15u64, 16u64];

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash512_ordering() {
        let hash1 = [1u64, 0, 0, 0, 0, 0, 0, 0];
        let hash2 = [2u64, 0, 0, 0, 0, 0, 0, 0];
        let hash3 = [1u64, 1u64, 0, 0, 0, 0, 0, 0];

        assert!(hash1 < hash2);
        assert!(hash1 < hash3);
        assert!(hash2 > hash3);
    }

    #[test]
    fn test_merkle_tree_large_dataset() {
        let mut hashes = Vec::new();
        for i in 0..100 {
            hashes.push([i as u64, 0, 0, 0, 0, 0, 0, 0]);
        }
        let salted_hashes = hashes.iter().map(|hash| hash512(*hash, SALT)).collect();

        let tree = MerkleTree::new(salted_hashes, SALT);

        assert_eq!(tree.leaf_count, 100);
        assert!(tree.root().is_some());

        // Test proof for middle element
        let proof = tree.get(&hashes[50]);
        assert!(proof.is_some());
    }

    #[test]
    fn test_multi_threaded_hash_store_concurrent_access() {
        let store = Arc::new(MultiThreadedHashStore::<8, 0>::new(4, SALT));
        let mut handles = Vec::new();

        // Spawn multiple threads adding hashes concurrently
        for i in 0..10 {
            let store_clone = Arc::clone(&store);
            let handle = std::thread::spawn(move || {
                for j in 0..100 {
                    let hash = [(i * 100 + j) as u64, 0, 0, 0, 0, 0, 0, 0];
                    store_clone.add_hash(hash);
                }
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Give some time for all operations to complete
        std::thread::sleep(Duration::from_millis(50));

        // Verify all hashes are present
        for i in 0..10 {
            for j in 0..100 {
                let hash = [(i * 100 + j) as u64, 0, 0, 0, 0, 0, 0, 0];
                assert!(store.contains(&hash));
            }
        }
    }
}
