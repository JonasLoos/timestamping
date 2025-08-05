use std::time::Instant;
use rand::Rng;
use timestamping::storage::{HashStore, Hash512};

static SALT: Hash512 = [0, 0, 0, 0, 0, 0, 0, 0];

// Generate a random 512-bit hash
fn generate_random_hash() -> Hash512 {
    let mut rng = rand::thread_rng();
    let mut hash = [0u64; 8];
    rng.fill(&mut hash);
    hash
}

// Generate a vector of random hashes
fn generate_random_hashes(count: usize) -> Vec<Hash512> {
    (0..count).map(|_| generate_random_hash()).collect()
}

// Benchmark hash insertion speed
fn benchmark_insertion_speed() {
    println!("=== Hash Insertion Speed Benchmark ===");
    println!("Testing different store configurations and hash counts\n");

    let test_sizes = vec![
        10_000,      // 10K hashes
        100_000,     // 100K hashes
        500_000,     // 500K hashes
        1_000_000,   // 1M hashes
        2_000_000,   // 2M hashes
    ];

    for size in test_sizes {
        println!("Testing with {} hashes:", size);
        let hashes = generate_random_hashes(size);

        // Test 16-bit index (65,536 buckets)
        let store_16 = HashStore::<16, 0>::new(SALT);
        let start = Instant::now();
        for hash in &hashes {
            store_16.add_hash(*hash);
        }
        let duration_16 = start.elapsed();
        let hashes_per_second_16 = size as f64 / duration_16.as_secs_f64();
        println!("  16-bit index: {:.2} hashes/sec ({:.2?})", hashes_per_second_16, duration_16);

        // Test 20-bit index (1,048,576 buckets)
        let store_20 = HashStore::<20, 0>::new(SALT);
        let start = Instant::now();
        for hash in &hashes {
            store_20.add_hash(*hash);
        }
        let duration_20 = start.elapsed();
        let hashes_per_second_20 = size as f64 / duration_20.as_secs_f64();
        println!("  20-bit index: {:.2} hashes/sec ({:.2?})", hashes_per_second_20, duration_20);

        // Test 24-bit index (16,777,216 buckets)
        let store_24 = HashStore::<24, 0>::new(SALT);
        let start = Instant::now();
        for hash in &hashes {
            store_24.add_hash(*hash);
        }
        let duration_24 = start.elapsed();
        let hashes_per_second_24 = size as f64 / duration_24.as_secs_f64();
        println!("  24-bit index: {:.2} hashes/sec ({:.2?})", hashes_per_second_24, duration_24);

        println!();
    }
}

// Benchmark lookup performance
fn benchmark_lookup_performance() {
    println!("=== Lookup Performance Benchmark ===");
    println!("Testing hash lookup speed after insertion\n");

    let insert_count = 100_000;
    let lookup_count = 10_000;

    // Insert hashes
    let store = HashStore::<16, 0>::new(SALT);
    let hashes = generate_random_hashes(insert_count);
    for hash in &hashes {
        store.add_hash(*hash);
    }

    // Generate lookup hashes (mix of existing and non-existing)
    let mut lookup_hashes = Vec::new();
    let mut rng = rand::thread_rng();
    let mut num_existing = 0;

    for _ in 0..lookup_count {
        if rng.gen_bool(0.5) {
            // 50% chance to lookup existing hash
            lookup_hashes.push(hashes[rng.gen_range(0..insert_count)]);
            num_existing += 1;
        } else {
            // 50% chance to lookup non-existing hash
            lookup_hashes.push(generate_random_hash());
        }
    }

    // Benchmark lookups
    let start = Instant::now();
    let mut found_count = 0;
    for hash in &lookup_hashes {
        if store.contains(hash) {
            found_count += 1;
        }
    }
    let duration = start.elapsed();
    let lookups_per_second = lookup_count as f64 / duration.as_secs_f64();
    assert_eq!(num_existing, found_count);

    println!("Lookup performance: {:.2} lookups/sec ({:.2?})", lookups_per_second, duration);
    println!();
}

fn main() {
    println!("Hash Store Performance Benchmark");
    println!("================================\n");

    benchmark_insertion_speed();
    benchmark_lookup_performance();

    println!("Benchmark completed!");
}
