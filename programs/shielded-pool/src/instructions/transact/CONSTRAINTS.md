# Execute Transact Constraints

This document enumerates all security constraints enforced by `execute_transact`.
Each constraint is tagged with its enforcement location and error code.

## Quick Reference

| ID | Constraint | Phase | Error |
|----|------------|-------|-------|
| C1 | Groth16 transact proof verifies | P10 | `InvalidProof` |
| C2 | Transact params hash matches | P8 | `TransactParamsHashMismatch` |
| C3 | Commitment root is known | P7 | `UnknownRoot` |
| C4 | Nullifier root is known | P12 | `UnknownNullifierRoot` |
| C5 | Nullifier non-membership proof verifies | P12 | `InvalidNullifierNonMembershipProof` |
| C6 | Nullifier PDAs are uninitialized | E1 | `NullifierAlreadyUsed` |
| C7 | Reward accumulators match on-chain | P6 | `InvalidAssetId` |
| C8 | Public amounts match computation | P11 | `InvalidPublicAmountData` |
| C9 | Fees are sufficient | P11 | `InsufficientFee` |
| C10 | Relayer is authorized | P3/P5 | `Unauthorized`/`InvalidRelayer` |
| C11 | Token accounts are valid | P5/P11 | `InvalidRecipient`/`RecipientMismatch` |
| C12 | Pools are operational | P3/P6/P11 | `PoolPaused` |
| C13 | Transaction not expired | P3 | `TransactionExpired` |
| C14 | Escrow is valid for deposit | E2 | `InvalidEscrowAccount` |

---

## Phase P1: Setup (Account Extraction)

No constraints - zero-cost binding of panchor-validated accounts.

**Implicit constraints (enforced by panchor):**
- `transact_session` owned by shielded-pool program
- `commitment_tree` owned by shielded-pool program
- `receipt_tree` owned by shielded-pool program
- `nullifier_indexed_tree` owned by shielded-pool program
- `global_config` owned by shielded-pool program
- `relayer` is a signer (via `Signer<'info>`)
- `payer` is a signer (via `Signer<'info>`)
- `token_program` matches SPL Token program ID
- `system_program` matches System program ID
- `shielded_pool_program` matches this program ID

---

## Phase P2: Session Parsing

**Location:** `execute_transact.rs:429-438`

| Constraint | Description | Error |
|------------|-------------|-------|
| Session parseable | Session data deserializes correctly | `ProgramError::InvalidAccountData` |

---

## Phase P3: Fail-Fast Checks

**Location:** `execute_transact.rs:440-478`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| P3.1 | `shielded_pool_program.key() == crate::ID` | Program account matches this program | `InvalidProgramAccount` |
| P3.2 | `session_data_len <= MAX_SESSION_DATA_LEN` | Session data within bounds | `ProofPayloadOverflow` |
| **C13** | `clock.slot <= slot_expiry` (if set) | Transaction not expired | `TransactionExpired` |
| **C10.1** | `relayer.key() == params.relayer` | Relayer pubkey matches ZK-bound value | `Unauthorized` |
| **C12.1** | `!global_config.paused()` | Global pool not paused | `PoolPaused` |

---

## Phase P4: Account Loading

**Location:** `execute_transact.rs:480-535`

| Constraint | Description | Error |
|------------|-------------|-------|
| `slot_pool_type[i]` valid | Pool type is 0, 1, or 2 | `InvalidPoolConfig` |
| Hub authority matches | `hub_authority.key() == HUB_AUTHORITY_ADDRESS` | `InvalidHubAuthority` |

### P4.1: Reward Config Loading
**Location:** `accounts.rs:build_reward_config_map()`

For each reward config:
- Pool config owner is shielded-pool program
- Pool type matches expected (Token or UnifiedSol)
- Pool program matches stored config

### P4.2: Slot Account Loading
**Location:** `accounts.rs:load_slot_accounts()`

For each slot:
- Pool config loaded successfully
- Token/UnifiedSol config loaded successfully
- All required accounts present

---

## Phase P5: Relayer Validation

**Location:** `execute_transact.rs:537-576`

**Condition:** Only if `has_relayer()` (any `relayer_fees[i] > 0`)

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C10.2** | `relayer.is_signer()` | Relayer signed the transaction | `SignerNotFound` |
| **C10.3** | `params.relayer != Pubkey::default()` | Relayer is not zero address | `InvalidRelayer` |
| **C11.1** | `relayer_token.owner() == TOKEN_PROGRAM_ID` | Relayer token is SPL token | `InvalidProgramOwner` |
| **C11.2** | `token_account_owner(relayer_token) == relayer.key()` | Relayer token owned by relayer | `InvalidTokenAccountOwner` |
| **C11.3** | `relayer_token == ATA(relayer, mint)` | Relayer token is canonical ATA | `InvalidAssociatedTokenAccount` |

