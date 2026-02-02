# Vault Balance Invariant Proof

This document provides a formal proof that the token pool's vault balance tracking is correct, which establishes that `SweepExcess` correctly identifies tokens that arrived outside of program control.

## The Invariant

Under normal operation (no direct SPL token transfers to the vault), the vault balance satisfies:

```
vault.amount = total_deposited - total_withdrawn
             + total_deposit_fees + total_withdrawal_fees
             + total_funded_rewards
```

Where:
- `vault.amount` is the actual SPL token balance in the vault account
- `total_deposited` is the cumulative net deposit amount (gross - fee)
- `total_withdrawn` is the cumulative gross withdrawal amount
- `total_deposit_fees` is the cumulative fees collected from deposits
- `total_withdrawal_fees` is the cumulative fees collected from withdrawals
- `total_funded_rewards` is the cumulative externally funded reward amount

## Part 1: Completeness

**Claim:** Only 3 of the 11 instructions in `TokenPoolInstruction` can affect the vault balance or the tracked balance fields.

### Instruction Enumeration

| # | Instruction | Modifies Vault Balance? | Modifies Tracking Fields? |
|---|-------------|------------------------|---------------------------|
| 0 | `Deposit` | Yes (transfer IN) | Yes (`total_deposited`, `total_deposit_fees`) |
| 1 | `Withdraw` | Yes (approve OUT) | Yes (`total_withdrawn`, `total_withdrawal_fees`) |
| 64 | `InitPool` | No | No (initializes to zero) |
| 65 | `SetPoolActive` | No | No |
| 66 | `SetFeeRates` | No | No |
| 67 | `FinalizeRewards` | No | No (only updates accumulator) |
| 68 | `FundRewards` | Yes (transfer IN) | Yes (`total_funded_rewards`) |
| 69 | `Log` | No | No |
| 70 | `SweepExcess` | No | Yes (`total_funded_rewards`) - restores invariant |
| 192 | `TransferAuthority` | No | No |
| 193 | `AcceptAuthority` | No | No |

### Analysis of Non-Modifying Instructions

1. **InitPool (64)**: Creates the pool config and vault. All tracking fields are initialized to zero, and the vault starts empty. Invariant holds trivially: `0 = 0 - 0 + 0 + 0 + 0`.

2. **SetPoolActive (65)**: Only modifies the `is_active` boolean flag. Does not touch vault or any tracking fields.

3. **SetFeeRates (66)**: Only modifies `deposit_fee_rate` and `withdrawal_fee_rate`. Does not touch vault or any tracking fields.

4. **FinalizeRewards (67)**: Moves rewards from `pending_funded_rewards` to the accumulator. This is an internal redistribution that doesn't change `total_funded_rewards` or the vault balance.

5. **Log (69)**: Pure event emission via CPI. No state modifications.

6. **SweepExcess (70)**: Reads vault balance and tracking fields. Only modifies `total_funded_rewards` to capture excess, which by definition restores the invariant rather than violating it.

7. **TransferAuthority (192)**: Only modifies `pending_authority`. Does not touch vault or any tracking fields.

8. **AcceptAuthority (193)**: Only modifies `authority` and `pending_authority`. Does not touch vault or any tracking fields.

## Part 2: Correctness

**Claim:** Each vault-modifying operation maintains the invariant. We prove this by showing that for each operation, `Δ(vault.amount) = Δ(expected)`.

### Base Case: InitPool

After `InitPool`:
- `vault.amount = 0`
- `total_deposited = 0`
- `total_withdrawn = 0`
- `total_deposit_fees = 0`
- `total_withdrawal_fees = 0`
- `total_funded_rewards = 0`

Expected = `0 - 0 + 0 + 0 + 0 = 0`. Invariant holds. **QED**

### Inductive Step: Deposit

**Precondition:** Invariant holds before operation.

**Operation** (from `deposit.rs`):
1. User deposits `gross_amount = params.amount` tokens
2. Fee calculated: `fee = gross_amount * deposit_fee_rate / 10000`
3. Net deposit: `principal = gross_amount - fee`
4. Transfer `gross_amount` tokens from depositor to vault
5. Update: `total_deposited += principal`
6. Update: `total_deposit_fees += fee`

