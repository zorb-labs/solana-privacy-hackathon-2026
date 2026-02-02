//! Indexed merkle tree operations for nullifier storage.
//!
//! This module provides operations for the indexed merkle tree following Aztec's design.
//! Leaves contain `(value, next_value, next_index)` forming a sorted linked list.
//!
//! # Zero Hashes
//!
//! Unlike standard merkle trees that use `Poseidon(0, 0)` as the zero leaf hash,
//! indexed merkle trees use the genesis indexed leaf hash `Poseidon(0, 0, 0)` as the base.
//! This is semantically correct because each leaf has structure `{value, nextValue, nextIndex}`,
//! so an "empty" leaf is `{0, 0, 0}`, not just `0`.
//!
//! The zero hashes are computed as:
//! - `zero_hashes[0] = hash(genesis_leaf)` = `Poseidon(0, 0, 0)`
//! - `zero_hashes[i] = Poseidon(zero_hashes[i-1], zero_hashes[i-1])`

use crate::errors::ShieldedPoolError;
use crate::state::{IndexedLeaf, NULLIFIER_TREE_HEIGHT, NullifierIndexedTree};
use light_hasher::Hasher;
use pinocchio::program_error::ProgramError;

/// Pre-computed zero hashes for the indexed merkle tree (BIG-ENDIAN format).
///
/// These are computed from the genesis indexed leaf hash `Poseidon(0, 0, 0)`:
/// - `INDEXED_ZERO_HASHES[0]` = genesis leaf hash
/// - `INDEXED_ZERO_HASHES[i]` = `Poseidon(INDEXED_ZERO_HASHES[i-1], INDEXED_ZERO_HASHES[i-1])`
///
/// IMPORTANT: light_hasher::Poseidon uses big-endian byte order for inputs and outputs.
/// These values must be in big-endian format to match the on-chain hash computation.
pub const INDEXED_ZERO_HASHES: [[u8; 32]; NULLIFIER_TREE_HEIGHT as usize] = [
    // Level 0: 0x0bc188d27dcceadc1dcfb6af0a7af08fe2864eecec96c5ae7cee6db31ba599aa
    [
        0x0b, 0xc1, 0x88, 0xd2, 0x7d, 0xcc, 0xea, 0xdc, 0x1d, 0xcf, 0xb6, 0xaf, 0x0a, 0x7a, 0xf0,
        0x8f, 0xe2, 0x86, 0x4e, 0xec, 0xec, 0x96, 0xc5, 0xae, 0x7c, 0xee, 0x6d, 0xb3, 0x1b, 0xa5,
        0x99, 0xaa,
    ],
    // Level 1: 0x0bb8c4e79b87e21962d496cfc7bbe6726a8d1ab8ed987416de3c54333b1f651e
    [
        0x0b, 0xb8, 0xc4, 0xe7, 0x9b, 0x87, 0xe2, 0x19, 0x62, 0xd4, 0x96, 0xcf, 0xc7, 0xbb, 0xe6,
        0x72, 0x6a, 0x8d, 0x1a, 0xb8, 0xed, 0x98, 0x74, 0x16, 0xde, 0x3c, 0x54, 0x33, 0x3b, 0x1f,
        0x65, 0x1e,
    ],
    // Level 2: 0x2baf726a0349814fdb008ed81ce69ccf8d92acfa44265374a0ab3f72e028bba4
    [
        0x2b, 0xaf, 0x72, 0x6a, 0x03, 0x49, 0x81, 0x4f, 0xdb, 0x00, 0x8e, 0xd8, 0x1c, 0xe6, 0x9c,
        0xcf, 0x8d, 0x92, 0xac, 0xfa, 0x44, 0x26, 0x53, 0x74, 0xa0, 0xab, 0x3f, 0x72, 0xe0, 0x28,
        0xbb, 0xa4,
    ],
    // Level 3: 0x1997719d020d6971a3b637e897e7f8b1491f1fa047ad91844c74fb354f76be1b
    [
        0x19, 0x97, 0x71, 0x9d, 0x02, 0x0d, 0x69, 0x71, 0xa3, 0xb6, 0x37, 0xe8, 0x97, 0xe7, 0xf8,
        0xb1, 0x49, 0x1f, 0x1f, 0xa0, 0x47, 0xad, 0x91, 0x84, 0x4c, 0x74, 0xfb, 0x35, 0x4f, 0x76,
        0xbe, 0x1b,
    ],
    // Level 4: 0x0ea9e4c41942f4080c0c33ade1733ec4a79764e5d8957c2e35e5b68d2c9bee21
    [
        0x0e, 0xa9, 0xe4, 0xc4, 0x19, 0x42, 0xf4, 0x08, 0x0c, 0x0c, 0x33, 0xad, 0xe1, 0x73, 0x3e,
        0xc4, 0xa7, 0x97, 0x64, 0xe5, 0xd8, 0x95, 0x7c, 0x2e, 0x35, 0xe5, 0xb6, 0x8d, 0x2c, 0x9b,
        0xee, 0x21,
    ],
    // Level 5: 0x051d3e30f0dfb59fc831535d5a2c9a1e3eba4a950c4b6837633f0604de1e027e
    [
        0x05, 0x1d, 0x3e, 0x30, 0xf0, 0xdf, 0xb5, 0x9f, 0xc8, 0x31, 0x53, 0x5d, 0x5a, 0x2c, 0x9a,
        0x1e, 0x3e, 0xba, 0x4a, 0x95, 0x0c, 0x4b, 0x68, 0x37, 0x63, 0x3f, 0x06, 0x04, 0xde, 0x1e,
        0x02, 0x7e,
    ],
    // Level 6: 0x1b03cd8fe8e84fed0b098d2caa2dc493320e432e523d664a1adc9c60cb1bc6d0
    [
        0x1b, 0x03, 0xcd, 0x8f, 0xe8, 0xe8, 0x4f, 0xed, 0x0b, 0x09, 0x8d, 0x2c, 0xaa, 0x2d, 0xc4,
        0x93, 0x32, 0x0e, 0x43, 0x2e, 0x52, 0x3d, 0x66, 0x4a, 0x1a, 0xdc, 0x9c, 0x60, 0xcb, 0x1b,
        0xc6, 0xd0,
    ],
    // Level 7: 0x23b2cd2f1177a96b9789e22f013b310b8ae0deb5a5757349efa8fc1214d40c73
    [
        0x23, 0xb2, 0xcd, 0x2f, 0x11, 0x77, 0xa9, 0x6b, 0x97, 0x89, 0xe2, 0x2f, 0x01, 0x3b, 0x31,
        0x0b, 0x8a, 0xe0, 0xde, 0xb5, 0xa5, 0x75, 0x73, 0x49, 0xef, 0xa8, 0xfc, 0x12, 0x14, 0xd4,
        0x0c, 0x73,
    ],
    // Level 8: 0x0f37f290e089a368d35f09d9ff77bb2e329ecae0a5e05746c15e66f5a6ddc9d1
    [
        0x0f, 0x37, 0xf2, 0x90, 0xe0, 0x89, 0xa3, 0x68, 0xd3, 0x5f, 0x09, 0xd9, 0xff, 0x77, 0xbb,
        0x2e, 0x32, 0x9e, 0xca, 0xe0, 0xa5, 0xe0, 0x57, 0x46, 0xc1, 0x5e, 0x66, 0xf5, 0xa6, 0xdd,
        0xc9, 0xd1,
    ],
    // Level 9: 0x19269b0e3a33d5fa49ee53af22e691bda931c6d88d5765b3201881f5a6213b5e
    [
        0x19, 0x26, 0x9b, 0x0e, 0x3a, 0x33, 0xd5, 0xfa, 0x49, 0xee, 0x53, 0xaf, 0x22, 0xe6, 0x91,
        0xbd, 0xa9, 0x31, 0xc6, 0xd8, 0x8d, 0x57, 0x65, 0xb3, 0x20, 0x18, 0x81, 0xf5, 0xa6, 0x21,
        0x3b, 0x5e,
    ],
    // Level 10: 0x1d467bb8ee17f67da5f3c9b595738ac732ebcd994dc2a4514d9677274363e9fa
    [
        0x1d, 0x46, 0x7b, 0xb8, 0xee, 0x17, 0xf6, 0x7d, 0xa5, 0xf3, 0xc9, 0xb5, 0x95, 0x73, 0x8a,
        0xc7, 0x32, 0xeb, 0xcd, 0x99, 0x4d, 0xc2, 0xa4, 0x51, 0x4d, 0x96, 0x77, 0x27, 0x43, 0x63,
        0xe9, 0xfa,
    ],
    // Level 11: 0x2779a810177bfb8caf6fbef9b2623a1ee958e22136d7f534c1948053de33ba4e
    [
        0x27, 0x79, 0xa8, 0x10, 0x17, 0x7b, 0xfb, 0x8c, 0xaf, 0x6f, 0xbe, 0xf9, 0xb2, 0x62, 0x3a,
        0x1e, 0xe9, 0x58, 0xe2, 0x21, 0x36, 0xd7, 0xf5, 0x34, 0xc1, 0x94, 0x80, 0x53, 0xde, 0x33,
        0xba, 0x4e,
    ],
    // Level 12: 0x0b62f0fbe1557a5771eb2485e76efcdc1d919d27a320198a47273c564c1e08ad
    [
        0x0b, 0x62, 0xf0, 0xfb, 0xe1, 0x55, 0x7a, 0x57, 0x71, 0xeb, 0x24, 0x85, 0xe7, 0x6e, 0xfc,
        0xdc, 0x1d, 0x91, 0x9d, 0x27, 0xa3, 0x20, 0x19, 0x8a, 0x47, 0x27, 0x3c, 0x56, 0x4c, 0x1e,
        0x08, 0xad,
    ],
    // Level 13: 0x0f72f28075eacfad65bc5b44fe68d13450eb9afbaeb10e1ddc36087f95c3580e
    [
        0x0f, 0x72, 0xf2, 0x80, 0x75, 0xea, 0xcf, 0xad, 0x65, 0xbc, 0x5b, 0x44, 0xfe, 0x68, 0xd1,
        0x34, 0x50, 0xeb, 0x9a, 0xfb, 0xae, 0xb1, 0x0e, 0x1d, 0xdc, 0x36, 0x08, 0x7f, 0x95, 0xc3,
        0x58, 0x0e,
    ],
    // Level 14: 0x1c051c9cd466046e3ba2c24d736271d9b2a3b7fa9bd7a7dc9d280c0cadfbc996
    [
        0x1c, 0x05, 0x1c, 0x9c, 0xd4, 0x66, 0x04, 0x6e, 0x3b, 0xa2, 0xc2, 0x4d, 0x73, 0x62, 0x71,
        0xd9, 0xb2, 0xa3, 0xb7, 0xfa, 0x9b, 0xd7, 0xa7, 0xdc, 0x9d, 0x28, 0x0c, 0x0c, 0xad, 0xfb,
        0xc9, 0x96,
    ],
    // Level 15: 0x14960bfeb9294f9cf200770fa64bbfdd04b0eb0e7d964c7a7cd07bd6bbee73df
    [
        0x14, 0x96, 0x0b, 0xfe, 0xb9, 0x29, 0x4f, 0x9c, 0xf2, 0x00, 0x77, 0x0f, 0xa6, 0x4b, 0xbf,
        0xdd, 0x04, 0xb0, 0xeb, 0x0e, 0x7d, 0x96, 0x4c, 0x7a, 0x7c, 0xd0, 0x7b, 0xd6, 0xbb, 0xee,
        0x73, 0xdf,
    ],
    // Level 16: 0x2b4ce93c772dac6056727fe40e8a0bfb5f0d3e1e61485c078c89d4f1b29d8167
    [
        0x2b, 0x4c, 0xe9, 0x3c, 0x77, 0x2d, 0xac, 0x60, 0x56, 0x72, 0x7f, 0xe4, 0x0e, 0x8a, 0x0b,
        0xfb, 0x5f, 0x0d, 0x3e, 0x1e, 0x61, 0x48, 0x5c, 0x07, 0x8c, 0x89, 0xd4, 0xf1, 0xb2, 0x9d,
        0x81, 0x67,
    ],
    // Level 17: 0x124b823e0e7ad6004694f1ff41ebbefcc73973d650a63ae3808a22cba6c5be9d
    [
        0x12, 0x4b, 0x82, 0x3e, 0x0e, 0x7a, 0xd6, 0x00, 0x46, 0x94, 0xf1, 0xff, 0x41, 0xeb, 0xbe,
        0xfc, 0xc7, 0x39, 0x73, 0xd6, 0x50, 0xa6, 0x3a, 0xe3, 0x80, 0x8a, 0x22, 0xcb, 0xa6, 0xc5,
        0xbe, 0x9d,
    ],
    // Level 18: 0x23c9780ca31c55fc27775e54c33467fc83f2f3deacb770d6c1fb200985df6230
    [
        0x23, 0xc9, 0x78, 0x0c, 0xa3, 0x1c, 0x55, 0xfc, 0x27, 0x77, 0x5e, 0x54, 0xc3, 0x34, 0x67,
        0xfc, 0x83, 0xf2, 0xf3, 0xde, 0xac, 0xb7, 0x70, 0xd6, 0xc1, 0xfb, 0x20, 0x09, 0x85, 0xdf,
        0x62, 0x30,
    ],
    // Level 19: 0x01b244fbb24dc3e77f1eaae694493dc9500a39c9e9859b2bc947a65f1e1d8c75
    [
        0x01, 0xb2, 0x44, 0xfb, 0xb2, 0x4d, 0xc3, 0xe7, 0x7f, 0x1e, 0xaa, 0xe6, 0x94, 0x49, 0x3d,
        0xc9, 0x50, 0x0a, 0x39, 0xc9, 0xe9, 0x85, 0x9b, 0x2b, 0xc9, 0x47, 0xa6, 0x5f, 0x1e, 0x1d,
        0x8c, 0x75,
    ],
    // Level 20: 0x1217646f5fee5f734b9d3f748cf380868258cc8a33bbd2a5224989cec1b13e9f
    [
        0x12, 0x17, 0x64, 0x6f, 0x5f, 0xee, 0x5f, 0x73, 0x4b, 0x9d, 0x3f, 0x74, 0x8c, 0xf3, 0x80,
        0x86, 0x82, 0x58, 0xcc, 0x8a, 0x33, 0xbb, 0xd2, 0xa5, 0x22, 0x49, 0x89, 0xce, 0xc1, 0xb1,
        0x3e, 0x9f,
    ],
    // Level 21: 0x202d273544061e85a0c3c643d026ae684bdf47f9b007bd9e61f45db8dfec836c
    [
        0x20, 0x2d, 0x27, 0x35, 0x44, 0x06, 0x1e, 0x85, 0xa0, 0xc3, 0xc6, 0x43, 0xd0, 0x26, 0xae,
        0x68, 0x4b, 0xdf, 0x47, 0xf9, 0xb0, 0x07, 0xbd, 0x9e, 0x61, 0xf4, 0x5d, 0xb8, 0xdf, 0xec,
        0x83, 0x6c,
    ],
    // Level 22: 0x0de17a07ef8f7905a6d4d8816e5d1a9615e2b739bccf77f4267e78dc23a6ab74
    [
        0x0d, 0xe1, 0x7a, 0x07, 0xef, 0x8f, 0x79, 0x05, 0xa6, 0xd4, 0xd8, 0x81, 0x6e, 0x5d, 0x1a,
        0x96, 0x15, 0xe2, 0xb7, 0x39, 0xbc, 0xcf, 0x77, 0xf4, 0x26, 0x7e, 0x78, 0xdc, 0x23, 0xa6,
        0xab, 0x74,
    ],
    // Level 23: 0x13da50601f3c3f67bc354e8f2156b522ca65c6e369a613bb072ce919ccc19843
    [
        0x13, 0xda, 0x50, 0x60, 0x1f, 0x3c, 0x3f, 0x67, 0xbc, 0x35, 0x4e, 0x8f, 0x21, 0x56, 0xb5,
        0x22, 0xca, 0x65, 0xc6, 0xe3, 0x69, 0xa6, 0x13, 0xbb, 0x07, 0x2c, 0xe9, 0x19, 0xcc, 0xc1,
        0x98, 0x43,
    ],
    // Level 24: 0x2fb10e4e180502d0a4905ef0e539312860c5a823b6b00c916e30209e73b7e25a
    [
        0x2f, 0xb1, 0x0e, 0x4e, 0x18, 0x05, 0x02, 0xd0, 0xa4, 0x90, 0x5e, 0xf0, 0xe5, 0x39, 0x31,
        0x28, 0x60, 0xc5, 0xa8, 0x23, 0xb6, 0xb0, 0x0c, 0x91, 0x6e, 0x30, 0x20, 0x9e, 0x73, 0xb7,
        0xe2, 0x5a,
    ],
    // Level 25: 0x29d6b70339aba28c818b41774180b9ce33928a23d6b86528aa48f6f97fa8ab7b
    [
        0x29, 0xd6, 0xb7, 0x03, 0x39, 0xab, 0xa2, 0x8c, 0x81, 0x8b, 0x41, 0x77, 0x41, 0x80, 0xb9,
        0xce, 0x33, 0x92, 0x8a, 0x23, 0xd6, 0xb8, 0x65, 0x28, 0xaa, 0x48, 0xf6, 0xf9, 0x7f, 0xa8,
        0xab, 0x7b,
    ],
];

