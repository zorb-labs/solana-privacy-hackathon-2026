# Yield-Bearing Mechanism

Zorb implements a privacy-preserving yield system that allows shielded deposits to earn rewards without revealing individual balances. This document explains the epoch-based reward accumulator model used by both Token Pool and Unified SOL Pool.

## Overview

The yield mechanism distributes rewards to depositors proportionally based on:
1. **Token Pool**: Transaction fees + externally funded rewards
2. **Unified SOL Pool**: Transaction fees + LST staking appreciation

Rewards are tracked using a **reward accumulator** - a running total of rewards-per-unit that increases over time. Users claim rewards in ZK proofs by computing the difference between current and entry accumulators.

## Epoch Model

### Finalization Epochs

Both pools use slot-based epochs for reward distribution:

```
UPDATE_SLOT_INTERVAL = 2700 slots (~18 minutes at 400ms/slot)
```

Anyone can call `finalize_rewards` once the interval passes. This:
1. Freezes the current reward accumulator value
2. Allows clients to generate ZK proofs against the frozen value
3. Distributes pending rewards to the accumulator

### Unified SOL Pool: Harvest-Finalize Cycle

The Unified SOL Pool has a two-phase cycle:

```
┌─────────────────────────────────────────────────────────────────┐
│  Epoch N                                                        │
├─────────────────────────────────────────────────────────────────┤
│  1. harvest_lst_appreciation (per LST)                          │
│     - Reads current exchange rate from stake pool               │
│     - Calculates appreciation since last harvest                │
│     - Adds to pending_appreciation                              │
│                                                                 │
│  2. finalize_unified_rewards (once per epoch)                   │
│     - Validates ALL LSTs were harvested this epoch              │
│     - Distributes pending rewards via accumulator               │
│     - Freezes exchange rates (harvested_exchange_rate)          │
│     - Increments reward_epoch                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Reward Accumulator

### Formula

The accumulator represents cumulative rewards per unit of deposit:

```
accumulator += (pending_rewards * PRECISION) / total_pool

Where:
  PRECISION = 1e18 (for fixed-point arithmetic)
  total_pool = finalized_balance + pending_deposits - pending_withdrawals
```

### User Reward Calculation (In Circuit)

```
user_reward = user_amount * (current_accumulator - entry_accumulator) / 1e18
```

The ZK circuit (`circom/lib/rewards.circom`) implements this with `ComputeReward`:

```circom
template ComputeReward() {
    signal input amount;
    signal input globalAccumulator;    // Current global reward accumulator
    signal input noteAccumulator;      // Accumulator when note was created
    signal input remainder;            // Division hint for verification
    signal output reward;
    signal output totalValue;          // amount + reward

    signal accumulatorDiff <== globalAccumulator - noteAccumulator;
    signal unscaledReward <== amount * accumulatorDiff;

    // Scale down by ACCUMULATOR_SCALE (1e18)
    component scaler = DivByAccumulatorScale();
    scaler.dividend <== unscaledReward;
    scaler.remainder <== remainder;

    reward <== scaler.out;
    totalValue <== amount + reward;
}
```

## Token Pool

### Reward Sources

1. **Deposit fees**: Collected on shield operations
2. **Withdrawal fees**: Collected on unshield operations
3. **Funded rewards**: Externally added via `fund_rewards` instruction

### State Fields

```rust
pub struct TokenPoolConfig {
    /// Balance frozen at last finalization
    pub finalized_balance: u128,

    /// Cumulative rewards per unit, scaled by 1e18
    pub reward_accumulator: u128,

    /// Pending values (reset on finalization)
    pub pending_deposits: u128,
    pub pending_withdrawals: u128,
    pub pending_deposit_fees: u64,
    pub pending_withdrawal_fees: u64,
    pub pending_funded_rewards: u64,