---

## Phase P6: Reward Accumulator Validation

**Location:** `execute_transact.rs:578-604`, `validators.rs`

For each non-zero `proof.reward_asset_id[i]`:

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C7.1** | Reward config exists | `reward_config_map.contains(asset_id)` | `MissingRewardConfig` |
| **C12.2** | Pool is active | `config.is_active != 0` | `PoolPaused` |
| **C7.2** | Accumulator matches | `config.reward_accumulator == proof.reward_acc[i]` | `InvalidAssetId` |

### Token Accumulator (validators.rs:176-231)
- `config_account.owner() == TOKEN_POOL_PROGRAM_ID`
- PDA derivation: `config.key() == find_token_pool_config_pda(mint)`
- Asset ID: `config.asset_id == proof.reward_asset_id[i]`

### Unified SOL Accumulator (validators.rs:242-291)
- `config_account.owner() == UNIFIED_SOL_POOL_PROGRAM_ID`
- Asset ID: `config.asset_id == proof.reward_asset_id[i]`

---

## Phase P7: Commitment Root Validation

**Location:** `execute_transact.rs:606-619`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C3** | `is_known_root(commitment_tree, proof.commitment_root)` | Root in tree's history | `UnknownRoot` |

---

## Phase P8: Transact Params Hash Validation

**Location:** `execute_transact.rs:621-632`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C2** | `SHA256(params) mod Fr == proof.transact_params_hash` | Hash matches ZK public input | `TransactParamsHashMismatch` |

---

## Phase P9: Nullifier PDA Key Validation

**Location:** `execute_transact.rs:634-642`

For each `i in 0..N_INS`:

| Constraint | Description | Error |
|------------|-------------|-------|
| `nullifiers[i].key() == find_nullifier_pda(proof.nullifiers[i])` | PDA key matches | `AccountKeyMismatch` |

---

## Phase P10: Groth16 Proof Verification

**Location:** `execute_transact.rs:644-652`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C1** | `verify_proof(proof, TRANSACT_VK)` | Groth16 proof valid | `InvalidProof` |

**Public inputs verified:**
- `commitmentRoot` - Merkle root of commitment tree
- `transactParamsHash` - Hash of bound parameters
- `publicAssetId[0..2]` - Asset IDs for public slots
- `publicAmount[0..2]` - Pool boundary deltas
- `nullifiers[0..4]` - Nullifier hashes
- `commitments[0..4]` - New commitment hashes
- `rewardAcc[0..8]` - Reward accumulators
- `rewardAssetId[0..8]` - Reward asset IDs

---

## Phase P11: Per-Slot Validation

**Location:** `slot_validation.rs:validate_public_slots()`

### P11.0: Inactive Slot Constraints
**Location:** `slot_validation.rs:204-252`

If `slot_accounts[i].is_none()`:

| Constraint | Description | Error |
|------------|-------------|-------|
| `proof.public_asset_ids[i] == [0; 32]` | No asset ID in proof | `InvalidSlotConfiguration` |
| `proof.public_amounts[i] == [0; 32]` | No amount in proof | `InvalidSlotConfiguration` |
| `params.ext_amounts[i] == 0` | No external amount | `InvalidSlotConfiguration` |
| `params.asset_ids[i] == [0; 32]` | No asset ID in params | `InvalidSlotConfiguration` |
| `params.mints[i] == Pubkey::default()` | No mint | `InvalidSlotConfiguration` |
| `params.fees[i] == 0` | No fee | `InvalidSlotConfiguration` |
| `params.recipients[i] == Pubkey::default()` | No recipient | `InvalidSlotConfiguration` |
| `params.relayer_fees[i] == 0` | No relayer fee | `InvalidSlotConfiguration` |

### P11.1: Active Slot Preconditions
**Location:** `slot_validation.rs:88-113`

| Constraint | Description | Error |
|------------|-------------|-------|
| `proof.public_asset_ids[i] != [0; 32]` | Active slot has asset ID | `InvalidSlotConfiguration` |
| `params.ext_amounts[i] != 0` | Active slot has amount | `InvalidSlotConfiguration` |
| `params.asset_ids[i] == proof.public_asset_ids[i]` | Params match proof | `InvalidAssetId` |

### P11.2: Hub Pool Config Validation
**Location:** `slot_validation.rs:260-305`