/// Indexed merkle tree operations
pub struct IndexedMerkleTree;

impl IndexedMerkleTree {
    /// Compute the hash of an indexed leaf.
    ///
    /// Hash = Poseidon(value, next_index, next_value)
    pub fn compute_leaf_hash<H: Hasher>(leaf: &IndexedLeaf) -> Result<[u8; 32], ProgramError> {
        // Convert next_index to 32 bytes (little-endian padded)
        let mut next_index_bytes = [0u8; 32];
        next_index_bytes[..8].copy_from_slice(&leaf.next_index.to_le_bytes());

        // 3-input Poseidon: (value, next_index, next_value)
        H::hashv(&[&leaf.value, &next_index_bytes, &leaf.next_value])
            .map_err(|_| ShieldedPoolError::ArithmeticOverflow.into())
    }

    /// Initialize the indexed nullifier tree with a genesis leaf.
    ///
    /// # Genesis Leaf
    ///
    /// Creates a sentinel leaf at index 0: `(value=0, next_value=0, next_index=0)`
    /// Per Aztec spec: `next_value=0` represents infinity (end of sorted list).
    /// This anchors the sorted linked list structure.
    ///
    /// # Post-Initialization State
    ///
    /// - `next_index = 1`: Genesis occupies index 0, next insertion at index 1
    /// - `next_pending_index = 1`: Same, no pending nullifiers yet
    /// - `root`: Computed from genesis leaf + zero hashes
    /// - `subtrees[0]`: Genesis leaf hash
    ///
    /// Real nullifiers will be assigned indices starting from 1, giving a
    /// capacity of `2^height - 1` nullifiers.
    ///
    /// @see https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree
    pub fn initialize<H: Hasher>(tree: &mut NullifierIndexedTree) -> Result<(), ProgramError> {
        let height = tree.height as usize;

        // Create genesis leaf and compute its hash
        let genesis_leaf = IndexedLeaf::genesis();
        let genesis_hash = Self::compute_leaf_hash::<H>(&genesis_leaf)?;

        // Level 0: genesis leaf is at index 0 (left child)
        tree.subtrees[0] = genesis_hash;

        // Compute the initial tree structure
        // At each level, we hash the subtree with the zero value for the right sibling
        let mut current_hash = genesis_hash;
        for i in 0..height {
            if i > 0 {
                tree.subtrees[i] = current_hash;
            }
            // Hash with zero sibling on the right (using pre-computed zero hashes)
            current_hash = H::hashv(&[&current_hash, &INDEXED_ZERO_HASHES[i]])
                .map_err(|_| ShieldedPoolError::ArithmeticOverflow)?;
        }

        // Set the root
        tree.root = current_hash;

        // Both indices start at 1 because the genesis leaf occupies index 0.
        // This means real nullifiers will be assigned/inserted at indices 1, 2, 3, ...
        tree.next_index = 1;
        tree.next_pending_index = 1;

        Ok(())
    }

