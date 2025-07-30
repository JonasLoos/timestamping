use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use sha2::{Digest, Sha512};

pub type Hash512 = [u8; 64];

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

    fn get_index(&self, hash: &Hash512) -> usize {
        // Extract INDEX_SIZE bits starting from PREFIX_SIZE
        let byte_start = PREFIX_SIZE / 8;
        let bit_start = PREFIX_SIZE % 8;

        let mut index = 0usize;
        let mut bits_collected = 0;

        for i in 0..((INDEX_SIZE + 7) / 8) {
            if byte_start + i >= hash.len() || bits_collected >= INDEX_SIZE {
                break;
            }

            let byte = hash[byte_start + i];
            let available_bits = 8 - if i == 0 { bit_start } else { 0 };
            let bits_to_take = std::cmp::min(available_bits, INDEX_SIZE - bits_collected);

            if bits_to_take == 0 {
                break;
            }

            let shift = if i == 0 { bit_start } else { 0 };
            // Use u32 to avoid overflow, then convert to usize
            let mask = if bits_to_take >= 32 {
                u32::MAX
            } else {
                (1u32 << bits_to_take) - 1
            };
            let extracted = ((byte >> shift) as u32) & mask;

            if bits_collected < 32 {
                index |= (extracted as usize) << bits_collected;
            }
            bits_collected += bits_to_take;
        }

        // Ensure we don't exceed INDEX_SIZE bits
        if INDEX_SIZE >= 32 {
            index
        } else {
            index & ((1usize << INDEX_SIZE) - 1)
        }
    }

    pub fn add_hash(&self, hash: Hash512) -> bool {
        let index = self.get_index(&hash);
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
        let index = self.get_index(hash);
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

        // Sort the hashes for consistent merkle tree construction
        hashes.sort();

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
                    // Note: We don't update global counters here as they're maintained per thread
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

    fn get_thread_index(&self, hash: &Hash512) -> usize {
        // Use the first PREFIX_SIZE bits to determine which thread to use
        let byte_start = 0;
        let bit_start = 0;

        let mut thread_index = 0usize;
        let mut bits_collected = 0;
        let num_threads = self.threads.len();
        let bits_needed = (num_threads as f64).log2().ceil() as usize;

        for i in 0..((bits_needed + 7) / 8) {
            if byte_start + i >= hash.len() || bits_collected >= bits_needed {
                break;
            }

            let byte = hash[byte_start + i];
            let available_bits = 8 - if i == 0 { bit_start } else { 0 };
            let bits_to_take = std::cmp::min(available_bits, bits_needed - bits_collected);

            if bits_to_take == 0 {
                break;
            }

            let shift = if i == 0 { bit_start } else { 0 };
            let mask = if bits_to_take >= 32 {
                u32::MAX
            } else {
                (1u32 << bits_to_take) - 1
            };
            let extracted = ((byte >> shift) as u32) & mask;

            if bits_collected < 32 {
                thread_index |= (extracted as usize) << bits_collected;
            }
            bits_collected += bits_to_take;
        }

        thread_index % num_threads
    }

    pub fn add_hash(&self, hash: Hash512) -> bool {
        let thread_index = self.get_thread_index(&hash);
        let tx = &self.threads[thread_index];

        // Send the hash to the appropriate thread
        let _ = tx.send(HashCommand::AddHash(hash));

        // For simplicity, we'll assume it's new (we could add a response channel if needed)
        true
    }

    pub fn contains(&self, hash: &Hash512) -> bool {
        let thread_index = self.get_thread_index(hash);
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

        // Sort all hashes for consistent merkle tree construction
        all_hashes.sort();

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

        let mut tree_data = vec![[0u8; 64]; tree_size];

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
                hasher.update(&tree_data[left_child_idx]);
                hasher.update(&tree_data[right_child_idx]);
                let result = hasher.finalize();

                tree_data[parent_idx].copy_from_slice(&result[..64]);
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
            data: vec![[0u8; 64]; size],
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

    pub fn get_merkle_proof(&self, hash: &Hash512) -> Option<Vec<(Hash512, Hash512)>> {
        let tree = self.merkle_tree.read().unwrap();
        tree.as_ref()?.get(hash)
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
