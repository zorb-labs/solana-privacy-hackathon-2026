# Unified SOL Pool

A Solana program implementing a multi-LST (Liquid Staking Token) vault plugin for the Zorb shielded pool system.

## Overview

Unified SOL Pool is a **plugin program** invoked by the Shielded Pool hub via CPI. It enables fungible deposits and withdrawals across different liquid staking tokens:

- **Cross-LST fungibility**: Deposit vSOL, withdraw jitoSOL
- **Virtual SOL tracking**: All LSTs converted to common unit
- **LST appreciation harvesting**: Staking rewards distributed to users
- **Exchange rate management**: Per-LST rates updated from stake pools

**Program ID:** `unixG6MuVwukHrmCbn4oE8LAPYKDfDMyNtNuMSEYJmi`

## Architecture

```
┌─────────────────────────────────────┐
│        SHIELDED-POOL (Hub)          │
│  • Verifies ZK proofs               │
│  • Calculates fees                  │
│  • Routes deposits/withdrawals      │
└─────────────────────────────────────┘
                 │
                 ▼ CPI
┌─────────────────────────────────────┐
│       UNIFIED-SOL-POOL              │
│  ┌─────────────────────────────┐    │
│  │  UnifiedSolPoolConfig       │    │
│  │  • Total virtual SOL        │    │
│  │  • Reward accumulator       │    │
│  │  • Fee rates                │    │
│  └─────────────────────────────┘    │
│           │                         │
│  ┌────────┼────────┬────────┐       │
│  ▼        ▼        ▼        ▼       │
│ WSOL    vSOL   jitoSOL   mSOL       │
│ (1:1)  (rate)  (rate)   (rate)      │
│         LstConfig accounts          │
└─────────────────────────────────────┘
```

## Structure

- `src/lib.rs` - Program entry point with panchor macro
- `src/instructions/` - Instruction handlers
  - `deposit.rs` - Handle deposits from hub CPI
  - `withdraw.rs` - Handle withdrawals from hub CPI
  - `init_unified_sol_pool_config.rs` - Initialize master config
  - `init_lst_config.rs` - Initialize per-LST configuration
  - `set_unified_sol_pool_config_active.rs` - Enable/disable pool
  - `set_lst_config_active.rs` - Enable/disable specific LST
  - `set_unified_sol_pool_config_fee_rates.rs` - Configure fees
  - `finalize_unified_rewards.rs` - Finalize rewards
  - `harvest_lst_appreciation.rs` - Harvest LST gains
  - `authority/` - Two-step authority transfer
- `src/state.rs` - Account definitions

## Building

```bash
# Build the program
cargo build-sbf -p unified-sol-pool

# Build for tests
cargo build -p unified-sol-pool
```

## Testing

```bash
# Run all tests
cargo test -p unified-sol-pool

# Run with output
cargo test -p unified-sol-pool -- --nocapture
```

## Instructions

### Pool Operations (CPI from Shielded Pool)

| Disc | Instruction | Description |
|------|-------------|-------------|
| 0 | `Deposit` | Transfer LST tokens to vault, credit virtual SOL |
| 1 | `Withdraw` | Transfer LST tokens from vault, debit virtual SOL |

### Admin Operations

| Disc | Instruction | Description |
|------|-------------|-------------|
| 64 | `InitUnifiedSolPoolConfig` | Initialize master config (singleton) |
| 65 | `InitLstConfig` | Initialize new LST configuration |
| 66 | `SetUnifiedSolPoolConfigActive` | Enable/disable unified pool |
| 67 | `SetLstConfigActive` | Enable/disable specific LST |
| 68 | `SetUnifiedSolPoolConfigFeeRates` | Configure fee rates |
| 69 | `FinalizeUnifiedRewards` | Finalize pending rewards (permissionless) |
| 70 | `HarvestLstAppreciation` | Harvest LST appreciation (permissionless) |
| 71 | `Log` | Emit events via CPI |

### Authority Management

| Disc | Instruction | Description |
|------|-------------|-------------|
| 192 | `TransferAuthority` | Initiate two-step authority transfer |
| 193 | `AcceptAuthority` | Complete two-step authority transfer |

## Accounts

### UnifiedSolPoolConfig

Master configuration account (singleton).

**Seeds:** `["unified_sol_pool"]`

**Fields:**
```rust
// Identity
asset_id: [u8; 32],          // Fixed: [0x00...0x01] (value 1, big-endian)
authority: Pubkey,           // Pool authority
pending_authority: Pubkey,   // For two-step transfer
reward_epoch: u64,           // Increments on each finalization

// Virtual SOL Tracking (lamports = 1e9 per SOL)
total_virtual_sol: u128,     // Total across all LSTs
finalized_balance: u128,     // Frozen at last finalization
pending_deposits: u128,      // Deposits since finalization
pending_withdrawals: u128,   // Withdrawals since finalization

// Reward Accumulator
reward_accumulator: u128,    // Cumulative rewards per unit (scaled by 1e18)
pending_rewards: u64,        // Fees + appreciation waiting to distribute
last_finalized_slot: u64,    // When finalization last occurred

// Fee Configuration (basis points)
deposit_fee_rate: u16,       // e.g., 100 = 1%
withdrawal_fee_rate: u16,

// Buffer Management
min_buffer_bps: u16,         // Minimum WSOL percentage (e.g., 2000 = 20%)
min_buffer_amount: u64,      // Absolute minimum WSOL amount

// Statistics
total_deposited: u128,       // Virtual SOL
total_withdrawn: u128,       // Virtual SOL
total_rewards_distributed: u64,
total_appreciation: u64,
total_deposit_fees: u64,
total_withdrawal_fees: u64,
deposit_count: u64,
withdrawal_count: u64,
lst_count: u8,               // Number of registered LSTs
```

