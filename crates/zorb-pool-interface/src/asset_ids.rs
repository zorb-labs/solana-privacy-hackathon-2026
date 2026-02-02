//! Reserved asset ID constants for the Zorb protocol.
//!
//! # Asset ID Namespace
//!
//! The 256-bit asset ID space is partitioned:
//!
//! - **Reserved range** (`asset_id < 2^64`): Protocol-defined unified pools
//!   - First 24 bytes are zero, last 8 bytes encode the pool identifier
//!   - Assigned sequentially: SOL=1, USD=2 (future), ETH=3 (future), etc.
//!
//! - **Derived range** (`asset_id >= 2^64`): Token pools
//!   - Computed as `Poseidon(mint_lo_128, mint_hi_128)`
//!   - Cryptographically unique per SPL token mint
//!
//! # Security
//!
//! The probability of a Poseidon hash colliding with the reserved range is
//! approximately 2^64 / 2^254 ≈ 0, making collisions computationally infeasible.

/// Unified SOL pool asset ID.
///
/// All LSTs (WSOL, vSOL, jitoSOL, mSOL, etc.) share this asset ID,
/// enabling fungibility within the shielded pool.
///
/// Value: `[0x00...0x01]` (1 as big-endian 256-bit integer)
pub const UNIFIED_SOL: [u8; 32] = {
    let mut id = [0u8; 32];
    id[31] = 1;
    id
};

/// Reserved for future: Unified USD pool asset ID.
///
/// All stablecoins (USDC, USDT, DAI, etc.) would share this asset ID.
///
/// Value: `[0x00...0x02]` (2 as big-endian 256-bit integer)
pub const UNIFIED_USD: [u8; 32] = {
    let mut id = [0u8; 32];
    id[31] = 2;
    id
};

/// Reserved for future: Unified ETH pool asset ID.
///
/// All ETH LSTs would share this asset ID.
///
/// Value: `[0x00...0x03]` (3 as big-endian 256-bit integer)
pub const UNIFIED_ETH: [u8; 32] = {
    let mut id = [0u8; 32];
    id[31] = 3;
    id
};

/// Check if an asset ID is in the reserved range.
///
/// Reserved asset IDs have the first 24 bytes as zero.
/// This range is used for unified pools (UnifiedSol, UnifiedUsd, etc.).
///
/// Token pools use Poseidon-derived asset IDs which are ~uniformly distributed
/// over the BN254 scalar field and will (with overwhelming probability) NOT
/// fall into the reserved range.
///
/// # Example
///
/// ```
/// use zorb_pool_interface::asset_ids::{is_reserved, UNIFIED_SOL};
///
/// assert!(is_reserved(&UNIFIED_SOL));
/// assert!(!is_reserved(&[0xFF; 32])); // High-value ID (token pool range)
/// ```
#[inline]
pub const fn is_reserved(asset_id: &[u8; 32]) -> bool {
    // Check if first 24 bytes are all zero
    let mut i = 0;
    while i < 24 {
        if asset_id[i] != 0 {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_sol_is_reserved() {
        assert!(is_reserved(&UNIFIED_SOL));
    }

    #[test]
    fn test_unified_usd_is_reserved() {
        assert!(is_reserved(&UNIFIED_USD));
    }

    #[test]
    fn test_unified_eth_is_reserved() {
        assert!(is_reserved(&UNIFIED_ETH));
    }

    #[test]
    fn test_high_value_not_reserved() {
        // A typical Poseidon output would have non-zero high bytes
        let token_asset_id = [0xFF; 32];
        assert!(!is_reserved(&token_asset_id));
    }

    #[test]
    fn test_boundary_case() {
        // First byte non-zero -> not reserved
        let mut boundary = [0u8; 32];
        boundary[0] = 1;
        assert!(!is_reserved(&boundary));

        // 24th byte non-zero -> not reserved (boundary of check)
        let mut boundary2 = [0u8; 32];
        boundary2[23] = 1;
        assert!(!is_reserved(&boundary2));

        // 25th byte non-zero but first 24 zero -> still reserved
        let mut boundary3 = [0u8; 32];
        boundary3[24] = 0xFF;
        assert!(is_reserved(&boundary3));
    }

    #[test]
    fn test_unified_sol_value() {
        // UNIFIED_SOL should be [0, 0, ..., 0, 1]
        let expected = {
            let mut id = [0u8; 32];
            id[31] = 1;
            id
        };
        assert_eq!(UNIFIED_SOL, expected);
    }

    #[test]
    fn test_unified_usd_value() {
        // UNIFIED_USD should be [0, 0, ..., 0, 2]
        let expected = {
            let mut id = [0u8; 32];
            id[31] = 2;
            id
        };
        assert_eq!(UNIFIED_USD, expected);
    }
}
