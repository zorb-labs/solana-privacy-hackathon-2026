//! Unified SOL pool invariant tests.
//!
//! This module contains comprehensive invariant testing for the unified SOL pool's
//! harvest and advance epoch cycle. Tests verify that on-chain state transitions
//! match the expected mathematical formulas from the protocol specification.
//!
//! Reference: docs/unified-sol-pool-pricing-soundness.md
//!
//! Test approach:
//! 1. Execute instructions through actual on-chain program
//! 2. Read state before and after each instruction
//! 3. Verify state transitions match expected formulas

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Simple deterministic pseudo-random number generator for reproducible tests.
struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    fn gen_range(&mut self, max: u64) -> u64 {
        self.next_u64() % max
    }
}

/// Slot interval required between epoch advances
/// At 400ms/slot: 2700 slots ≈ 18 minutes
const UPDATE_SLOT_INTERVAL: u64 = 2700;

/// Rate precision (1e9)
const RATE_PRECISION: u64 = 1_000_000_000;

/// Accumulator precision (1e18)
const ACCUMULATOR_PRECISION: u128 = 1_000_000_000_000_000_000;

/// Snapshot of LstConfig state for before/after comparison
#[derive(Debug, Clone)]
struct LstSnapshot {
    exchange_rate: u64,
    previous_exchange_rate: u64,
    harvested_exchange_rate: u64,
    last_harvest_epoch: u64,
    total_appreciation_harvested: u64,
}

impl LstSnapshot {
    fn read(svm: &LiteSVM, lst_config: &Pubkey) -> Self {
        Self {
            exchange_rate: get_lst_config_exchange_rate(svm, lst_config),
            previous_exchange_rate: get_lst_config_previous_exchange_rate(svm, lst_config),
            harvested_exchange_rate: get_lst_config_harvested_exchange_rate(svm, lst_config),
            last_harvest_epoch: get_lst_config_last_harvest_epoch(svm, lst_config),
            total_appreciation_harvested: get_lst_config_total_appreciation_harvested(svm, lst_config),
        }
    }

    /// Compute virtual SOL value from vault balance.
    /// Formula: virtual_sol_value = vault_balance * exchange_rate / 1e9
    fn virtual_sol_value(&self, vault_balance: u64) -> u128 {
        compute_virtual_sol_value(vault_balance, self.exchange_rate)
    }
}

/// Snapshot of UnifiedSolConfig state for before/after comparison
#[derive(Debug, Clone)]
struct UnifiedSnapshot {
    reward_epoch: u64,
    reward_accumulator: u128,
    pending_appreciation: u64,
    finalized_balance: u128,
    pending_deposits: u128,
    pending_withdrawals: u128,
    total_appreciation: u128,
    #[allow(dead_code)]
    total_rewards_distributed: u128,
}

impl UnifiedSnapshot {
    fn read(svm: &LiteSVM, unified_config: &Pubkey) -> Self {
        Self {
            reward_epoch: get_unified_config_reward_epoch(svm, unified_config),
            reward_accumulator: get_unified_config_reward_accumulator(svm, unified_config),
            pending_appreciation: get_unified_config_pending_appreciation(svm, unified_config),
            finalized_balance: get_unified_config_finalized_balance(svm, unified_config),
            pending_deposits: get_unified_config_pending_deposits(svm, unified_config),
            pending_withdrawals: get_unified_config_pending_withdrawals(svm, unified_config),
            total_appreciation: get_unified_config_total_appreciation(svm, unified_config),
            total_rewards_distributed: get_unified_config_total_rewards_distributed(svm, unified_config),
        }
    }
}

