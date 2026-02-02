//! Program deployment helpers for unified-sol-pool tests.

use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;

/// Unified SOL Pool program ID (from centralized zorb-program-ids crate)
pub const UNIFIED_SOL_POOL_PROGRAM_ID: Pubkey = Pubkey::new_from_array(
    five8_const::decode_32_const(zorb_program_ids::UNIFIED_SOL_POOL_ID),
);

/// Deploy the unified SOL pool program
pub fn deploy_unified_sol_pool_program(svm: &mut LiteSVM) -> Pubkey {
    let program_id = UNIFIED_SOL_POOL_PROGRAM_ID;
    let program_data = include_bytes!("../../../../target/deploy/unified_sol_pool.so");
    let _ = svm.add_program(program_id, program_data);
    program_id
}

/// Warp the slot forward
pub fn warp_to_slot(svm: &mut LiteSVM, slot: u64) {
    svm.warp_to_slot(slot);
}