| Constraint | Description | Error |
|------------|-------------|-------|
| Hub pool config loadable | Valid owner and discriminator | `InvalidPoolConfig` |
| Pool type matches slot | `hub_config.pool_type == expected` | `InvalidPoolConfig` |
| Pool program matches | `hub_config.pool_program == slot.pool_program.key()` | `InvalidPoolProgram` |

### P11.3: Pool Config PDA Validation
**Location:** `slot_validation.rs:314-382`

**Token Pool:**
- `token_config.key() == find_token_pool_config_pda(mint)`

**Unified SOL Pool:**
- `unified_config.key() == find_unified_sol_pool_config_pda()`
- `lst_config.key() == find_lst_config_pda(lst_mint)`

### P11.4: Relayer Fee Validation (Withdrawals)
**Location:** `fee.rs:60-86`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C9.1** | `relayer_fee <= amount × withdrawal_fee_rate / 10000` | Relayer fee ≤ protocol fee | `RelayerFeeExceedsPoolFee` |

### P11.5: Public Amount Validation
**Location:** `validators.rs`

**Token Pool (validators.rs:110-160):**
| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C12.3** | `pool.is_active()` | Pool is active | `PoolPaused` |
| **C8.1** | `check_public_amount(ext_amount, fee, public_amount)` | Amount matches | `InvalidPublicAmountData` |
| **C9.2** | `validate_fee(...)` | Fee within bounds | `InsufficientFee` |

**Unified SOL Pool (validators.rs:43-97):**
| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C12.4** | `pool.is_active()` | Pool is active | `PoolPaused` |
| **C8.2** | `check_public_amount_unified(ext_amount, fee, public_amount, rates)` | Amount with exchange rate matches | `InvalidPublicAmountData` |
| **C9.3** | `validate_fee_unified(...)` | Fee within bounds | `InsufficientFee` |

### P11.6: Vault Validation
**Location:** `slot_validation.rs:152-169`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C11.4** | `pool.vault_mint() == params.mints[i]` | Mint matches config | `InvalidMint` |
| **C11.5** | `pool.expected_vault_address() == slot.vault.key()` | Vault address matches | `InvalidVault` |
| **C11.6** | `token_account_mint(vault) == mint` | Vault mint correct | `InvalidMint` |

### P11.7: Deposit Validation
**Location:** `slot_validation.rs:172-176`

If `ext_amount > 0`:
| Constraint | Description | Error |
|------------|-------------|-------|
| `pool.validate_deposit(amount, epoch)` | Pool-specific deposit checks | Various |

### P11.8: Token Account Validation
**Location:** `slot_validation.rs:395-450`

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C11.7** | `slot.recipient_token.key() == params.recipients[i]` | Recipient address matches | `RecipientMismatch` |

**For withdrawals (`ext_amount < 0`):**
| Constraint | Description | Error |
|------------|-------------|-------|
| `recipient != Pubkey::default()` (if amount > 0) | Valid recipient | `InvalidRecipient` |
| `recipient_token.owner() == TOKEN_PROGRAM_ID` | Valid SPL token | `InvalidProgramOwner` |
| `token_account_mint(recipient_token) == mint` | Correct mint | `InvalidMint` |

**For relayer fees (`relayer_fee > 0`):**
| Constraint | Description | Error |
|------------|-------------|-------|
| `token_account_mint(relayer_token) == mint` | Correct mint | `InvalidMint` |

---

## Phase P12: Nullifier Non-Membership Validation

**Location:** `nullifier.rs:68-165`

### Root Validation

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C4.1** | `tree.is_current_root(nullifier_root)` OR `epoch_root.root == nullifier_root` | Root is known | `UnknownNullifierRoot` |

**If using historical root (epoch_root_pda):**
| Constraint | Description | Error |
|------------|-------------|-------|
| Valid AccountLoader load | Correct owner/discriminator | `InvalidNullifierEpochRootPda` |
| `epoch_root.nullifier_epoch >= tree.earliest_provable_epoch` | Epoch still valid | `EpochTooOld` |
| `epoch_root_pda.key() == find_nullifier_epoch_root_pda(epoch)` | PDA derivation | `InvalidNullifierEpochRootPda` |

### Proof Verification

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C5** | `verify_groth16(nm_proof, public_inputs, NULLIFIER_NM_VK)` | ZK proof valid | `InvalidNullifierNonMembershipProof` |

**Public inputs:**
- `nullifier_root` - Root of indexed merkle tree
- `nullifiers[0..4]` - Nullifier hashes (must not exist in tree)

---

