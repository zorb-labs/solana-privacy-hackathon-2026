//! Fund rewards instruction handler.
//!
//! Allows external callers to fund the reward pool with tokens.
//! Tokens are transferred to vault and tracked in pending_rewards.

use crate::{TokenPoolConfig, TokenPoolError};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo};
use pinocchio_token::{instructions::Transfer, state::TokenAccount};

/// Instruction data for FundRewards.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct FundRewardsData {
    /// Amount of tokens to fund as rewards
    pub amount: u64,
}

/// Accounts for the FundRewards instruction.
#[derive(Accounts)]
pub struct FundRewardsAccounts<'info> {
    /// Pool configuration account (writable for state updates)
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Vault token account (receives reward tokens)
    #[account(mut, pda = Vault, pda::pool_config = pool_config.key())]
    pub vault: &'info AccountInfo,

    /// Funder's token account (source of reward tokens)
    #[account(mut)]
    pub mine_sol: LazyAccount<'info, TokenAccount>,

    /// Funder authority (signer for transfer)
    pub funder: Signer<'info>,

    /// SPL Token program (required for Transfer CPI)
    #[account(address = pinocchio_token::ID)]
    pub token_program: &'info AccountInfo,
}

/// Fund the reward pool with external tokens.
///
/// Permissionless - anyone with tokens can fund rewards.
/// Tokens are transferred to vault and added to pending_rewards.
pub fn process_fund_rewards(
    ctx: Context<FundRewardsAccounts>,
    data: FundRewardsData,
) -> ProgramResult {
    let FundRewardsAccounts {
        pool_config,
        vault: vault_acc,
        mine_sol,
        funder: funder_acc,
        token_program: _,
    } = ctx.accounts;

    // Validate config is active and amount > 0
    pool_config.try_inspect(|config| {
        config.require_active()?;
        if data.amount == 0 {
            return Err(TokenPoolError::InvalidAmount.into());
        }
        Ok(())
    })?;

    // Transfer tokens: mine_sol -> vault (borrow released)
    Transfer {
        from: mine_sol.info(),
        to: vault_acc,
        authority: funder_acc,
        amount: data.amount,
    }
    .invoke()?;

    // Update state
    pool_config.try_inspect_mut(|config| {
        // Update pending_funded_rewards (tracks external funding separately from fees)
        config.pending_funded_rewards = config
            .pending_funded_rewards
            .checked_add(data.amount)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Update total_funded_rewards (cumulative tracking)
        config.total_funded_rewards = config
            .total_funded_rewards
            .checked_add(data.amount as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        Ok(())
    })
}