    /// Slot when last finalized
    pub last_finalized_slot: u64,
}
```

### Finalization Flow

```rust
fn finalize_rewards(&mut self, current_slot: u64) {
    // 1. Check interval elapsed
    if current_slot < self.last_finalized_slot + UPDATE_SLOT_INTERVAL {
        return Err(RewardsNotReady);
    }

    // 2. Calculate total pool
    let total_pool = finalized_balance + pending_deposits - pending_withdrawals;

    // 3. Calculate and distribute rewards
    let total_pending = pending_deposit_fees + pending_withdrawal_fees + pending_funded_rewards;

    if total_pool > 0 && total_pending > 0 {
        let reward_delta = (total_pending * 1e18) / total_pool;
        self.reward_accumulator += reward_delta;
    }

    // 4. Update state
    self.finalized_balance = total_pool;
    self.pending_deposits = 0;
    self.pending_withdrawals = 0;
    self.pending_deposit_fees = 0;
    self.pending_withdrawal_fees = 0;
    self.pending_funded_rewards = 0;
    self.last_finalized_slot = current_slot;
}
```

## Unified SOL Pool

The Unified SOL Pool handles Liquid Staking Tokens (LSTs) with variable exchange rates.

### Exchange Rate Model

Each LST has two exchange rates:
- `exchange_rate`: Current rate (updated on harvest)
- `harvested_exchange_rate`: Frozen rate for ZK proofs

```
virtual_sol = lst_tokens * exchange_rate / 1e9
lst_tokens = virtual_sol * 1e9 / exchange_rate
```

### LST Appreciation

When LST exchange rates increase (staking rewards), the appreciation is captured:

```rust
fn update_exchange_rate(&mut self, vault_balance: u64, new_rate: u64) -> u64 {
    let old_rate = self.exchange_rate;

    if new_rate > old_rate {
        // Calculate appreciation in virtual SOL
        let old_virtual = (vault_balance * old_rate) / RATE_PRECISION;
        let new_virtual = (vault_balance * new_rate) / RATE_PRECISION;
        let appreciation = new_virtual - old_virtual;

        self.exchange_rate = new_rate;
        return appreciation;
    }

    0 // No appreciation if rate decreased or unchanged
}
```

### Harvest Validation

The `harvest_lst_appreciation` instruction:
1. Reads current exchange rate from stake pool on-chain account
2. Validates stake pool was updated in current Solana epoch (prevents stale data)
3. Enforces `MAX_RATE_CHANGE_BPS` (0.5%) to bound arbitrage window
4. Calculates appreciation and adds to `pending_appreciation`
5. Marks LST as harvested for current epoch

### Finalization Requirements

Before `finalize_unified_rewards` succeeds:
1. All registered LSTs must be passed as remaining accounts
2. All **active** LSTs must have `last_harvest_epoch == current_reward_epoch`
3. Exchange rates are frozen: `harvested_exchange_rate = exchange_rate`

## Fungibility

The Unified SOL Pool enables **LST fungibility**:
- Deposit jitoSOL, withdraw mSOL (or any supported LST)
- All LSTs are tracked in "virtual SOL" units
- Exchange rate conversion happens at deposit/withdraw time

```
User deposits 100 jitoSOL at rate 1.05:
  virtual_sol = 100 * 1.05 = 105 SOL equivalent

User withdraws in mSOL at rate 1.02:
  mSOL_received = 105 / 1.02 = 102.94 mSOL
```

## Security Properties

### Timing Attack Mitigation

The `MAX_RATE_CHANGE_BPS` (0.5%) bounds the harvest-finalize timing window:
- Attacker cannot exploit stale rates for more than 0.5% gain
- Entry cost at stale rate >= captured yield

### Epoch Freshness

LST harvest validates stake pool's `last_update_epoch == current_epoch`:
- Prevents reading stale stake rewards data at epoch boundaries
- Ensures exchange rates reflect actual on-chain state

### Accumulator Monotonicity

- Reward accumulator only increases (never decreases)
- Finalization always advances `last_finalized_slot`
- ZK proofs validate `entry_accumulator <= current_accumulator`

## Circuit Integration

The on-chain programs track accumulators; the ZK circuits verify reward claims:

1. **On-chain**: `finalize_rewards` updates `reward_accumulator`
2. **Client**: Reads current accumulator, builds transaction
3. **Circuit**: Verifies `reward = amount * (current_acc - entry_acc) / 1e18`
4. **On-chain**: Verifies Groth16 proof, allows withdrawal of `amount + reward`

This ensures users can claim accrued rewards without revealing their balance or entry time.