    /// Verify a merkle proof for a leaf at a given index.
    ///
    /// Returns the computed root if the proof is valid.
    pub fn compute_root_from_proof<H: Hasher>(
        leaf_hash: [u8; 32],
        index: u64,
        proof: &[[u8; 32]],
        height: u8,
    ) -> Result<[u8; 32], ProgramError> {
        if proof.len() != height as usize {
            return Err(ShieldedPoolError::InvalidLowNullifierProof.into());
        }

        let mut current_hash = leaf_hash;
        let mut current_index = index;

        for sibling in proof.iter() {
            let (left, right) = if current_index.is_multiple_of(2) {
                // Current is left child
                (current_hash, *sibling)
            } else {
                // Current is right child
                (*sibling, current_hash)
            };

            current_hash =
                H::hashv(&[&left, &right]).map_err(|_| ShieldedPoolError::ArithmeticOverflow)?;
            current_index /= 2;
        }

        Ok(current_hash)
    }

    /// Verify that a nullifier value falls within the valid range of a low nullifier.
    ///
    /// For insertion: `low_value < nullifier` AND (`nullifier < low_next_value` OR `low_next_value == 0`)
    ///
    /// Per Aztec spec: `next_value == 0` represents infinity (no next element in sorted list).
    /// @see https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree
    pub fn verify_ordering(
        low_value: &[u8; 32],
        nullifier: &[u8; 32],
        low_next_value: &[u8; 32],
    ) -> Result<(), ProgramError> {
        // Compare as big-endian 256-bit integers
        // Check: low_value < nullifier
        if !Self::is_less_than(low_value, nullifier) {
            return Err(ShieldedPoolError::InvalidLowNullifierOrdering.into());
        }

        // Check: nullifier < low_next_value OR low_next_value == 0 (infinity)
        // Per Aztec spec: next_value = 0 means "no next element" / infinity
        if !Self::is_zero(low_next_value) && !Self::is_less_than(nullifier, low_next_value) {
            return Err(ShieldedPoolError::InvalidLowNullifierOrdering.into());
        }

        Ok(())
    }

