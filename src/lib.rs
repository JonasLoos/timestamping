pub mod storage;

#[cfg(test)]
mod tests {
    use crate::storage::{Hash512, Hash512Ops};

    #[test]
    fn test_hash512_base64_conversion() {
        // Create a test hash
        let original_hash: Hash512 = [1u64; 8];

        // Test to_base64
        let base64_str = original_hash.to_base64();
        assert!(!base64_str.is_empty());

        // Test from_base64
        let decoded_hash = Hash512::from_base64(&base64_str).unwrap();
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
        // Test invalid base64 string
        let result = Hash512::from_base64("invalid_base64_string");
        assert!(result.is_err());

        // Test empty string
        let result = Hash512::from_base64("");
        assert!(result.is_err());
    }
}
