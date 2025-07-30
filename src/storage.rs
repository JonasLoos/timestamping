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
            let end = start + 8;
            if end <= bytes.len() {
                hash_array[i] = u64::from_le_bytes([
                    bytes[start], bytes[start + 1], bytes[start + 2], bytes[start + 3],
                    bytes[start + 4], bytes[start + 5], bytes[start + 6], bytes[start + 7]
                ]);
            }
        }
        Ok(hash_array)
    }

    fn to_bytes(&self) -> Vec<u8> {
        self.iter().flat_map(|&u64_val| u64_val.to_le_bytes()).collect()
    }

    fn to_index(&self, prefix_size: usize, index_size: usize) -> usize {
        // Extract index_size bits starting from prefix_size, assuming prefix_size + index_size <= 64
        if prefix_size + index_size > 64 {
            panic!("Prefix size + index size must be less than or equal to 64");
        }

        let bit_start = prefix_size % 64;
        let u64_val = self[0]; // Only use the first u64

        let mask = if index_size == 64 {
            u64::MAX
        } else {
            (1u64 << index_size) - 1
        };

        let extracted = (u64_val >> bit_start) & mask;
        extracted as usize
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

#[derive(Debug)]
pub struct HashStore<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    data: Arc<RwLock<Vec<Option<Box<HashLL>>>>>,
    num_elements: Arc<RwLock<usize>>,
    buckets_filled: Arc<RwLock<usize>>,
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> HashStore<INDEX_SIZE, PREFIX_SIZE> {
    pub fn new() -> Self {
        let total_buckets = 1 << INDEX_SIZE;
        Self {
            data: Arc::new(RwLock::new(vec![None; total_buckets])),
            num_elements: Arc::new(RwLock::new(0)),
            buckets_filled: Arc::new(RwLock::new(0)),
        }
    }

    pub fn add_hash(&self, hash: Hash512) -> bool {
        let index = hash.to_index(PREFIX_SIZE, INDEX_SIZE);
        let mut data = self.data.write().unwrap();

        if data[index].is_none() {
            // Add hash to new bucket
            data[index] = Some(Box::new(HashLL::new(hash, None)));
            *self.buckets_filled.write().unwrap() += 1;
            *self.num_elements.write().unwrap() += 1;
            return true;
        }

        // Check if hash already exists and find insertion point
        {
            let bucket = data[index].as_ref().unwrap();
            if hash == bucket.hash {
                return false; // Hash already exists
            }

            if hash < bucket.hash {
                // Insert at the front
                let old_bucket = data[index].take().unwrap();
                data[index] = Some(Box::new(HashLL::new(hash, Some(old_bucket))));
                *self.num_elements.write().unwrap() += 1;
                return true;
            }
        }

        // Traverse the linked list to find the correct insertion point
        let bucket = data[index].as_mut().unwrap();
        let mut current = bucket;

        loop {
            if let Some(next_node) = &current.next {
                if hash == next_node.hash {
                    return false; // Hash already exists
                }
                if hash < next_node.hash {
                    // Insert between current and next
                    let old_next = current.next.take();
                    current.next = Some(Box::new(HashLL::new(hash, old_next)));
                    *self.num_elements.write().unwrap() += 1;
                    return true;
                }
                // Move to next node
                current = current.next.as_mut().unwrap();
            } else {
                // Insert at the end
                current.next = Some(Box::new(HashLL::new(hash, None)));
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
        let index = hash.to_index(PREFIX_SIZE, INDEX_SIZE);
        let data = self.data.read().unwrap();

        if let Some(node) = &data[index] {
            let mut current = node;
            loop {
                if current.hash == *hash {
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

    pub fn to_array(&self) -> HashArray {
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

        HashArray { data: hashes }
    }
}

#[derive(Debug)]
pub struct MultiThreadedHashStore<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    threads: Vec<Sender<HashCommand>>,
}

#[derive(Debug)]
enum HashCommand {
    AddHash(Hash512),
    Contains(Hash512, Sender<bool>),
    GetArray(Sender<HashArray>),
    GetLen(Sender<usize>),
    GetOccupiedSlots(Sender<usize>),
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> MultiThreadedHashStore<INDEX_SIZE, PREFIX_SIZE> {
    pub fn new(num_threads: usize) -> Self {
        // Ensure num_threads is a power of 2
        if !num_threads.is_power_of_two() {
            panic!("Number of threads must be a power of 2");
        }
        let mut threads = Vec::new();

        for _ in 0..num_threads {
            let (tx, rx) = channel();
            threads.push(tx);

            let store = HashStore::<INDEX_SIZE, PREFIX_SIZE>::new();

            thread::spawn(move || {
                Self::hash_store_worker(store, rx);
            });
        }

        Self {
            threads,
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

    pub fn to_array(&self) -> HashArray {
        let mut all_hashes = Vec::new();

        // Collect arrays from all threads
        for tx in &self.threads {
            let (response_tx, response_rx) = channel();
            let _ = tx.send(HashCommand::GetArray(response_tx));
            if let Ok(array) = response_rx.recv() {
                all_hashes.extend(array.data);
            }
        }

        HashArray { data: all_hashes }
    }
}

#[derive(Debug, Clone)]
pub struct HashArray {
    pub data: Vec<Hash512>,
}

impl HashArray {
    pub fn to_merkle_tree(&self) -> MerkleTree {
        if self.data.is_empty() {
            return MerkleTree::new(0);
        }

        let n = self.data.len();
        let depth = (n as f64).log2().ceil() as usize;
        let tree_size = (1 << (depth + 1)) - 1;

        let mut tree_data = vec![[0u64; 8]; tree_size];

        // Copy data to leaves (rightmost part of the tree)
        let leaf_start = (1 << depth) - 1;
        tree_data[leaf_start..leaf_start + n].copy_from_slice(&self.data[..n]);

        // Build tree from bottom up
        for level in (0..depth).rev() {
            let level_start = (1 << level) - 1;
            let child_level_start = (1 << (level + 1)) - 1;

            for i in 0..(1 << level) {
                let parent_idx = level_start + i;
                let left_child_idx = child_level_start + 2 * i;
                let right_child_idx = child_level_start + 2 * i + 1;

                let mut hasher = Sha512::new();
                hasher.update(&tree_data[left_child_idx].to_bytes());
                hasher.update(&tree_data[right_child_idx].to_bytes());
                let result = hasher.finalize();

                // Convert result back to [u64; 8]
                let mut parent_hash = [0u64; 8];
                for i in 0..8 {
                    let start = i * 8;
                    parent_hash[i] = u64::from_le_bytes([
                        result[start], result[start + 1], result[start + 2], result[start + 3],
                        result[start + 4], result[start + 5], result[start + 6], result[start + 7]
                    ]);
                }
                tree_data[parent_idx] = parent_hash;
            }
        }

        MerkleTree {
            data: tree_data,
            depth,
            leaf_count: n,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MerkleTree {
    pub data: Vec<Hash512>,
    pub depth: usize,
    pub leaf_count: usize,
}

impl MerkleTree {
    pub fn new(depth: usize) -> Self {
        let size = if depth == 0 { 0 } else { (1 << (depth + 1)) - 1 };
        Self {
            data: vec![[0u64; 8]; size],
            depth,
            leaf_count: 0,
        }
    }

    pub fn get(&self, hash: &Hash512) -> Option<Vec<(Hash512, Hash512)>> {
        if self.leaf_count == 0 {
            return None;
        }

        // Find the hash in the leaves
        let leaf_start = (1 << self.depth) - 1;
        let mut hash_idx = None;

        for i in 0..self.leaf_count {
            if self.data[leaf_start + i] == *hash {
                hash_idx = Some(i);
                break;
            }
        }

        let hash_idx = hash_idx?;

        // Generate proof path from leaf to root
        let mut proof = Vec::new();
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
        Self {
            hash_store: Arc::new(MultiThreadedHashStore::new(num_threads)),
            merkle_tree: Arc::new(RwLock::new(None)),
            last_tree_update: Arc::new(RwLock::new(None)),
        }
    }

    pub fn update_merkle_tree(&self) {
        let hash_array = self.hash_store.to_array();
        let new_tree = hash_array.to_merkle_tree();

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
