use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;
use zorb_pool_interface::authority::HasAuthority;

use crate::state::ShieldedPoolAccount;

/// Global configuration singleton for the shielded pool.
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][72-byte struct data]`
///
/// Total on-chain size: 80 bytes
#[account(ShieldedPoolAccount::GlobalConfig)]
#[repr(C)]
pub struct GlobalConfig {
    /// Authority that controls the pool
    pub authority: Pubkey,
    /// Pending authority for two-step transfer.
    /// Set by `transfer_authority`, must call `accept_authority` to complete.
    pub pending_authority: Pubkey,
    /// Whether the pool is paused (0 = active, 1 = paused)
    pub is_paused: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 6],
}

impl GlobalConfig {
    /// Returns true if the pool is paused
    #[inline]
    pub fn paused(&self) -> bool {
        self.is_paused != 0
    }
}

impl HasAuthority for GlobalConfig {
    fn authority(&self) -> &Pubkey {
        &self.authority
    }
    fn authority_mut(&mut self) -> &mut Pubkey {
        &mut self.authority
    }
    fn pending_authority(&self) -> &Pubkey {
        &self.pending_authority
    }
    fn pending_authority_mut(&mut self) -> &mut Pubkey {
        &mut self.pending_authority
    }
}