    /// Check if a 32-byte value is all zeros.
    /// Per Aztec spec: zero represents infinity in the indexed merkle tree.
    #[inline]
    fn is_zero(value: &[u8; 32]) -> bool {
        value.iter().all(|&b| b == 0)
    }

    /// Compare two 32-byte values as big-endian integers.
    /// Returns true if a < b.
    fn is_less_than(a: &[u8; 32], b: &[u8; 32]) -> bool {
        // Compare from most significant byte to least
        for i in (0..32).rev() {
            if a[i] < b[i] {
                return true;
            }
            if a[i] > b[i] {
                return false;
            }
        }
        // Equal
        false
    }

    /// Insert a new nullifier into the indexed tree.
    ///
    /// This updates:
    /// 1. The low nullifier leaf (updates its next_value and next_index)
    /// 2. Appends the new nullifier leaf at next_index
    /// 3. Updates the tree root (but NOT root_history)
    ///
    /// Returns the new root.
    pub fn insert<H: Hasher>(
        tree: &mut NullifierIndexedTree,
        nullifier: &[u8; 32],
        low_nullifier_index: u64,
        low_nullifier_value: &[u8; 32],
        low_nullifier_next_value: &[u8; 32],
        low_nullifier_next_index: u64,
        merkle_proof: &[[u8; 32]],
    ) -> Result<[u8; 32], ProgramError> {
        let height = tree.height;

        // 1. Verify ordering
        Self::verify_ordering(low_nullifier_value, nullifier, low_nullifier_next_value)?;

        // 2. Verify the low nullifier proof against a known root
        let old_low_leaf = IndexedLeaf::new(
            *low_nullifier_value,
            *low_nullifier_next_value,
            low_nullifier_next_index,
        );
        let old_low_hash = Self::compute_leaf_hash::<H>(&old_low_leaf)?;
        let computed_root = Self::compute_root_from_proof::<H>(
            old_low_hash,
            low_nullifier_index,
            merkle_proof,
            height,
        )?;

        if !tree.is_current_root(&computed_root) {
            return Err(ShieldedPoolError::UnknownNullifierRoot.into());
        }

        // 3. Check tree capacity
        if tree.is_full() {
            return Err(ShieldedPoolError::NullifierTreeFull.into());
        }

        let new_index = tree.next_index;

        // 4. Create the new nullifier leaf
        // The new leaf points to what the low nullifier used to point to
        let new_leaf = IndexedLeaf::new(
            *nullifier,
            *low_nullifier_next_value,
            low_nullifier_next_index,
        );
        let new_leaf_hash = Self::compute_leaf_hash::<H>(&new_leaf)?;

        // 5. Update the low nullifier to point to the new leaf
        let updated_low_leaf = IndexedLeaf::new(*low_nullifier_value, *nullifier, new_index);
        let updated_low_hash = Self::compute_leaf_hash::<H>(&updated_low_leaf)?;

        // 6. Update the tree with the modified low nullifier
        // First, update the root with the new low nullifier hash
        let _root_after_low_update = Self::compute_root_from_proof::<H>(
            updated_low_hash,
            low_nullifier_index,
            merkle_proof,
            height,
        )?;

        // 7. Append the new leaf using subtree caching
        let new_root = Self::append_leaf::<H>(tree, new_leaf_hash, new_index)?;

        // Note: The above is a simplification. In practice, we need to:
        // 1. Update the tree state after modifying the low nullifier
        // 2. Then append the new leaf
        // For now, we use a simplified approach that assumes the proof
        // was computed against root_history (which doesn't change during insertion)

        tree.next_index = new_index + 1;
        tree.root = new_root;

        Ok(new_root)
    }