## Phase E1: Nullifier PDA Creation

**Location:** `nullifier.rs:180-250`

For each `i in 0..N_INS`:

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| `nullifiers[i].key() == find_nullifier_pda(hash)` | PDA derivation | `AccountKeyMismatch` |
| **C6** | `require_uninitialized(nullifier)` | Account not initialized | `NullifierAlreadyUsed` |

**State changes:**
- Creates nullifier PDA
- Sets discriminator
- Stores `pending_index` and `authority`
- Emits `NewNullifierEvent`

---

## Phase E2: Pool CPIs

**Location:** `cpi_execution.rs:46-136`

### Escrow Verification (Deposits)
**Location:** `escrow.rs:37-101`

If `ext_amount > 0`:

| ID | Constraint | Description | Error |
|----|------------|-------------|-------|
| **C14.1** | `escrow.owner() == program_id` | Escrow owned by program | `InvalidEscrowAccount` |
| **C14.2** | `escrow.discriminator == DepositEscrow::DISCRIMINATOR` | Valid discriminator | `InvalidEscrowAccount` |
| **C14.3** | `escrow.proof_hash == SHA256(session_body)` | Proof hash matches | `EscrowProofHashMismatch` |
| **C14.4** | `escrow.is_relayer_authorized(relayer)` | Relayer authorized | `EscrowUnauthorizedRelayer` |
| **C14.5** | `!escrow.is_consumed()` | Not already used | `EscrowAlreadyConsumed` |
| **C14.6** | `!escrow.is_expired(clock.slot)` | Not expired | `EscrowExpired` |

### CPI Execution

**Deposits:** Verified escrow → pool.deposit CPI
**Withdrawals:** hub_authority signs → pool.withdraw CPI

**State changes:**
- Marks escrow as consumed
- Transfers tokens via pool CPI

---

## Phase E3: Commitment Tree Updates

**Location:** `tree_updates.rs:append_commitment()`

For each `i in 0..N_OUTS`:

| Constraint | Description | Error |
|------------|-------------|-------|
| `tree.append(commitment)` succeeds | Tree not full | `MerkleTreeFull` |

**State changes:**
- Appends commitment to merkle tree
- Updates tree root
- Emits `NewCommitmentEvent`

---

## Phase E4: Receipt Tree Update

**Location:** `tree_updates.rs:compute_receipt_and_hash()`, `emit_receipt_event()`

| Constraint | Description | Error |
|------------|-------------|-------|
| `receipt_tree.append(hash)` succeeds | Tree not full | `MerkleTreeFull` |

**State changes:**
- Computes receipt hash
- Appends to receipt tree
- Emits `NewReceiptEvent`

---

## Arithmetic Safety Constraints

All arithmetic operations use checked math:

| Location | Operation | Error |
|----------|-----------|-------|
| `fee.rs:33-40` | Fee calculation | `ArithmeticOverflow` |
| `fee.rs:69-72` | Max relayer fee | `ArithmeticOverflow` |
| `execute_transact.rs:712-716` | Pending index increment | `ArithmeticOverflow` |
| `cpi_execution.rs` | Amount conversions | `ArithmeticOverflow` |
| `slot_validation.rs:127-129` | Absolute amount | `ArithmeticOverflow` |

---

## Constraint Dependencies (DAG)

```
C1 (proof verifies) depends on:
├── C3 (commitment root known) - used as public input
├── C2 (params hash matches) - used as public input
├── C7 (accumulators match) - used as public input
├── C8 (amounts correct) - encoded in public inputs
└── C9 (fees sufficient) - encoded in public inputs

C5 (NM proof verifies) depends on:
├── C4 (nullifier root known) - used as public input
└── C6 (PDAs uninitialized) - complementary check

C14 (escrow valid) depends on:
└── C10 (relayer authorized) - relayer must match escrow.authorized_relayer
```

---

## Attack Prevention Summary

| Attack | Prevented By |
|--------|--------------|
| Double-spend | C5 (NM proof) + C6 (PDA exists) |
| Proof forgery | C1 (Groth16 verification) |
| Stale accumulator | C7 (on-chain accumulator match) |
| Amount manipulation | C8 (public amount validation) |
| Fee underpayment | C9 (fee validation) |
| Unauthorized relayer | C10 (relayer signature + pubkey match) |
| Token account spoofing | C11 (address + mint validation) |
| Pool bypass | C12 (pool active check) |
| Expired transaction | C13 (slot expiry check) |
| Escrow replay | C14 (consumed flag) |
| Escrow hijacking | C14.4 (authorized relayer check) |
