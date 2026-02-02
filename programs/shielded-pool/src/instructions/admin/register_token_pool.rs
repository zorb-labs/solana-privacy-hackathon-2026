//! Register a token pool with the hub.
//!
//! Creates a PoolConfigAccount linking to an existing TokenPoolConfig,
//! enabling the hub to route deposits/withdrawals for this token.

use crate::{
    errors::ShieldedPoolError,
    events::{PoolRegisteredEvent, emit_event},
    pda::{POOL_CONFIG_SEED, find_pool_config_pda, gen_global_config_seeds},
    state::{GlobalConfig, HubPoolType, PoolConfig, TokenPoolConfig},
};
use panchor::{SetDiscriminator, prelude::*};
use pinocchio::{
    ProgramResult,
    instruction::{Seed, Signer as PinocchioSigner},
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use zorb_pool_interface::{TOKEN_POOL_PROGRAM_ID, asset_ids};

/// Accounts for the RegisterTokenPool instruction.
#[derive(Accounts)]
pub struct RegisterTokenPoolAccounts<'info> {
    /// Global config PDA ["global_config"]
    #[account(owner = crate::ID)]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Pool config PDA to create ["pool_config", asset_id]
    /// Raw AccountInfo since we're creating this account via CPI
    #[account(mut)]
    pub pool_config: &'info AccountInfo,

    /// TokenPoolConfig account from the token-pool program.
    /// Must be owned by TOKEN_POOL_PROGRAM_ID.
    #[account(owner = TOKEN_POOL_PROGRAM_ID)]
    pub token_pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Must match global_config.authority (signer and payer)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Register a token pool with the hub.
///
/// Creates a PoolConfigAccount linking to an existing TokenPoolConfig.
/// The asset_id is read from the TokenPoolConfig.
///
/// # Authority
///
/// Must be GlobalConfig.authority.
///
/// # Prerequisites
///
/// The TokenPoolConfig must already exist (created via token-pool's InitPool).
pub fn process_register_token_pool(ctx: Context<RegisterTokenPoolAccounts>) -> ProgramResult {
    let RegisterTokenPoolAccounts {
        global_config,
        pool_config,
        token_pool_config,
        authority,
        system_program,
        shielded_pool_program,
    } = ctx.accounts;

    // Validate system program
    if *system_program.key() != pinocchio_contrib::constants::SYSTEM_PROGRAM_ID {
        log!("register_token_pool: invalid system program");
        return Err(ShieldedPoolError::InvalidSystemProgram.into());
    }

    // Validate authority against GlobalConfig and get bump for event emission
    let global_config_bump = global_config.try_map(|config| {
        if config.authority != *authority.key() {
            log!("register_token_pool: unauthorized");
            return Err(ShieldedPoolError::Unauthorized.into());
        }
        Ok(config.bump)
    })?;

    // Read asset_id from TokenPoolConfig using type-safe AccountLoader
    let asset_id = token_pool_config.map(|config| config.asset_id)?;

    // Validate asset_id is NOT in reserved range (reserved for unified pools)
    // Token pools use Poseidon-derived asset IDs which should never fall in reserved range
    if asset_ids::is_reserved(&asset_id) {
        log!("register_token_pool: asset_id is in reserved range");
        return Err(ShieldedPoolError::InvalidAssetId.into());
    }

    // Verify pool_config PDA matches expected derivation
    // Note: Runtime validation required since asset_id comes from account data
    let (expected_pda, bump) = find_pool_config_pda(&asset_id);
    if pool_config.key() != &expected_pda {
        log!("register_token_pool: invalid pool config PDA");
        return Err(ShieldedPoolError::InvalidPoolConfigPda.into());
    }

    // Get rent sysvar
    let rent = Rent::get()?;

    // Create pool_config account PDA
    let bump_bytes = [bump];
    let seeds = [
        Seed::from(POOL_CONFIG_SEED),
        Seed::from(&asset_id),
        Seed::from(&bump_bytes),
    ];
    let signer = PinocchioSigner::from(&seeds);

    CreateAccount {
        from: authority,
        to: pool_config,
        lamports: rent.minimum_balance(PoolConfig::INIT_SPACE),
        space: PoolConfig::INIT_SPACE as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[signer])?;

    // Set discriminator on the newly created account
    {
        let mut data = pool_config.try_borrow_mut_data()?;
        PoolConfig::set_discriminator(&mut data);
    }

    // Initialize pool_config fields (order matches struct layout)
    AccountLoader::<PoolConfig>::new(pool_config)?
        .inspect_mut(|config| {
            // Routing (64 bytes)
            config.pool_program = TOKEN_POOL_PROGRAM_ID;
            config.asset_id = asset_id;
            // Metadata (8 bytes)
            config.pool_type = HubPoolType::Token as u8;
            config.is_active = 1;
            config.bump = bump;
            config._padding = [0u8; 5];
        })?;

    // Emit pool registered event
    let global_bump_bytes = [global_config_bump];
    let global_signer_seeds = gen_global_config_seeds(&global_bump_bytes);
    let global_signer = PinocchioSigner::from(&global_signer_seeds);
    let event = PoolRegisteredEvent {
        pool_type: HubPoolType::Token as u8,
        _padding: [0u8; 7],
        asset_id,
        pool_program: TOKEN_POOL_PROGRAM_ID,
    };
    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        global_signer,
        &event,
    )?;

    log!("register_token_pool: success");

    Ok(())
}
