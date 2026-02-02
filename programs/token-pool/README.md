# Token Pool

A Solana program implementing a 1:1 SPL token vault plugin for the Zorb shielded pool system.

## Overview

Token Pool is a **plugin program** invoked by the Shielded Pool hub via CPI. It manages:
- SPL token deposits (user -> vault transfers)
- SPL token withdrawals (vault -> recipient transfers via delegation)
- Pool state, fees, and reward accumulators
- Reward finalization every ~5 minutes (750 slots)

**Program ID:** `tokucUdUVP8k9xMS98cnVFmy4Yg3zkKmjfmGuYma8ah`

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
│          TOKEN-POOL                 │
│  • 1:1 token vault                  │
│  • Deposit: user → vault            │
│  • Withdraw: vault → user           │
│  • Reward accumulator               │
└─────────────────────────────────────┘
```

## Structure

- `src/lib.rs` - Program entry point with panchor macro
- `src/instructions/` - Instruction handlers
  - `deposit.rs` - Handle deposits from hub CPI
  - `withdraw.rs` - Handle withdrawals from hub CPI
  - `init_pool.rs` - Initialize new token pool
  - `set_pool_active.rs` - Enable/disable pool
  - `set_fee_rates.rs` - Configure fee rates
  - `finalize_rewards.rs` - Finalize pending rewards
  - `fund_rewards.rs` - External reward funding
  - `authority/` - Two-step authority transfer
- `src/state.rs` - TokenPoolConfig account definition

## Building

```bash
# Build the program
cargo build-sbf -p token-pool

# Build for tests
cargo build -p token-pool
```

## Testing

```bash
# Run all tests
cargo test -p token-pool

# Run with output
cargo test -p token-pool -- --nocapture
```

## Instructions

### Pool Operations (CPI from Shielded Pool)

| Disc | Instruction | Description |
|------|-------------|-------------|
| 0 | `Deposit` | Transfer tokens from depositor to vault, update pending balances |
| 1 | `Withdraw` | Validate amounts, approve hub_authority for output, update state |

### Admin Operations

| Disc | Instruction | Description |
|------|-------------|-------------|
| 64 | `InitPool` | Initialize a new token pool with vault |
| 65 | `SetPoolActive` | Enable/disable the pool |
| 66 | `SetFeeRates` | Configure deposit/withdrawal fee rates |
| 67 | `FinalizeRewards` | Finalize pending rewards (permissionless) |
| 68 | `FundRewards` | Fund reward pool externally (permissionless) |
| 69 | `Log` | Emit events via CPI |

### Authority Management

| Disc | Instruction | Description |
|------|-------------|-------------|
| 192 | `TransferAuthority` | Initiate two-step authority transfer |
| 193 | `AcceptAuthority` | Complete two-step authority transfer |

## Accounts

### TokenPoolConfig

The main state account for each token pool.

**Seeds:** `["token_pool", mint]`

**Fields:**
```rust
// Core Identity
authority: Pubkey,           // Pool authority
pending_authority: Pubkey,   // For two-step transfer
mint: Pubkey,                // Token mint address
vault: Pubkey,               // Vault token account PDA
asset_id: [u8; 32],          // Poseidon hash of mint

// Balance Tracking (token base units)
finalized_balance: u128,     // Frozen at last finalization
pending_deposits: u128,      // Deposits since finalization
pending_withdrawals: u128,   // Withdrawals since finalization
pending_rewards: u64,        // Fees waiting to be distributed

// Reward Accumulator
reward_accumulator: u128,    // Cumulative rewards per unit (scaled by 1e18)
last_finalized_slot: u64,    // When finalization last occurred

// Fee Configuration (basis points)
deposit_fee_rate: u16,       // e.g., 100 = 1%
withdrawal_fee_rate: u16,
decimals: u8,                // Token decimals