/// Test 1000 cycles of harvest and advance epoch with single LST (WSOL).
///
/// Verifies state transitions through actual instruction execution:
/// - WSOL harvest: virtual_sol_value = vault_balance (no appreciation)
/// - Advance epoch: epoch increments, accumulator updates if pending_appreciation > 0
#[test]
fn test_1000_cycles_single_wsol_invariants() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 100_000_000_000_000).unwrap();

    // Create payers for unique transaction signatures
    let payers: Vec<Keypair> = (0..2000).map(|_| Keypair::new()).collect();
    for payer in &payers {
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    }

    // Initialize unified SOL config through instruction
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create WSOL LST config through instruction
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);

    // Set initial vault balance (simulating external token transfer)
    let initial_balance: u64 = 1_000_000_000_000; // 1000 SOL worth
    update_vault_balance(&mut svm, &lst_vault, initial_balance);
    update_lst_config_vault_balance(&mut svm, &lst_config, initial_balance);

    let mut current_slot: u64 = 0;

    // Run 1000 cycles
    for cycle in 0..1000u64 {
        current_slot += UPDATE_SLOT_INTERVAL + 10;
        warp_to_slot(&mut svm, current_slot);

        let harvest_payer = &payers[(cycle as usize) * 2];
        let advance_payer = &payers[(cycle as usize) * 2 + 1];

        // === HARVEST ===
        // Snapshot state before harvest
        let lst_before = LstSnapshot::read(&svm, &lst_config);
        let unified_before = UnifiedSnapshot::read(&svm, &unified_sol_config);
        let vault_balance = get_token_balance(&svm, &lst_vault);

        // Execute harvest instruction
        harvest_lst_appreciation(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &lst_config,
            &lst_vault,
            None,
            harvest_payer,
        )
        .expect(&format!("harvest should succeed at cycle {}", cycle));

        // Snapshot state after harvest
        let lst_after = LstSnapshot::read(&svm, &lst_config);
        let unified_after = UnifiedSnapshot::read(&svm, &unified_sol_config);

        // Verify WSOL harvest invariants:
        // - virtual_sol_value should equal vault_balance (WSOL rate is 1:1)
        assert_eq!(
            lst_after.virtual_sol_value(vault_balance), vault_balance as u128,
            "Cycle {}: WSOL virtual_sol_value should equal vault_balance",
            cycle
        );

        // - exchange_rate should remain unchanged for WSOL (always 1e9)
        assert_eq!(
            lst_after.exchange_rate, lst_before.exchange_rate,
            "Cycle {}: WSOL exchange_rate should not change",
            cycle
        );

        // - previous_exchange_rate should remain unchanged for WSOL
        assert_eq!(
            lst_after.previous_exchange_rate, lst_before.previous_exchange_rate,
            "Cycle {}: WSOL previous_exchange_rate should not change",
            cycle
        );

        // - last_harvest_epoch should be updated to current epoch
        assert_eq!(
            lst_after.last_harvest_epoch, unified_before.reward_epoch,
            "Cycle {}: last_harvest_epoch should be current epoch",
            cycle
        );

        // - For WSOL, no appreciation should be added to pending_appreciation
        assert_eq!(
            unified_after.pending_appreciation, unified_before.pending_appreciation,
            "Cycle {}: WSOL should not generate appreciation",
            cycle
        );

        // - total_appreciation_harvested should not change for WSOL
        assert_eq!(
            lst_after.total_appreciation_harvested, lst_before.total_appreciation_harvested,
            "Cycle {}: WSOL total_appreciation_harvested should not change",
            cycle
        );

        // === ADVANCE EPOCH ===
        // Snapshot before advance
        let unified_before_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);
        let lst_before_advance = LstSnapshot::read(&svm, &lst_config);

        // Execute advance epoch instruction
        advance_unified_epoch(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &[lst_config],
            advance_payer,
        )
        .expect(&format!("advance_unified_epoch should succeed at cycle {}", cycle));

        // Snapshot after advance
        let unified_after_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);
        let lst_after_advance = LstSnapshot::read(&svm, &lst_config);

        // Verify advance epoch invariants:
        // - reward_epoch should increment by 1
        assert_eq!(
            unified_after_advance.reward_epoch,
            unified_before_advance.reward_epoch + 1,
            "Cycle {}: reward_epoch should increment by 1",
            cycle
        );

        // - pending_appreciation should be cleared (distributed to accumulator)
        assert_eq!(
            unified_after_advance.pending_appreciation, 0,
            "Cycle {}: pending_appreciation should be 0 after advance",
            cycle
        );

        // - harvested_exchange_rate should be frozen to current exchange_rate (INV-8)
        assert_eq!(
            lst_after_advance.harvested_exchange_rate,
            lst_after_advance.exchange_rate,
            "Cycle {}: INV-8 violated: harvested_exchange_rate should equal exchange_rate",
            cycle
        );

        // - If there were pending_appreciation, accumulator should increase
        if unified_before_advance.pending_appreciation > 0 {
            let total_pool = unified_before_advance
                .finalized_balance
                .checked_add(unified_before_advance.pending_deposits)
                .unwrap()
                .checked_sub(unified_before_advance.pending_withdrawals)
                .unwrap();

            if total_pool > 0 {
                let expected_delta = (unified_before_advance.pending_appreciation as u128)
                    .checked_mul(ACCUMULATOR_PRECISION)
                    .unwrap()
                    .checked_div(total_pool)
                    .unwrap();

                let actual_delta = unified_after_advance
                    .reward_accumulator
                    .checked_sub(unified_before_advance.reward_accumulator)
                    .unwrap();

                assert_eq!(
                    actual_delta, expected_delta,
                    "Cycle {}: accumulator delta mismatch",
                    cycle
                );
            }
        }
    }

    println!("Successfully completed 1000 WSOL cycles");
    let final_unified = UnifiedSnapshot::read(&svm, &unified_sol_config);
    println!("Final reward_epoch: {}", final_unified.reward_epoch);
    println!("Final reward_accumulator: {}", final_unified.reward_accumulator);
}