### LstConfig

Per-LST configuration account.

**Seeds:** `["lst_config", lst_mint]`

**Fields:**
```rust
// References
lst_mint: Pubkey,            // LST token mint
stake_pool: Pubkey,          // Underlying stake pool address
stake_pool_program: Pubkey,  // Stake pool program ID
lst_vault: Pubkey,           // PDA token account for this LST
pool_type: PoolType,         // Wsol, SplStakePool, Marinade, Lido

// Exchange Rate
exchange_rate: u64,          // 1 LST = exchange_rate/1e9 SOL
previous_exchange_rate: u64, // For appreciation calculation
last_rate_update_slot: u64,  // When rate was last updated
harvested_exchange_rate: u64,// Rate frozen at finalization

// Virtual SOL Value
virtual_sol_value: u128,     // Cached virtual SOL equivalent

// Epoch Tracking
last_harvest_epoch: u64,     // When LST was last harvested

// Statistics
total_deposited: u128,       // LST token units
total_withdrawn: u128,       // LST token units
total_appreciation_harvested: u64, // Virtual SOL
deposit_count: u64,
withdrawal_count: u64,
```

## Exchange Rate Model

### Rate Definition

```
1 LST = exchange_rate / 1e9 SOL

Example: rate 1,050,000,000 means 1 LST = 1.05 SOL
```

### Conversions

```rust
// Deposit: LST tokens -> Virtual SOL
virtual_sol = lst_tokens * exchange_rate / 1e9

// Withdrawal: Virtual SOL -> LST tokens
lst_tokens = virtual_sol * 1e9 / exchange_rate
```

### Rate Constraints

- Max change per harvest: 50 basis points (0.5%)
- WSOL always has rate = 1e9 (1:1)
- Rates updated from on-chain stake pool state

## Supported Pool Types

```rust
enum PoolType {
    Wsol = 0,           // Wrapped SOL (1:1 rate)
    SplStakePool = 1,   // Jito, Sanctum, etc.
    Marinade = 2,       // Marinade stake pool
    Lido = 3,           // Lido stake pool
}
```

## LST Appreciation Harvesting

When LST exchange rates increase, the appreciation is harvested and distributed:

### Process

```
1. Read current exchange rate from stake pool
2. Calculate appreciation:
   old_value = vault_balance * old_rate / 1e9
   new_value = vault_balance * new_rate / 1e9
   appreciation = new_value - old_value
3. Add appreciation to pending_rewards
4. Update exchange rate and last_rate_update_slot
5. Distribute on next finalize_unified_rewards
```

### Constraints

- Harvesting requires rate increase > 0
- Rate capped at 50 bps increase per harvest
- Each LST tracks its own last_harvest_epoch

## Reward Finalization

Similar to Token Pool, but includes LST appreciation:

```
1. Check if 750+ slots elapsed since last_finalized_slot
2. Calculate total_pool = finalized_balance + pending_deposits - pending_withdrawals
3. pending_rewards includes both fees AND harvested appreciation
4. Update accumulator: accumulator += (pending_rewards * 1e18) / total_pool
5. Reset pending fields, update finalized_balance
6. Increment reward_epoch
```

## Buffer Management

To ensure withdrawal liquidity, the pool maintains a WSOL buffer:

```rust
// Required buffer calculation
required = max(
    total_virtual_sol * min_buffer_bps / 10000,
    min_buffer_amount
)
```

Example: With 20% buffer and 100 SOL total, 20 SOL must remain as WSOL.

## CPI Interface

### Deposit Flow

```
Hub calls: Deposit { amount, expected_output, lst_mint }
Pool:
  1. Transfer LST tokens from depositor to vault
  2. Convert to virtual SOL: virtual_sol = amount * rate / 1e9
  3. Update pending_deposits += virtual_sol
  4. Return { fee } via return data
```

### Withdrawal Flow

```
Hub calls: Withdraw { amount, expected_output, lst_mint }
Pool:
  1. Convert virtual SOL to LST tokens: lst_tokens = amount * 1e9 / rate
  2. Approve hub_authority for lst_tokens
  3. Update pending_withdrawals += amount (virtual SOL)
  4. Return { fee } via return data
Hub then:
  1. Transfer vault -> recipient (tokens)
```

## Cross-LST Fungibility

The key feature enabling privacy-preserving LST swaps:

```
User deposits 10 vSOL (rate: 1.05 SOL)
  → Credits 10.5 virtual SOL to commitment

Later, user withdraws 10 jitoSOL (rate: 1.04 SOL)
  → Debits ~10.92 virtual SOL from note
  → User receives 10 jitoSOL

Net effect: Converted vSOL to jitoSOL privately
```

## Events

Events are emitted via self-CPI through the `Log` instruction. Each event has a unique discriminator.

### Core Events (1-15)

| Disc | Event | Description |
|------|-------|-------------|
| 1 | `UnifiedSolDeposit` | LST deposit completed. Includes lst_mint, lst_amount, sol_value, fee, exchange_rate, slot |
| 2 | `UnifiedSolWithdrawal` | LST withdrawal completed. Includes lst_mint, lst_amount, sol_value, fee, exchange_rate, slot |
| 3 | `UnifiedSolRewardsFinalized` | Reward accumulator updated. Includes total_virtual_sol, reward_delta, new_accumulator, pending_rewards, epoch, slot |

### LST Events (16-31)

| Disc | Event | Description |
|------|-------|-------------|
| 16 | `AppreciationHarvested` | LST appreciation captured. Includes lst_mint, previous_rate, current_rate, appreciation_amount, epoch, slot |
| 17 | `ExchangeRateUpdated` | Exchange rate updated for an LST. Includes lst_mint, previous_rate, current_rate, slot |

## Deployment

### Prerequisites

1. Solana CLI tools installed
2. Funded keypair for deployment

### Build and Deploy

```bash
# Build
cargo build-sbf -p unified-sol-pool

# Deploy to devnet
solana config set --url devnet
solana program deploy ./target/deploy/unified_sol_pool.so

# Deploy to mainnet
solana config set --url mainnet-beta
solana program write-buffer ./target/deploy/unified_sol_pool.so
solana program deploy --buffer <BUFFER_PUBKEY> --program-id <PROGRAM_KEYPAIR>
```

### Initialization

After deployment:

1. **Initialize UnifiedSolPoolConfig** (singleton):
   ```typescript
   await program.methods.initUnifiedSolPoolConfig({
     depositFeeRate: 50,     // 0.5%
     withdrawalFeeRate: 50,  // 0.5%
     minBufferBps: 2000,     // 20%
     minBufferAmount: new BN(1_000_000_000), // 1 SOL minimum
   }).accounts({
     authority: admin,
   }).rpc();
   ```

2. **Initialize LstConfig** for each supported LST:
   ```typescript
   await program.methods.initLstConfig({
     poolType: { splStakePool: {} },  // or wsol, marinade, lido
   }).accounts({
     authority: admin,
     lstMint: jitoSolMint,
     stakePool: jitoStakePool,
     stakePoolProgram: SPL_STAKE_POOL_PROGRAM,
   }).rpc();
   ```

3. **Register with Shielded Pool Hub**:
   ```typescript
   // In shielded-pool program
   await hubProgram.methods.registerUnifiedSolPool({
     poolProgram: UNIFIED_SOL_POOL_PROGRAM_ID,
     assetId: UNIFIED_SOL_ASSET_ID,  // [0x00...0x01]
   }).rpc();
   ```

## Security Considerations

- Only the shielded pool hub should call deposit/withdraw instructions
- Exchange rate changes are capped to prevent manipulation
- Authority transfer is two-step to prevent accidental transfers
- Harvesting and finalization are permissionless but rate-limited
- Buffer requirements prevent liquidity drain attacks
- Use multisig for authority in production

## Audit TODOs

### Exchange Rate Invariant: `rate >= 1e9`

The `virtual_sol_to_tokens` conversion function contains an unchecked `u128 as u64` cast that is only safe when `exchange_rate >= RATE_PRECISION (1e9)`. This invariant is critical for correct operation.

**Current enforcement points:**

| Location | Type | Description |
|----------|------|-------------|
| `init_lst_config.rs:210-226` | Initialization | Rates initialized to exactly `RATE_PRECISION` |
| `harvest_lst_appreciation.rs:124` | Runtime check | **SINGLE** check that rejects `rate < RATE_PRECISION` |

**Audit items:**

1. [ ] **Defense-in-depth**: Add explicit `rate >= RATE_PRECISION` check in `virtual_sol_to_tokens` itself
2. [ ] **Invariant documentation**: Document the rate >= 1e9 invariant in `LstConfig` struct
3. [ ] **Fuzz testing**: Add property-based tests that verify the invariant holds across all code paths
4. [ ] **Consider checked cast**: Replace `result as u64` with `u64::try_from(result)?` in `virtual_sol_to_tokens`

**Risk if violated**: If `exchange_rate < 1e9`, the cast truncates silently, causing users to receive fewer tokens than expected on withdrawal.

## Related Documentation

- Protocol specification: `../../../app/docs/protocol/specification.md`
- Multi-LST design: `../../../app/docs/protocol/multi-lst.md`
- Shielded pool hub: `../shielded-pool/README.md`
- Token pool: `../token-pool/README.md`