// Statistics
total_deposited: u128,
total_withdrawn: u128,
total_deposit_fees: u64,
total_withdrawal_fees: u64,
total_funded_rewards: u64,
deposit_count: u64,
withdrawal_count: u64,
```

## Reward Accumulator

The reward accumulator enables fair distribution of fees to pool participants:

### How It Works

1. **Accumulation**: When rewards are finalized, the accumulator increases:
   ```
   reward_accumulator += (pending_rewards * 1e18) / total_pool
   ```

2. **User Rewards**: The ZK circuit calculates user rewards as:
   ```
   user_reward = user_amount * (current_accumulator - entry_accumulator) / 1e18
   ```

3. **Finalization Interval**: Every 750 slots (~5 minutes)

### Finalization Flow

```
1. Check if 750+ slots elapsed since last_finalized_slot
2. Calculate total_pool = finalized_balance + pending_deposits - pending_withdrawals
3. Update accumulator: accumulator += (pending_rewards * 1e18) / total_pool
4. Reset pending fields, update finalized_balance
5. Increment last_finalized_slot
```

### Privacy Properties

- `finalized_balance` is frozen between finalization events
- Deposits/withdrawals never touch `finalized_balance` during normal operation
- This prevents timing attacks that could correlate deposits with withdrawals

## CPI Interface

### Deposit Flow

```
Hub calls: Deposit { amount, expected_output }
Pool:
  1. Transfer `amount` tokens from depositor to vault
  2. Update pending_deposits += amount
  3. Return { fee } via return data
```

### Withdrawal Flow

```
Hub calls: Withdraw { amount, expected_output }
Pool:
  1. Approve hub_authority for `expected_output` tokens
  2. Update pending_withdrawals += amount
  3. Return { fee } via return data
Hub then:
  1. Transfer vault -> recipient (expected_output)
```

## Events

Events are emitted via self-CPI through the `Log` instruction. Each event has a unique discriminator.

### Core Events (1-15)

| Disc | Event | Description |
|------|-------|-------------|
| 1 | `TokenDeposit` | Token deposit completed. Includes mint, amount, fee, net_amount, new_balance, slot |
| 2 | `TokenWithdrawal` | Token withdrawal completed. Includes mint, amount, fee, new_balance, slot |
| 3 | `TokenRewardsFinalized` | Reward accumulator updated. Includes mint, pending_rewards, new_accumulator, total_pool, slot |

## Deployment

### Prerequisites

1. Solana CLI tools installed
2. Funded keypair for deployment

### Build and Deploy

```bash
# Build
cargo build-sbf -p token-pool

# Deploy to devnet
solana config set --url devnet
solana program deploy ./target/deploy/token_pool.so

# Deploy to mainnet
solana config set --url mainnet-beta
solana program write-buffer ./target/deploy/token_pool.so
solana program deploy --buffer <BUFFER_PUBKEY> --program-id <PROGRAM_KEYPAIR>
```

### Initialization

After deployment:

1. **Initialize Pool** for each supported token:
   ```typescript
   await program.methods.initPool({
     depositFeeRate: 50,     // 0.5%
     withdrawalFeeRate: 50,  // 0.5%
   }).accounts({
     authority: admin,
     mint: tokenMint,
   }).rpc();
   ```

2. **Register with Shielded Pool Hub**:
   ```typescript
   // In shielded-pool program
   await hubProgram.methods.registerTokenPool({
     poolProgram: TOKEN_POOL_PROGRAM_ID,
     assetId: computeAssetId(tokenMint),
   }).rpc();
   ```

## Security Considerations

- Only the shielded pool hub should call deposit/withdraw instructions
- Authority transfer is two-step to prevent accidental transfers
- Finalization is permissionless but has time-based rate limiting
- Use multisig for authority in production

## Related Documentation

- Protocol specification: `../../../app/docs/protocol/specification.md`
- Shielded pool hub: `../shielded-pool/README.md`
- Unified SOL pool: `../unified-sol-pool/README.md`