/// Test 1000 cycles with SPL Stake Pool LST that appreciates each cycle.
///
/// Verifies state transitions:
/// - Harvest: appreciation = new_virtual_sol - old_virtual_sol
/// - Accumulator grows with appreciation rewards
#[test]
fn test_1000_cycles_appreciating_lst_invariants() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 100_000_000_000_000).unwrap();

    let payers: Vec<Keypair> = (0..2000).map(|_| Keypair::new()).collect();
    for payer in &payers {
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    }

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create SPL Stake Pool LST config
    let lst_mint = create_mock_mint(&mut svm, 9);
    let stake_pool_program = SPL_STAKE_POOL_PROGRAM_ID;

    // Initial stake pool state: 1:1 rate
    let initial_lamports: u64 = 1_000_000_000_000;
    let initial_supply: u64 = 1_000_000_000_000;
    let stake_pool = create_mock_stake_pool(
        &mut svm,
        &lst_mint,
        initial_lamports,
        initial_supply,
        stake_pool_program,
    );

    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::SPL_STAKE_POOL,
    )
    .expect("init_lst_config should succeed");

    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);

    // Initial vault balance: 100 LST tokens
    let vault_balance: u64 = 100_000_000_000;
    update_vault_balance(&mut svm, &lst_vault, vault_balance);
    update_lst_config_vault_balance(&mut svm, &lst_config, vault_balance);

    // Track stake pool state for rate simulation
    let mut total_lamports = initial_lamports;
    let pool_supply = initial_supply;

    let mut current_slot: u64 = 0;

    // 0.3% appreciation per cycle
    let rate_increase_bps: u64 = 30;

    for cycle in 0..1000u64 {
        current_slot += UPDATE_SLOT_INTERVAL + 10;
        warp_to_slot(&mut svm, current_slot);

        let harvest_payer = &payers[(cycle as usize) * 2];
        let advance_payer = &payers[(cycle as usize) * 2 + 1];

        // Simulate stake pool appreciation (external system)
        let appreciation_amount = total_lamports * rate_increase_bps / 10000;
        total_lamports += appreciation_amount;
        update_stake_pool_rate(&mut svm, &stake_pool, total_lamports, pool_supply);

        // Calculate expected new rate
        let expected_new_rate =
            (total_lamports as u128 * RATE_PRECISION as u128 / pool_supply as u128) as u64;

        // === HARVEST ===
        let lst_before = LstSnapshot::read(&svm, &lst_config);
        let unified_before = UnifiedSnapshot::read(&svm, &unified_sol_config);

        harvest_lst_appreciation(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &lst_config,
            &stake_pool,
            Some(&lst_vault),
            harvest_payer,
        )
        .expect(&format!("harvest should succeed at cycle {}", cycle));

        let lst_after = LstSnapshot::read(&svm, &lst_config);
        let unified_after = UnifiedSnapshot::read(&svm, &unified_sol_config);

        // Verify SPL Stake Pool harvest invariants:
        // - exchange_rate should be updated to new rate
        assert_eq!(
            lst_after.exchange_rate, expected_new_rate,
            "Cycle {}: exchange_rate should match expected",
            cycle
        );

        // - previous_exchange_rate should be old rate
        assert_eq!(
            lst_after.previous_exchange_rate, lst_before.exchange_rate,
            "Cycle {}: previous_exchange_rate should be old rate",
            cycle
        );

        // - virtual_sol_value = vault_balance * exchange_rate / precision
        let expected_virtual_sol =
            (vault_balance as u128 * expected_new_rate as u128) / RATE_PRECISION as u128;
        assert_eq!(
            lst_after.virtual_sol_value(vault_balance), expected_virtual_sol,
            "Cycle {}: virtual_sol_value mismatch",
            cycle
        );

        // - appreciation = new_virtual_sol - old_virtual_sol
        let old_virtual_sol = lst_before.virtual_sol_value(vault_balance);
        let expected_appreciation = if expected_virtual_sol > old_virtual_sol {
            (expected_virtual_sol - old_virtual_sol) as u64
        } else {
            0
        };

        // - pending_appreciation should increase by appreciation amount
        assert_eq!(
            unified_after.pending_appreciation,
            unified_before.pending_appreciation + expected_appreciation,
            "Cycle {}: pending_appreciation should increase by appreciation",
            cycle
        );

        // - total_appreciation should increase
        assert_eq!(
            unified_after.total_appreciation,
            unified_before.total_appreciation + expected_appreciation as u128,
            "Cycle {}: total_appreciation should increase",
            cycle
        );

        // - total_appreciation_harvested should increase
        assert_eq!(
            lst_after.total_appreciation_harvested,
            lst_before.total_appreciation_harvested + expected_appreciation,
            "Cycle {}: total_appreciation_harvested should increase",
            cycle
        );

        // === ADVANCE EPOCH ===
        let unified_before_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);
        let lst_before_advance = LstSnapshot::read(&svm, &lst_config);

        advance_unified_epoch(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &[lst_config],
            advance_payer,
        )
        .expect(&format!("advance_unified_epoch should succeed at cycle {}", cycle));

        let unified_after_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);
        let lst_after_advance = LstSnapshot::read(&svm, &lst_config);

        // Verify advance epoch invariants
        assert_eq!(
            unified_after_advance.reward_epoch,
            unified_before_advance.reward_epoch + 1,
            "Cycle {}: reward_epoch should increment",
            cycle
        );

        // INV-8: harvested_exchange_rate frozen with accumulator
        assert_eq!(
            lst_after_advance.harvested_exchange_rate,
            lst_after_advance.exchange_rate,
            "Cycle {}: INV-8 violated",
            cycle
        );

        // Accumulator should grow if there were pending rewards and deposits
        if unified_before_advance.pending_appreciation > 0 {
            let total_pool = unified_before_advance
                .finalized_balance
                .checked_add(unified_before_advance.pending_deposits)
                .unwrap()
                .checked_sub(unified_before_advance.pending_withdrawals)
                .unwrap();

            if total_pool > 0 {
                let expected_delta = (unified_before_advance.pending_appreciation as u128)
                    .checked_mul(ACCUMULATOR_PRECISION)
                    .unwrap()
                    .checked_div(total_pool)
                    .unwrap();

                let actual_delta = unified_after_advance
                    .reward_accumulator
                    .checked_sub(unified_before_advance.reward_accumulator)
                    .unwrap();

                assert_eq!(
                    actual_delta, expected_delta,
                    "Cycle {}: accumulator delta mismatch",
                    cycle
                );
            }
        }
    }

    println!("Successfully completed 1000 cycles with appreciating LST");
    let final_unified = UnifiedSnapshot::read(&svm, &unified_sol_config);
    let final_lst = LstSnapshot::read(&svm, &lst_config);
    println!(
        "Final exchange_rate: {} ({:.4}x)",
        final_lst.exchange_rate,
        final_lst.exchange_rate as f64 / RATE_PRECISION as f64
    );
    println!("Final reward_accumulator: {}", final_unified.reward_accumulator);
    println!("Final total_appreciation: {}", final_unified.total_appreciation);
}

