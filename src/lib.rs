pub mod storage;

#[cfg(test)]
mod tests {
    use crate::storage::{Hash512, Hash512Ops};

    #[test]
    fn test_hash512_bytes_conversion() {
        // Create a test hash
        let original_hash: Hash512 = [1u64; 8];

        // Test to_bytes
        let bytes = original_hash.to_bytes();
        assert_eq!(bytes.len(), 64);

        // Test from_bytes
        let decoded_hash = Hash512::from_bytes(&bytes).unwrap();
        assert_eq!(original_hash, decoded_hash);
    }

    #[test]
    fn test_hash512_to_index() {
        // Create a test hash with known pattern
        let mut hash: Hash512 = [0u64; 8];
        hash[0] = 0b1111000010101010; // Set different patterns in different 4-bit sections

        // Test to_index with different parameters (all within first u64)
        let index1 = hash.to_index(0, 8);
        let index2 = hash.to_index(0, 4);
        let index3 = hash.to_index(8, 4); // Extract from bit position 8-11

        // These should be different since we're extracting different bits
        assert_ne!(index1, index2);
        assert_ne!(index2, index3);

        // Test that index is within expected bounds
        assert!(index1 < (1 << 8));
        assert!(index2 < (1 << 4));
        assert!(index3 < (1 << 4));
    }

    #[test]
    fn test_hash512_error_handling() {
        // Test invalid bytes length
        let invalid_bytes = vec![1, 2, 3]; // Too short
        let result = Hash512::from_bytes(&invalid_bytes);
        assert!(result.is_err());

        // Test empty bytes
        let result = Hash512::from_bytes(&[]);
        assert!(result.is_err());

        // Test correct length
        let valid_bytes = vec![0u8; 64];
        let result = Hash512::from_bytes(&valid_bytes);
        assert!(result.is_ok());
    }
}
