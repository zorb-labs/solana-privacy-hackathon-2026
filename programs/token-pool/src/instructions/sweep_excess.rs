//! Sweep excess tokens instruction handler.
//!
//! Permissionless instruction that detects tokens in the vault that arrived
//! outside of normal deposit/fund_rewards flows and adds them to pending rewards.
//!
//! # Vault Balance Invariant
//!
//! Under normal operation (no direct SPL transfers to vault):
//!
//! ```text
//! vault.amount = total_deposited - total_withdrawn
//!              + total_deposit_fees + total_withdrawal_fees
//!              + total_funded_rewards
//! ```
//!
//! ## Proof Sketch
//!
//! **Completeness:** Only 3 of 11 instructions affect vault balance:
//! - `Deposit`: transfers tokens IN, updates `total_deposited` + `total_deposit_fees`
//! - `Withdraw`: approves tokens OUT, updates `total_withdrawn` + `total_withdrawal_fees`
//! - `FundRewards`: transfers tokens IN, updates `total_funded_rewards`
//!
//! The other 8 instructions (InitPool, SetPoolActive, SetFeeRates, FinalizeRewards,
//! Log, SweepExcess, TransferAuthority, AcceptAuthority) do not transfer tokens
//! to/from the vault or modify the tracked balance fields.
//!
//! **Correctness:** For each vault-modifying operation, Δvault = Δexpected:
//! - Deposit: `Δvault = +gross`, `Δexpected = +(net + fee) = +gross` ✓
//! - Withdraw: `Δvault = -output`, `Δexpected = -gross + fee = -(gross - fee) = -output` ✓
//! - FundRewards: `Δvault = +amount`, `Δexpected = +amount` ✓
//!
//! **Corollary:** Any `excess = vault.amount - expected > 0` represents tokens
//! that arrived outside program control (direct SPL transfers). SweepExcess
//! captures these by adding to `total_funded_rewards`, restoring the invariant.
//!
//! See [`docs/vault-invariant-proof.md`](../../../docs/vault-invariant-proof.md) for the complete formal proof.

use crate::{
    TokenPoolConfig, TokenPoolError, emit_event, events::SweepExcessEvent,
    gen_token_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{ProgramResult, instruction::Signer as PinocchioSigner, sysvars::Sysvar};
use pinocchio_token::state::TokenAccount;

/// Accounts for the SweepExcess instruction.
#[derive(Accounts)]
pub struct SweepExcessAccounts<'info> {
    /// Pool configuration account (writable for state updates)
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Vault token account (read-only to check balance)
    #[account(pda = Vault, pda::pool_config = pool_config.key())]
    pub vault: LazyAccount<'info, TokenAccount>,

    /// Token pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub token_pool_program: &'info AccountInfo,
}

/// Sweep excess tokens from the vault into pending rewards.
///
/// Permissionless - anyone can call this to recover tokens that arrived
/// in the vault outside of normal deposit/fund_rewards flows.
///
/// Excess = vault_balance - expected_balance
/// where expected_balance is derived from cumulative accounting stats.
pub fn process_sweep_excess(ctx: Context<SweepExcessAccounts>) -> ProgramResult {
    let SweepExcessAccounts { pool_config, vault, token_pool_program } = ctx.accounts;

    // Get actual vault balance by loading the token account
    let vault_data = vault.load()?;
    let vault_balance = vault_data.amount();

    // Calculate expected vault balance and update state
    let (excess, mint, bump) = pool_config.try_map(|config| {
        // Expected vault balance formula:
        // = tokens_deposited - tokens_withdrawn_out + funded_rewards
        // = (total_deposited + total_deposit_fees)
        //   - (total_withdrawn - total_withdrawal_fees)
        //   + total_funded_rewards
        //
        // Simplified:
        // = total_deposited - total_withdrawn
        //   + total_deposit_fees + total_withdrawal_fees
        //   + total_funded_rewards
        let expected = config
            .total_deposited
            .checked_sub(config.total_withdrawn)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_add(config.total_deposit_fees)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_add(config.total_withdrawal_fees)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_add(config.total_funded_rewards)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Calculate excess (saturating to 0 if vault has less than expected)
        let vault_balance_u128 = vault_balance as u128;
        let excess = vault_balance_u128.saturating_sub(expected);

        Ok((excess, config.mint, config.bump))
    })?;

    // Nothing to sweep
    if excess == 0 {
        return Ok(());
    }

    // Excess should fit in u64 (it's bounded by vault balance which is u64)
    let excess_u64 = u64::try_from(excess).map_err(|_| TokenPoolError::ArithmeticOverflow)?;

    // Update pending and total funded rewards
    pool_config.try_inspect_mut(|config| {
        config.pending_funded_rewards = config
            .pending_funded_rewards
            .checked_add(excess_u64)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        config.total_funded_rewards = config
            .total_funded_rewards
            .checked_add(excess)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        Ok(())
    })?;

    // Emit event
    let bump_bytes = [bump];
    let seeds = gen_token_pool_config_seeds(&mint, &bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        pool_config.account_info(),
        token_pool_program,
        signer,
        &SweepExcessEvent {
            mint,
            amount: excess_u64,
            slot: pinocchio::sysvars::clock::Clock::get()?.slot,
        },
    )?;

    Ok(())
}