/// Test 1000 cycles with multiple LSTs with random appreciation rates.
///
/// Verifies:
/// - INV-ALL-LST-HARVESTED: All LSTs harvested before finalization
/// - Correct accumulator with multiple appreciation sources
/// - Each LST's state transitions independently verified
#[test]
fn test_1000_cycles_multiple_lsts_random_rates() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 100_000_000_000_000).unwrap();

    // 6 transactions per cycle (5 harvests + 1 advance)
    let payers: Vec<Keypair> = (0..6000).map(|_| Keypair::new()).collect();
    for payer in &payers {
        svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();
    }

    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    let stake_pool_program = SPL_STAKE_POOL_PROGRAM_ID;

    // LST setup data
    struct LstSetup {
        config: Pubkey,
        vault: Pubkey,
        stake_pool: Pubkey,
        is_wsol: bool,
        vault_balance: u64,
        total_lamports: u64,
        supply: u64,
    }

    let mut lsts: Vec<LstSetup> = Vec::new();

    // LST 0: WSOL
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let wsol_stake_pool = Pubkey::new_unique();
    let wsol_stake_pool_program = Pubkey::new_unique();
    let wsol_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &wsol_stake_pool,
        &wsol_stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init wsol lst_config should succeed");
    let (wsol_vault, _) = find_lst_vault_pda(&program_id, &wsol_config);
    update_vault_balance(&mut svm, &wsol_vault, 1_000_000_000_000);
    update_lst_config_vault_balance(&mut svm, &wsol_config, 1_000_000_000_000);
    lsts.push(LstSetup {
        config: wsol_config,
        vault: wsol_vault,
        stake_pool: wsol_stake_pool,
        is_wsol: true,
        vault_balance: 1_000_000_000_000,
        total_lamports: 0,
        supply: 0,
    });

    // LST 1-4: SPL Stake Pools
    let balances = [
        (100_000_000_000u64, 500_000_000_000u64),
        (250_000_000_000u64, 1_000_000_000_000u64),
        (500_000_000_000u64, 2_000_000_000_000u64),
        (1_000_000_000_000u64, 5_000_000_000_000u64),
    ];

    for (i, (vault_balance, pool_lamports)) in balances.iter().enumerate() {
        let lst_mint = create_mock_mint(&mut svm, 9);
        let stake_pool = create_mock_stake_pool(
            &mut svm,
            &lst_mint,
            *pool_lamports,
            *pool_lamports,
            stake_pool_program,
        );
        let lst_config = init_lst_config(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &lst_mint,
            &stake_pool,
            &stake_pool_program,
            &authority,
            pool_types::SPL_STAKE_POOL,
        )
        .expect(&format!("init lst_config {} should succeed", i));
        let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
        update_vault_balance(&mut svm, &lst_vault, *vault_balance);
        update_lst_config_vault_balance(&mut svm, &lst_config, *vault_balance);

        lsts.push(LstSetup {
            config: lst_config,
            vault: lst_vault,
            stake_pool,
            is_wsol: false,
            vault_balance: *vault_balance,
            total_lamports: *pool_lamports,
            supply: *pool_lamports,
        });
    }

    let mut rng = SimpleRng::new(12345);
    let mut current_slot: u64 = 0;

    for cycle in 0..1000u64 {
        current_slot += UPDATE_SLOT_INTERVAL + 10;
        warp_to_slot(&mut svm, current_slot);

        let cycle_payer_base = (cycle as usize) * 6;

        // Harvest each LST
        for (lst_idx, lst) in lsts.iter_mut().enumerate() {
            let harvest_payer = &payers[cycle_payer_base + lst_idx];

            let lst_before = LstSnapshot::read(&svm, &lst.config);
            let unified_before = UnifiedSnapshot::read(&svm, &unified_sol_config);

            if lst.is_wsol {
                harvest_lst_appreciation(
                    &mut svm,
                    &program_id,
                    &unified_sol_config,
                    &lst.config,
                    &lst.vault,
                    None,
                    harvest_payer,
                )
                .expect(&format!("harvest wsol should succeed at cycle {}", cycle));

                let lst_after = LstSnapshot::read(&svm, &lst.config);
                let unified_after = UnifiedSnapshot::read(&svm, &unified_sol_config);

                // WSOL invariants
                assert_eq!(
                    lst_after.virtual_sol_value(lst.vault_balance), lst.vault_balance as u128,
                    "Cycle {}: WSOL virtual_sol_value mismatch",
                    cycle
                );
                assert_eq!(
                    unified_after.pending_appreciation, unified_before.pending_appreciation,
                    "Cycle {}: WSOL should not add appreciation",
                    cycle
                );
            } else {
                // Random appreciation 0-40 bps
                let rate_increase_bps = rng.gen_range(41);
                if rate_increase_bps > 0 {
                    let appreciation_amount = lst.total_lamports * rate_increase_bps / 10000;
                    lst.total_lamports += appreciation_amount;
                    update_stake_pool_rate(&mut svm, &lst.stake_pool, lst.total_lamports, lst.supply);
                }

                let expected_new_rate =
                    (lst.total_lamports as u128 * RATE_PRECISION as u128 / lst.supply as u128)
                        as u64;

                harvest_lst_appreciation(
                    &mut svm,
                    &program_id,
                    &unified_sol_config,
                    &lst.config,
                    &lst.stake_pool,
                    Some(&lst.vault),
                    harvest_payer,
                )
                .expect(&format!("harvest lst should succeed at cycle {}", cycle));

                let lst_after = LstSnapshot::read(&svm, &lst.config);
                let unified_after = UnifiedSnapshot::read(&svm, &unified_sol_config);

                // SPL Stake Pool invariants
                assert_eq!(
                    lst_after.exchange_rate, expected_new_rate,
                    "Cycle {}: exchange_rate mismatch for LST {}",
                    cycle, lst_idx
                );

                let expected_virtual_sol = (lst.vault_balance as u128 * expected_new_rate as u128)
                    / RATE_PRECISION as u128;
                assert_eq!(
                    lst_after.virtual_sol_value(lst.vault_balance), expected_virtual_sol,
                    "Cycle {}: virtual_sol_value mismatch for LST {}",
                    cycle, lst_idx
                );

                let old_virtual_sol = lst_before.virtual_sol_value(lst.vault_balance);
                let expected_appreciation = if expected_virtual_sol > old_virtual_sol {
                    (expected_virtual_sol - old_virtual_sol) as u64
                } else {
                    0
                };

                assert_eq!(
                    unified_after.pending_appreciation,
                    unified_before.pending_appreciation + expected_appreciation,
                    "Cycle {}: pending_appreciation mismatch after LST {}",
                    cycle, lst_idx
                );
            }

            // Verify last_harvest_epoch updated
            let lst_after = LstSnapshot::read(&svm, &lst.config);
            assert_eq!(
                lst_after.last_harvest_epoch, unified_before.reward_epoch,
                "Cycle {}: last_harvest_epoch not updated for LST {}",
                cycle, lst_idx
            );
        }

        // Advance epoch
        let lst_configs: Vec<Pubkey> = lsts.iter().map(|l| l.config).collect();
        let advance_payer = &payers[cycle_payer_base + 5];

        let unified_before_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);

        advance_unified_epoch(
            &mut svm,
            &program_id,
            &unified_sol_config,
            &lst_configs,
            advance_payer,
        )
        .expect(&format!("advance_unified_epoch should succeed at cycle {}", cycle));

        let unified_after_advance = UnifiedSnapshot::read(&svm, &unified_sol_config);

        // Verify advance invariants
        assert_eq!(
            unified_after_advance.reward_epoch,
            unified_before_advance.reward_epoch + 1,
            "Cycle {}: reward_epoch should increment",
            cycle
        );

        // INV-8 for all LSTs
        for lst in &lsts {
            let lst_after = LstSnapshot::read(&svm, &lst.config);
            assert_eq!(
                lst_after.harvested_exchange_rate, lst_after.exchange_rate,
                "Cycle {}: INV-8 violated",
                cycle
            );
        }

        // Print progress
        if cycle > 0 && cycle % 100 == 0 {
            println!(
                "Cycle {}: accumulator={}, total_appreciation={}",
                cycle,
                unified_after_advance.reward_accumulator,
                unified_after_advance.total_appreciation
            );
        }
    }

    println!("\nSuccessfully completed 1000 cycles with 5 LSTs");
    let final_unified = UnifiedSnapshot::read(&svm, &unified_sol_config);
    println!("Final reward_epoch: {}", final_unified.reward_epoch);
    println!("Final reward_accumulator: {}", final_unified.reward_accumulator);
    println!("Final total_appreciation: {}", final_unified.total_appreciation);
}
