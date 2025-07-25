use std::sync::Mutex;

/// A 512-bit hash represented as 64 bytes
pub type Hash512 = [u8; 64];

/// Linked List node for storing hashes
pub struct HashLL {
    pub hash: Hash512,
    pub next: Option<Box<HashLL>>,
}

impl HashLL {
    pub fn new(hash: Hash512) -> Self {
        Self {
            hash,
            next: None,
        }
    }
}

/// Fixed-size array of hashes
pub struct HashArray {
    pub data: Vec<Hash512>,
}

impl HashArray {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn push(&mut self, hash: Hash512) {
        self.data.push(hash);
    }

    pub fn get(&self, index: usize) -> Option<&Hash512> {
        self.data.get(index)
    }
}

/// Main hash storage structure with configurable index and prefix sizes
pub struct HashStore<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> {
    data: Mutex<Vec<Option<Box<HashLL>>>>,
    num_elements: Mutex<usize>,
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> HashStore<INDEX_SIZE, PREFIX_SIZE> {
    /// Create a new HashStore with the specified sizes
    ///
    /// # Panics
    ///
    /// Panics if INDEX_SIZE is greater than 64 (since we're working with 512-bit hashes)
    pub fn new() -> Self {
        assert!(INDEX_SIZE <= 64, "INDEX_SIZE cannot be greater than 64");
        assert!(PREFIX_SIZE + INDEX_SIZE <= 64, "PREFIX_SIZE + INDEX_SIZE cannot exceed 64");

        let capacity = 1 << INDEX_SIZE;
        let mut data = Vec::with_capacity(capacity);
        data.resize_with(capacity, || None);

        Self {
            data: Mutex::new(data),
            num_elements: Mutex::new(0),
        }
    }

    /// Add a hash to the store
    pub fn add_hash(&self, hash: Hash512) {
        let index = self.get_index(&hash);

        // Create new linked list node
        let new_node = Box::new(HashLL::new(hash));

        // Lock the data mutex and insert at the beginning of the linked list for O(1) insertion
        let mut data = self.data.lock().unwrap();
        let mut new_node = new_node;

        // Move the existing list to the new node's next pointer
        new_node.next = data[index].take();

        // Set the new node as the head of the list
        data[index] = Some(new_node);

        // Increment element count
        let mut count = self.num_elements.lock().unwrap();
        *count += 1;
    }

    /// Get the number of elements in the store
    pub fn len(&self) -> usize {
        *self.num_elements.lock().unwrap()
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert the store to a HashArray
    pub fn to_array(&self) -> HashArray {
        let num_elements = self.len();
        let mut hash_array = HashArray::new(num_elements);

        let data = self.data.lock().unwrap();
        for bucket in data.iter() {
            let mut current = bucket.as_ref();
            while let Some(node) = current {
                hash_array.push(node.hash);
                current = node.next.as_ref();
            }
        }

        hash_array
    }

    /// Get all hashes as a vector
    pub fn to_vec(&self) -> Vec<Hash512> {
        self.to_array().data
    }

    /// Check if a hash exists in the store
    pub fn contains(&self, hash: &Hash512) -> bool {
        let index = self.get_index(hash);

        let data = self.data.lock().unwrap();
        let mut current = &data[index];
        while let Some(node) = current {
            if &node.hash == hash {
                return true;
            }
            current = &node.next;
        }

        false
    }

    /// Get the index for a hash based on the PREFIX_SIZE and INDEX_SIZE configuration
    fn get_index(&self, hash: &Hash512) -> usize {
        // Extract the relevant bits from the hash
        // We'll use the first INDEX_SIZE bits starting from PREFIX_SIZE
        let mut index = 0usize;

        // Calculate which byte we start from
        let start_byte = PREFIX_SIZE / 8;
        let start_bit = PREFIX_SIZE % 8;

        // Calculate how many bytes we need
        let num_bytes = (INDEX_SIZE + 7) / 8; // Ceiling division

        for i in 0..num_bytes {
            let byte_index = start_byte + i;
            if byte_index >= 64 {
                break;
            }

            let mut byte = hash[byte_index];

            // Handle bit alignment
            if i == 0 && start_bit > 0 {
                byte >>= start_bit;
            }

            // Only take the bits we need
            let bits_to_take = if i == num_bytes - 1 {
                let remaining_bits = INDEX_SIZE - (i * 8);
                if remaining_bits < 8 {
                    remaining_bits
                } else {
                    8
                }
            } else {
                8
            };

            let mask = if bits_to_take == 8 { 0xFF } else { (1 << bits_to_take) - 1 };
            byte &= mask;

            index |= (byte as usize) << (i * 8);
        }

        index % (1 << INDEX_SIZE)
    }
}

// Default implementation for common use cases
impl HashStore<16, 0> {
    /// Create a default HashStore with 16-bit index and 0-bit prefix
    pub fn new_default() -> Self {
        Self::new()
    }
}

// Implement Clone for HashArray
impl Clone for HashArray {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
        }
    }
}

// Implement Debug for all types
impl std::fmt::Debug for HashLL {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashLL")
            .field("hash", &hex::encode(self.hash))
            .field("next", &self.next.is_some())
            .finish()
    }
}

impl std::fmt::Debug for HashArray {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashArray")
            .field("len", &self.len())
            .field("data", &self.data.iter().map(|h| hex::encode(h)).collect::<Vec<_>>())
            .finish()
    }
}

impl<const INDEX_SIZE: usize, const PREFIX_SIZE: usize> std::fmt::Debug for HashStore<INDEX_SIZE, PREFIX_SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashStore")
            .field("index_size", &INDEX_SIZE)
            .field("prefix_size", &PREFIX_SIZE)
            .field("capacity", &(1 << INDEX_SIZE))
            .field("num_elements", &self.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_ll() {
        let hash = [1u8; 64];
        let node = HashLL::new(hash);
        assert_eq!(node.hash, hash);
        assert!(node.next.is_none());
    }

    #[test]
    fn test_hash_array() {
        let mut array = HashArray::new(10);
        assert!(array.is_empty());

        let hash = [1u8; 64];
        array.push(hash);
        assert_eq!(array.len(), 1);
        assert_eq!(array.get(0), Some(&hash));
    }

    #[test]
    fn test_hash_store_basic() {
        let store = HashStore::<4, 0>::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_hash_store_add_and_contains() {
        let store = HashStore::<4, 0>::new();
        let hash = [1u8; 64];

        assert!(!store.contains(&hash));
        store.add_hash(hash);
        assert!(store.contains(&hash));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_hash_store_to_array() {
        let store = HashStore::<4, 0>::new();
        let hash1 = [1u8; 64];
        let hash2 = [2u8; 64];

        store.add_hash(hash1);
        store.add_hash(hash2);

        let array = store.to_array();
        assert_eq!(array.len(), 2);
        assert!(array.data.contains(&hash1));
        assert!(array.data.contains(&hash2));
    }

    #[test]
    fn test_hash_store_collision() {
        let store = HashStore::<1, 0>::new(); // Only 2 buckets, will cause collisions
        let hash1 = [1u8; 64];
        let hash2 = [2u8; 64];
        let hash3 = [3u8; 64];

        store.add_hash(hash1);
        store.add_hash(hash2);
        store.add_hash(hash3);

        assert_eq!(store.len(), 3);
        assert!(store.contains(&hash1));
        assert!(store.contains(&hash2));
        assert!(store.contains(&hash3));
    }
}