**Delta Analysis:**
```
Δ(vault.amount) = +gross_amount

Δ(total_deposited) = +principal = +(gross_amount - fee)
Δ(total_deposit_fees) = +fee

Δ(expected) = Δ(total_deposited) + Δ(total_deposit_fees)
            = (gross_amount - fee) + fee
            = gross_amount
```

**Conclusion:** `Δ(vault.amount) = Δ(expected) = gross_amount`. Invariant preserved. **QED**

### Inductive Step: Withdraw

**Precondition:** Invariant holds before operation.

**Operation** (from `withdraw.rs`):
1. User withdraws with `gross_amount = params.amount`
2. Fee calculated: `fee = gross_amount * withdrawal_fee_rate / 10000`
3. Output (what leaves vault): `output = gross_amount - fee`
4. Approve `output` tokens from vault (hub transfers them out)
5. Update: `total_withdrawn += gross_amount` (full amount, not output!)
6. Update: `total_withdrawal_fees += fee`

**Delta Analysis:**
```
Δ(vault.amount) = -output = -(gross_amount - fee)

Δ(total_withdrawn) = +gross_amount
Δ(total_withdrawal_fees) = +fee

Δ(expected) = -Δ(total_withdrawn) + Δ(total_withdrawal_fees)
            = -gross_amount + fee
            = -(gross_amount - fee)
            = -output
```

**Conclusion:** `Δ(vault.amount) = Δ(expected) = -output`. Invariant preserved. **QED**

### Inductive Step: FundRewards

**Precondition:** Invariant holds before operation.

**Operation** (from `fund_rewards.rs`):
1. Funder transfers `fund_amount = data.amount` tokens to vault
2. Update: `total_funded_rewards += fund_amount`

**Delta Analysis:**
```
Δ(vault.amount) = +fund_amount

Δ(total_funded_rewards) = +fund_amount

Δ(expected) = Δ(total_funded_rewards) = fund_amount
```

**Conclusion:** `Δ(vault.amount) = Δ(expected) = fund_amount`. Invariant preserved. **QED**

## Part 3: Corollary (SweepExcess Correctness)

**Claim:** SweepExcess correctly captures exactly the tokens that arrived outside program control.

### Proof

From Part 1 and Part 2, we have established that under normal program operation:

```
vault.amount = expected = total_deposited - total_withdrawn
                        + total_deposit_fees + total_withdrawal_fees
                        + total_funded_rewards
```

Now consider the case where tokens arrive in the vault outside program control (e.g., direct SPL token transfer):

1. Let `vault_actual` be the true vault balance
2. Let `expected` be the computed expected balance
3. Define `excess = vault_actual - expected`

**If `excess > 0`:**
- These tokens arrived without updating any tracking fields
- They represent value that "appeared" in the vault unexpectedly
- SweepExcess adds `excess` to `total_funded_rewards`
- After: `expected' = expected + excess = vault_actual`
- Invariant is restored

**If `excess = 0`:**
- All vault tokens are accounted for
- No action needed

**If `excess < 0`:**
- This would indicate tokens left the vault without program control
- SPL token program prevents unauthorized transfers from vault (requires PDA signature)
- This case is impossible under normal operation
- SweepExcess saturates to 0 as a safety measure

### Security Properties

1. **No theft possible:** SweepExcess adds to `total_funded_rewards`, which only increases the reward pool for all depositors. It cannot extract value.

2. **Permissionless safety:** Anyone can call SweepExcess. The worst case is that excess tokens get distributed as rewards earlier than intended.

3. **Idempotent:** After SweepExcess runs and restores the invariant, calling it again produces `excess = 0` and no state changes occur.

## Summary

| Property | Status |
|----------|--------|
| Invariant holds at initialization | Proven (Base Case) |
| Deposit preserves invariant | Proven (Inductive Step) |
| Withdraw preserves invariant | Proven (Inductive Step) |
| FundRewards preserves invariant | Proven (Inductive Step) |
| Other instructions don't affect invariant | Proven (Completeness) |
| SweepExcess correctly identifies excess | Proven (Corollary) |

**Theorem:** The vault balance invariant is maintained by all program operations, and SweepExcess correctly captures any tokens that bypass program control by adding them to the reward pool.
