//! Initialize unified SOL pool configuration.

use crate::{
    UNIFIED_SOL_ASSET_ID, UnifiedSolPoolConfig, UnifiedSolPoolError,
    find_unified_sol_pool_config_pda,
};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;
use zorb_pool_interface::BASIS_POINTS;

/// Instruction data for InitUnifiedSolPoolConfig.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct InitUnifiedSolPoolConfigData {
    /// Maximum tokens allowed per deposit transaction (0 = no limit)
    pub max_deposit_amount: u64,
    /// Deposit fee in basis points (e.g., 100 = 1%)
    pub deposit_fee_rate: u16,
    /// Withdrawal fee in basis points (e.g., 100 = 1%)
    pub withdrawal_fee_rate: u16,
    /// Minimum WSOL buffer as basis points (e.g., 2000 = 20%)
    pub min_buffer_bps: u16,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 2],
    /// Minimum absolute WSOL buffer in lamports
    pub min_buffer_amount: u64,
}

/// Accounts for the InitUnifiedSolPoolConfig instruction.
#[derive(Accounts)]
pub struct InitUnifiedSolPoolConfigAccounts<'info> {
    /// UnifiedSolPoolConfig PDA to create ["unified_sol_pool"]
    #[account(init, payer = authority, pda = UnifiedSolPoolConfig)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// Authority for this pool (becomes pool authority, pays for account creation)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Initialize unified SOL pool configuration.
///
/// Creates a UnifiedSolPoolConfig PDA that will manage all SOL-equivalent assets.
/// Account creation is handled by panchor via the `init` constraint.
pub fn process_init_unified_sol_pool_config(
    ctx: Context<InitUnifiedSolPoolConfigAccounts>,
    data: InitUnifiedSolPoolConfigData,
) -> ProgramResult {
    let InitUnifiedSolPoolConfigAccounts {
        unified_sol_pool_config,
        authority,
        system_program: _,
    } = ctx.accounts;

    // Validate fee rates (max 100%)
    if data.deposit_fee_rate > BASIS_POINTS as u16 || data.withdrawal_fee_rate > BASIS_POINTS as u16
    {
        log!("init_unified_sol_pool_config: fee rate too high");
        return Err(UnifiedSolPoolError::InvalidFeeRate.into());
    }

    // Get PDA bump for storing in config
    let (_, unified_bump) = find_unified_sol_pool_config_pda();

    // Initialize unified sol pool config data
    // Note: Account is already created by panchor's init constraint
    unified_sol_pool_config.inspect_mut(|config| {
        config.asset_id = UNIFIED_SOL_ASSET_ID;
        config.authority = *authority.key();
        config.pending_authority = [0u8; 32];
        // AUDIT: EPOCH MODEL - reward_epoch starts at 1, not 0
        // =====================================================================
        // Epoch 0 is reserved as the "uninitialized" epoch. Starting at 1 ensures
        // that newly added LSTs (which have last_harvest_epoch = 0) cannot pass
        // the finalize_unified_rewards check (last_harvest_epoch == reward_epoch)
        // until they've actually been harvested.
        //
        // Invariant: At any reward_epoch N >= 1, an LST can only be finalized if
        // last_harvest_epoch == N, which requires harvest_lst_appreciation to have
        // been called during epoch N.
        //
        // See also:
        // - init_lst_config.rs: last_harvest_epoch initialized to 0
        // - finalize_unified_rewards.rs: last_harvest_epoch == reward_epoch check
        // - harvest_lst_appreciation.rs: sets last_harvest_epoch = current_epoch
        // - pool_config.rs (shielded-pool): last_harvest_epoch == accumulator_epoch - 1
        // =====================================================================
        config.reward_epoch = 1;
        config._reserved1 = [0u64; 7];
        config.total_virtual_sol = 0;
        config.reward_accumulator = 0;
        config.last_finalized_slot = 0;
        config.pending_deposit_fees = 0;
        config.pending_withdrawal_fees = 0;
        config.pending_appreciation = 0;
        config.finalized_balance = 0;
        config.pending_deposits = 0;
        config.pending_withdrawals = 0;
        config.deposit_fee_rate = data.deposit_fee_rate;
        config.withdrawal_fee_rate = data.withdrawal_fee_rate;
        config.min_buffer_bps = data.min_buffer_bps;
        config._pad1 = [0u8; 2];
        config.min_buffer_amount = data.min_buffer_amount;
        config.is_active = 1;
        config.bump = unified_bump;
        config._pad2 = [0u8; 14];
        config.total_deposited = 0;
        config.total_withdrawn = 0;
        config.total_rewards_distributed = 0;
        config.total_deposit_fees = 0;
        config.total_withdrawal_fees = 0;
        config._reserved_stats = 0;
        config.total_appreciation = 0;
        config.max_deposit_amount = data.max_deposit_amount;
        config.deposit_count = 0;
        config.withdrawal_count = 0;
        config.lst_count = 0;
        config._reserved = [0u8; 23];
    })?;

    log!("init_unified_sol_pool_config: initialized successfully");

    Ok(())
}