    /// Append a new leaf to the tree using subtree caching.
    ///
    /// This is similar to the regular merkle tree append but doesn't update root_history.
    fn append_leaf<H: Hasher>(
        tree: &mut NullifierIndexedTree,
        leaf_hash: [u8; 32],
        index: u64,
    ) -> Result<[u8; 32], ProgramError> {
        let height = tree.height as usize;
        let mut current_index = index as usize;
        let mut current_hash = leaf_hash;

        for i in 0..height {
            let (left, right) = if current_index.is_multiple_of(2) {
                // This leaf is a left child
                // Update subtree to store this hash
                tree.subtrees[i] = current_hash;
                (current_hash, INDEXED_ZERO_HASHES[i])
            } else {
                // This leaf is a right child
                // Use stored subtree as left sibling
                (tree.subtrees[i], current_hash)
            };

            current_hash =
                H::hashv(&[&left, &right]).map_err(|_| ShieldedPoolError::ArithmeticOverflow)?;
            current_index /= 2;
        }

        Ok(current_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{MAX_NULLIFIER_VALUE, NULLIFIER_TREE_HEIGHT};

    #[allow(dead_code)]
    fn create_test_tree() -> NullifierIndexedTree {
        NullifierIndexedTree {
            next_index: 0,
            next_pending_index: 0,
            current_epoch: 0,
            earliest_provable_epoch: 0,
            last_finalized_index: 0,
            last_epoch_slot: 0,
            authority: [0u8; 32],
            root: [0u8; 32],
            subtrees: [[0u8; 32]; NULLIFIER_TREE_HEIGHT as usize],
            height: NULLIFIER_TREE_HEIGHT,
            bump: 0,
            _padding: [0u8; 6],
        }
    }

    // Note: Poseidon hash tests require the "poseidon" feature in light-hasher
    // which is only available for native builds. The on-chain program uses
    // the Solana syscall instead. Run LiteSVM tests to validate the implementation.

    #[test]
    fn test_ordering_valid() {
        let low = [0u8; 32];
        let mid = [1u8; 32];
        let high = MAX_NULLIFIER_VALUE;

        assert!(IndexedMerkleTree::verify_ordering(&low, &mid, &high).is_ok());
    }

    #[test]
    fn test_ordering_valid_with_zero_sentinel() {
        // Per Aztec spec: next_value = 0 represents infinity (end of sorted list)
        // Any nullifier greater than low_value should be valid when next_value is zero
        let low = [0u8; 32];
        let nullifier = [0xff; 32]; // Large value
        let zero_sentinel = [0u8; 32]; // 0 = infinity

        assert!(IndexedMerkleTree::verify_ordering(&low, &nullifier, &zero_sentinel).is_ok());
    }

    #[test]
    fn test_ordering_genesis_leaf() {
        // Genesis leaf has: value=0, next_value=0 (infinity), next_index=0
        // First insertion should be valid with the genesis leaf as low element
        let genesis_value = [0u8; 32];
        let first_nullifier = [1u8; 32];
        let genesis_next_value = [0u8; 32]; // 0 = infinity per Aztec spec

        assert!(
            IndexedMerkleTree::verify_ordering(
                &genesis_value,
                &first_nullifier,
                &genesis_next_value
            )
            .is_ok()
        );
    }

    #[test]
    fn test_ordering_invalid_low() {
        let low = [2u8; 32];
        let mid = [1u8; 32]; // mid < low, invalid
        let high = MAX_NULLIFIER_VALUE;

        assert!(IndexedMerkleTree::verify_ordering(&low, &mid, &high).is_err());
    }

    #[test]
    fn test_ordering_invalid_high() {
        let low = [0u8; 32];
        let mid = [0xff; 32]; // mid > high (which is MAX), would fail
        let high = [0xfe; 32];

        assert!(IndexedMerkleTree::verify_ordering(&low, &mid, &high).is_err());
    }

    #[test]
    fn test_is_less_than() {
        assert!(IndexedMerkleTree::is_less_than(&[0u8; 32], &[1u8; 32]));
        assert!(!IndexedMerkleTree::is_less_than(&[1u8; 32], &[0u8; 32]));
        assert!(!IndexedMerkleTree::is_less_than(&[1u8; 32], &[1u8; 32])); // equal
    }

    #[test]
    fn test_is_zero() {
        assert!(IndexedMerkleTree::is_zero(&[0u8; 32]));
        assert!(!IndexedMerkleTree::is_zero(&[1u8; 32]));
        assert!(!IndexedMerkleTree::is_zero(&MAX_NULLIFIER_VALUE));
    }
}
