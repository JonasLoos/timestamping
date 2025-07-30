# Hash Storage Implementation

This project implements a configurable hash storage system in Rust, designed to efficiently store and retrieve 512-bit hashes using a hash table with linked list collision resolution.

## Data Structures

### HashLL (Linked List Node)
- Stores a single 512-bit hash
- Contains a reference to the next node in the linked list
- Used for collision resolution in the hash table

### HashArray
- A dynamic array of 512-bit hashes
- Provides methods for adding, accessing, and iterating over hashes
- Used as output format for converting the hash store to a simple array

### HashStore<INDEX_SIZE, PREFIX_SIZE>
- Main storage structure with configurable parameters
- Uses a hash table with 2^INDEX_SIZE buckets
- Extracts INDEX_SIZE bits starting from PREFIX_SIZE position in the hash
- Handles collisions using linked lists (HashLL nodes)
- Thread-safe with interior mutability using Mutex

## Features

- **Configurable**: INDEX_SIZE and PREFIX_SIZE can be adjusted for different use cases
- **Thread-safe**: Uses Mutex for concurrent access
- **Efficient**: O(1) average insertion and lookup time
- **Collision handling**: Uses linked lists for hash collisions
- **Memory efficient**: Only allocates space for actual data
- **Type safety**: Strongly typed with Rust's type system

## Usage

### Basic Usage

```rust
use timestamping::storage::{HashStore, Hash512};

// Create a hash store with 16-bit index (65536 buckets) and 0-bit prefix
let store = HashStore::<16, 0>::new();

// Create a 512-bit hash
let hash: Hash512 = [1u8; 64];

// Add hash to store
store.add_hash(hash);

// Check if hash exists
assert!(store.contains(&hash));

// Get number of elements
println!("Store size: {}", store.len());

// Convert to array
let array = store.to_array();
println!("Array size: {}", array.len());
```

### Configuration Options

- **INDEX_SIZE**: Number of bits used for indexing (1-64)
  - Larger values = more buckets = fewer collisions
  - Memory usage: 2^INDEX_SIZE * sizeof(pointer)
- **PREFIX_SIZE**: Number of bits to skip before indexing
  - Useful for ignoring certain bits in the hash
  - Must satisfy: PREFIX_SIZE + INDEX_SIZE ≤ 64

### Example Configurations

```rust
// Small store for testing (256 buckets)
let small_store = HashStore::<8, 0>::new();

// Medium store for typical use (65536 buckets)
let medium_store = HashStore::<16, 0>::new();

// Large store for high-performance (16777216 buckets)
let large_store = HashStore::<24, 0>::new();

// Store ignoring first 8 bits
let prefix_store = HashStore::<16, 8>::new();
```

## API Reference

### HashStore Methods

- `new()` - Create a new hash store
- `add_hash(hash: Hash512)` - Add a hash to the store
- `contains(hash: &Hash512) -> bool` - Check if a hash exists
- `len() -> usize` - Get the number of elements
- `is_empty() -> bool` - Check if the store is empty
- `to_array() -> HashArray` - Convert to array format
- `to_vec() -> Vec<Hash512>` - Convert to vector

### HashArray Methods

- `new(capacity: usize)` - Create a new array with given capacity
- `len() -> usize` - Get the number of elements
- `is_empty() -> bool` - Check if the array is empty
- `push(hash: Hash512)` - Add a hash to the array
- `get(index: usize) -> Option<&Hash512>` - Get hash at index

## Web Server

The project includes a web server that accepts POST requests to add hashes:

```bash
# Start the server
cargo run

# Add a hash via HTTP
curl -X POST http://localhost:3427/add \
  -H "Content-Type: application/json" \
  -d '{"hash": "0123456789abcdef..."}'
```

The hash must be exactly 128 hexadecimal characters (512 bits).

## Testing

Run the test suite:

```bash
cargo test
```

Tests cover:
- Basic functionality (add, contains, len)
- Collision handling
- Array conversion
- Edge cases and error conditions

## Performance Characteristics

- **Insertion**: O(1) average, O(n) worst case (all collisions)
- **Lookup**: O(1) average, O(n) worst case (all collisions)
- **Memory**: O(n) where n is the number of stored hashes
- **Space overhead**: Minimal - only stores actual data plus pointers

## Thread Safety

The implementation is thread-safe using Rust's `Mutex` for interior mutability. Multiple threads can safely:
- Add hashes concurrently
- Check for hash existence
- Read the store size
- Convert to array (creates a snapshot)

## License

This project is open source and available under the MIT License.
