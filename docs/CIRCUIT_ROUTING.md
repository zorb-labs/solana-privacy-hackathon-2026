# Transaction Circuit Routing Architecture

This document diagrams the `transaction4.circom` circuit's value routing system, focusing on yield realization, plausible deniability, and state contention properties.

## Circuit Parameters (transaction-4)

```
nInputNotes  = 4    // Input notes being spent
nOutputNotes = 4    // Output notes being created
nRosterSlots = 4    // Private routing slots
nRewardLines = 8    // Public reward accumulator entries
nPublicLines = 2    // Public deposit/withdrawal lines
```

## Three-Tier Routing Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           PUBLIC TIER (On-Chain State)                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─── Reward Registry (nRewardLines = 8) ────────────────────────────────┐  │
│  │                                                                       │  │
│  │  [0] SOL    acc=1.05e18   ◄── Always stale (finalized ~18 min ago)    │  │
│  │  [1] USDC   acc=1.02e18                                               │  │
│  │  [2] jitoSOL acc=1.08e18                                              │  │
│  │  [3] mSOL   acc=1.06e18                                               │  │
│  │  [4] BONK   acc=1.00e18   ◄── Plausible deniability: 8 entries       │  │
│  │  [5] ...    acc=...           but tx may only use 1-2 assets          │  │
│  │  [6] ...    acc=...                                                   │  │
│  │  [7] ...    acc=...                                                   │  │
│  │                                                                       │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                │                                                            │
│                │ rosterRewardLineSel[j][k]  (one-hot: which registry       │
│                │                             line has this asset?)          │
│                ▼                                                            │
│  ┌─── Public Delta Lines (nPublicLines = 2) ────────────────────────────┐   │
│  │                                                                       │   │
│  │  [0] assetId=SOL  amount=+100e9  enabled=1  (deposit 100 SOL)         │   │
│  │  [1] assetId=0    amount=0       enabled=0  (unused)                  │   │
│  │                                                                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                │                                                             │
│                │ publicLineSlotSel[i][j]  (one-hot: which roster slot?)     │
│                ▼                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        PRIVATE ROUTING TIER (Witnesses)                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─── Roster (nRosterSlots = 4) ────────────────────────────────────────┐   │
│  │                                                                       │   │
│  │  Slot [0]: assetId=SOL,     enabled=1,  globalAcc=1.05e18            │   │
│  │            sumIn=150e9+5e9=155e9  publicDelta=+100e9  sumOut=255e9   │   │
│  │            ───────────────────────────────────────────────────────   │   │
│  │            Conservation: 155e9 + 100e9 = 255e9 ✓                     │   │
│  │                                                                       │   │
│  │  Slot [1]: assetId=USDC,    enabled=1,  globalAcc=1.02e18            │   │
│  │            sumIn=1000e6     publicDelta=0         sumOut=1000e6      │   │
│  │            ───────────────────────────────────────────────────────   │   │
│  │            (private transfer, no public visibility)                  │   │
│  │                                                                       │   │
│  │  Slot [2]: assetId=0,       enabled=0   (unused padding slot)        │   │
│  │  Slot [3]: assetId=0,       enabled=0   (unused padding slot)        │   │
│  │                                                                       │   │
│  └───────────────────────────────────────────────────────────────────────┘   │
│                ▲                           ▲                                │
│                │ inNoteSlotSel[n][j]       │ outNoteSlotSel[n][j]           │
│                │ (one-hot selector)        │ (one-hot selector)             │
│                │                           │                                │
└────────────────┼───────────────────────────┼────────────────────────────────┘
                 │                           │
                 ▼                           ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                              NOTE TIER (Private)                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─── Input Notes (nInputNotes = 4) ────┐  ┌─── Output Notes (4) ────────┐  │
│  │                                      │  │                             │  │
│  │  [0] SOL  amt=100e9  noteAcc=1.00e18 │  │  [0] SOL  amt=200e9         │  │
│  │      yield = 100e9 × 0.05 = 5e9      │  │      outAcc=1.05e18 (snap)  │  │
│  │      value = 100e9 + 5e9 = 105e9     │  │                             │  │
│  │                                      │  │  [1] SOL  amt=55e9          │  │
│  │  [1] SOL  amt=50e9   noteAcc=1.00e18 │  │      outAcc=1.05e18         │  │
│  │      yield = 50e9 × 0.05 = 2.5e9     │  │                             │  │
│  │      value = 50e9 + 2.5e9 ≈ 52.5e9   │  │  [2] USDC amt=1000e6        │  │
│  │                                      │  │      outAcc=1.02e18         │  │
│  │  [2] USDC amt=1000e6 noteAcc=1.00e18 │  │                             │  │
│  │      yield = 1000e6 × 0.02 = 20e6    │  │  [3] (dummy, amt=0)         │  │
│  │      value = 1000e6 + 20e6 = 1020e6  │  │                             │  │
│  │                                      │  │                             │  │
│  │  [3] (dummy note, amt=0)             │  └─────────────────────────────┘  │
│  │                                      │                                   │
│  └──────────────────────────────────────┘                                   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Yield Realization Flow

All input notes realize yield when spent:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         YIELD REALIZATION (per input note)                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Note was created at time T₀ with:                                       │
│     ┌──────────────────────────────────────────────┐                        │
│     │  commitment = Poseidon(                      │                        │
│     │    version=0,                                │                        │
│     │    assetId,                                  │                        │
│     │    amount,          ◄── principal            │                        │
│     │    pk,                                       │                        │
│     │    blinding,                                 │                        │
│     │    rewardAcc=1.00e18  ◄── snapshot at T₀     │                        │
│     │    rho                                       │                        │
│     │  )                                           │                        │
│     └──────────────────────────────────────────────┘                        │
│                                                                             │
│  2. Time passes... pool earns fees, LSTs appreciate, rewards funded         │
│                                                                             │
│  3. At spend time T₁, on-chain state has:                                   │
│     ┌──────────────────────────────────────────────┐                        │
│     │  rewardRegistry[assetId].globalAcc = 1.05e18 │ ◄── 5% yield accrued   │
│     └──────────────────────────────────────────────┘                        │
│                                                                             │
│  4. Circuit computes total value:                                           │
│     ┌──────────────────────────────────────────────────────────────────┐    │
│     │  accDiff = globalAcc - noteAcc = 1.05e18 - 1.00e18 = 0.05e18     │    │
│     │  yield   = amount × accDiff / 1e18 = 100e9 × 0.05 = 5e9          │    │
│     │  value   = amount + yield = 100e9 + 5e9 = 105e9                  │    │
│     └──────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  5. Conservation uses `value` (not `amount`):                               │
│     ┌──────────────────────────────────────────────────────────────────┐    │
│     │  sumIn[slot] + publicDelta[slot] = sumOut[slot]                  │    │
│     │  ─────────────────────────────────────────────────               │    │
│     │  (105e9 + 52.5e9) + 100e9 = 200e9 + 55e9 + 2.5e9                 │    │
│     │  257.5e9 ≈ 257.5e9 ✓                                             │    │
│     └──────────────────────────────────────────────────────────────────┘    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Plausible Deniability via nRewardLines

The circuit accepts 8 reward accumulator entries as public input, but a transaction typically uses only 1-2 assets:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    PLAUSIBLE DENIABILITY MODEL                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PUBLIC INPUTS (visible to everyone):                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  rewardAssetId[8] = [SOL, USDC, jitoSOL, mSOL, BONK, WIF, JUP, RAY] │    │
│  │  rewardAcc[8]     = [1.05e18, 1.02e18, 1.08e18, ...]                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Observer sees: "This transaction COULD involve any of these 8 assets"     │
│                                                                             │
│  PRIVATE WITNESS (hidden):                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  rosterRewardLineSel[0] = [1,0,0,0,0,0,0,0]  ◄── slot 0 uses SOL    │    │
│  │  rosterRewardLineSel[1] = [0,0,0,0,0,0,0,0]  ◄── slot 1 disabled    │    │
│  │  rosterRewardLineSel[2] = [0,0,0,0,0,0,0,0]                         │    │
│  │  rosterRewardLineSel[3] = [0,0,0,0,0,0,0,0]                         │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Reality: Transaction only touches SOL                                      │
│  But: ZK proof hides which of the 8 assets are actually used               │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  ANONYMITY SET:                                                             │
│                                                                             │
│  Without nRewardLines padding:                                              │
│    Verifier sees exactly 1 accumulator → knows exact asset                  │
│                                                                             │
│  With nRewardLines = 8:                                                     │
│    Verifier sees 8 accumulators → 8 possible assets                         │
│    Actual asset hidden in the ZK proof                                      │
│                                                                             │
│  Trade-off: More reward lines = larger anonymity set = more constraints     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## State Contention: Always-Stale Accumulator

The reward accumulator is intentionally stale to enable ZK proof generation:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    STATE CONTENTION MODEL                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  TIMELINE:                                                                  │
│                                                                             │
│  ──────┬──────────────────────┬──────────────────────┬──────────────────►   │
│        │                      │                      │              time    │
│    finalize()             finalize()             finalize()                 │
│    slot=1000              slot=3700              slot=6400                  │
│    acc=1.00e18            acc=1.02e18            acc=1.05e18                │
│                                                                             │
│        └──── 2700 slots ─────┘                                              │
│              (~18 minutes)                                                  │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  WHY STALENESS IS REQUIRED:                                                 │
│                                                                             │
│  1. User reads on-chain state:  acc = 1.02e18                               │
│                                                                             │
│  2. User generates ZK proof (~30-60 seconds):                               │
│     - Circuit input: rewardAcc = 1.02e18                                    │
│     - Computes yield against this value                                     │
│                                                                             │
│  3. User submits transaction (~1-5 seconds)                                 │
│                                                                             │
│  PROBLEM: If accumulator could change during proof generation,              │
│           proof would be invalid when submitted!                            │
│                                                                             │
│  SOLUTION: UPDATE_SLOT_INTERVAL = 2700 (~18 minutes)                        │
│            - Accumulator is "frozen" for at least 18 minutes                │
│            - User has ample time to generate and submit proof               │
│            - Staleness is a feature, not a bug                              │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  CONTENTION ANALYSIS:                                                       │
│                                                                             │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  Epoch N (slots 1000-3699)                                         │     │
│  │  ─────────────────────────────────────────────────                 │     │
│  │  acc = 1.00e18 (frozen)                                            │     │
│  │                                                                    │     │
│  │  Activities during epoch:                                          │     │
│  │  - Deposits: pending_deposits += ...                               │     │
│  │  - Withdrawals: pending_withdrawals += ...                         │     │
│  │  - Fees collected: pending_fees += ...                             │     │
│  │                                                                    │     │
│  │  All proofs in this epoch use acc = 1.00e18                        │     │
│  │  No contention: everyone sees same frozen value                    │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                           │                                                 │
│                           ▼ finalize_rewards()                              │
│  ┌────────────────────────────────────────────────────────────────────┐     │
│  │  Epoch N+1 (slots 3700-6399)                                       │     │
│  │  ─────────────────────────────────────────────────                 │     │
│  │  acc = 1.02e18 (new frozen value)                                  │     │
│  │                                                                    │     │
│  │  Previous pending values distributed:                              │     │
│  │  acc += pending_fees × 1e18 / total_pool                           │     │
│  │                                                                    │     │
│  │  New proofs must use acc = 1.02e18                                 │     │
│  │  In-flight proofs from epoch N are still valid                     │     │
│  │  (verifier checks: noteAcc <= globalAcc, not ==)                   │     │
│  └────────────────────────────────────────────────────────────────────┘     │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  YIELD TRACKING ACROSS EPOCHS:                                              │
│                                                                             │
│  User deposits at epoch N with acc = 1.00e18                                │
│  User withdraws at epoch N+5 with acc = 1.10e18                             │
│                                                                             │
│  Yield = amount × (1.10e18 - 1.00e18) / 1e18 = 10% of principal             │
│                                                                             │
│  The "staleness" within an epoch doesn't affect total yield:                │
│  - User gets same accumulator regardless of when in epoch they withdraw     │
│  - Fairness: all users in same epoch treated equally                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Public Lines: Deposits and Withdrawals

The circuit supports `nPublicLines = 2` public delta lines, each representing a visible token transfer at the pool boundary.

### Sign Convention

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PUBLIC AMOUNT SIGN CONVENTION                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  publicAmount > 0  →  DEPOSIT (tokens flow INTO shielded pool)              │
│  publicAmount < 0  →  WITHDRAWAL (tokens flow OUT OF shielded pool)         │
│  publicAmount = 0  →  Line disabled (no public transfer for this slot)      │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  DEPOSIT FLOW:                                                              │
│  ┌─────────────┐      publicAmount = +100e9      ┌─────────────────────┐    │
│  │ User Wallet │ ─────────────────────────────►  │ Pool Vault (Token)  │    │
│  │   (ATA)     │      SPL Token Transfer         │       (PDA)         │    │
│  └─────────────┘                                 └─────────────────────┘    │
│                                                                             │
│  Circuit sees: publicAmount[i] = +100e9 (positive)                          │
│  Conservation: sumIn + (+100e9) = sumOut                                    │
│  User receives: 100e9 in shielded note (minus fees)                         │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  WITHDRAWAL FLOW:                                                           │
│  ┌─────────────────────┐      publicAmount = -50e9       ┌─────────────┐    │
│  │ Pool Vault (Token)  │ ─────────────────────────────►  │ Recipient   │    │
│  │       (PDA)         │      SPL Token Transfer         │   Wallet    │    │
│  └─────────────────────┘                                 └─────────────┘    │
│                                                                             │
│  Circuit sees: publicAmount[i] = -50e9 (negative, as field element)         │
│  Conservation: sumIn + (-50e9) = sumOut                                     │
│  Note burned: 50e9 from shielded balance                                    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  PRIVATE TRANSFER (no public visibility):                                   │
│  ┌─────────────┐                                 ┌─────────────┐            │
│  │ Input Notes │ ─────── (inside circuit) ─────► │ Output Notes│            │
│  │ (nullified) │                                 │ (committed) │            │
│  └─────────────┘                                 └─────────────┘            │
│                                                                             │
│  Circuit sees: publicAmount[0] = 0, publicAmount[1] = 0                     │
│  Conservation: sumIn + 0 = sumOut (all value stays shielded)                │
│  On-chain: no token transfers, only nullifier/commitment updates            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Field Element Representation of Negative Amounts

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    NEGATIVE AMOUNTS AS FIELD ELEMENTS                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  BN254 Scalar Field:                                                        │
│    p = 21888242871839275222246405745257275088548364400416034343698204186575808495617
│                                                                             │
│  Negative amount representation:                                            │
│    -x  ≡  p - x  (mod p)                                                    │
│                                                                             │
│  Example (withdrawal of 50 SOL = 50e9 lamports):                            │
│    publicAmount = p - 50_000_000_000                                        │
│                 = 21888242871839275222246405745257275088548364400416034343698154186575808495617
│                                                                             │
│  Circuit arithmetic handles this naturally:                                 │
│    sumIn + publicAmount = sumOut                                            │
│    100e9 + (p - 50e9) ≡ 50e9  (mod p)  ✓                                    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  RANGE CHECK INTENTIONALLY OMITTED:                                         │
│                                                                             │
│  The circuit does NOT range-check publicAmount because:                     │
│    1. Withdrawals require ~254-bit field elements (p - small_value)         │
│    2. Value conservation constrains publicAmount implicitly                 │
│    3. On-chain program validates actual token transfer amounts              │
│                                                                             │
│  If prover claims publicAmount = 1000e9 but only transfers 100e9:           │
│    → On-chain validation fails (circuit proof is valid but tx rejected)     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Public Input ↔ Token Transfer Binding

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  ON-CHAIN VALIDATION OF PUBLIC INPUTS                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  The ZK proof guarantees internal consistency but NOT external binding.     │
│  The on-chain program MUST validate:                                        │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  FOR EACH publicLine[i] WHERE enabled[i] == true:                   │    │
│  │                                                                     │    │
│  │  1. ASSET BINDING                                                   │    │
│  │     publicAssetId[i] == Poseidon(token_mint_pubkey)                 │    │
│  │     └── Circuit's assetId matches the SPL token being transferred  │    │
│  │                                                                     │    │
│  │  2. AMOUNT BINDING (Deposits)                                       │    │
│  │     IF publicAmount[i] > 0:                                         │    │
│  │       token_transfer_amount == publicAmount[i]                      │    │
│  │       transfer_direction == User → Vault                            │    │
│  │                                                                     │    │
│  │  3. AMOUNT BINDING (Withdrawals)                                    │    │
│  │     IF publicAmount[i] < 0:  (i.e., > p/2 as field element)         │    │
│  │       token_transfer_amount == |publicAmount[i]| == p - amount      │    │
│  │       transfer_direction == Vault → Recipient                       │    │
│  │                                                                     │    │
│  │  4. ACCOUNT BINDING                                                 │    │
│  │     Depositor/recipient accounts match transactParamsHash           │    │
│  │     └── Proof is bound to specific wallets via hash                 │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  SOLANA PROGRAM PSEUDO-CODE:                                                │
│                                                                             │
│  ```rust                                                                    │
│  fn execute_transact(                                                       │
│      proof: &Groth16Proof,                                                  │
│      public_inputs: &TransactPublicInputs,                                  │
│      token_transfers: &[TokenTransferParams],                               │
│  ) -> Result<()> {                                                          │
│      // 1. Verify ZK proof                                                  │
│      verify_groth16(VERIFYING_KEY, proof, public_inputs)?;                  │
│                                                                             │
│      // 2. Validate each public line against actual transfers              │
│      for (i, transfer) in token_transfers.iter().enumerate() {              │
│          let expected_asset_id = poseidon_hash(&transfer.mint);             │
│          require!(                                                          │
│              public_inputs.public_asset_id[i] == expected_asset_id,         │
│              "Asset ID mismatch"                                            │
│          );                                                                 │
│                                                                             │
│          let public_amount = public_inputs.public_amount[i];                │
│          if is_positive(public_amount) {                                    │
│              // Deposit: transfer FROM depositor TO vault                   │
│              require!(transfer.amount == public_amount);                    │
│              spl_token::transfer(                                           │
│                  from: depositor_ata,                                       │
│                  to: vault_ata,                                             │
│                  amount: transfer.amount,                                   │
│              )?;                                                            │
│          } else {                                                           │
│              // Withdrawal: transfer FROM vault TO recipient                │
│              let withdraw_amount = FIELD_MODULUS - public_amount;           │
│              require!(transfer.amount == withdraw_amount);                  │
│              spl_token::transfer(                                           │
│                  from: vault_ata,                                           │
│                  to: recipient_ata,                                         │
│                  amount: withdraw_amount,                                   │
│                  signer_seeds: vault_pda_seeds,                             │
│              )?;                                                            │
│          }                                                                  │
│      }                                                                      │
│                                                                             │
│      // 3. Record nullifiers (prevent double-spend)                         │
│      for nullifier in &public_inputs.nullifiers {                           │
│          insert_nullifier(nullifier)?;                                      │
│      }                                                                      │
│                                                                             │
│      // 4. Insert commitments into Merkle tree                              │
│      for commitment in &public_inputs.commitments {                         │
│          append_commitment(commitment)?;                                    │
│      }                                                                      │
│                                                                             │
│      Ok(())                                                                 │
│  }                                                                          │
│  ```                                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Multi-Asset Transaction Example

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              EXAMPLE: Deposit SOL + Withdraw USDC (same tx)                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  User wants to:                                                             │
│    - Deposit 10 SOL (shield)                                                │
│    - Withdraw 100 USDC (unshield from existing balance)                     │
│                                                                             │
│  Public Lines:                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  publicLine[0]:                                                     │    │
│  │    assetId = SOL_ASSET_ID                                           │    │
│  │    amount  = +10_000_000_000  (10 SOL in lamports)                  │    │
│  │    enabled = 1                                                      │    │
│  │                                                                     │    │
│  │  publicLine[1]:                                                     │    │
│  │    assetId = USDC_ASSET_ID                                          │    │
│  │    amount  = p - 100_000_000  (−100 USDC, 6 decimals)               │    │
│  │    enabled = 1                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  Roster (private):                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  slot[0]: assetId=SOL,  enabled=1                                   │    │
│  │           sumIn=0  publicDelta=+10e9  sumOut=10e9                   │    │
│  │           (new note created with deposited SOL)                     │    │
│  │                                                                     │    │
│  │  slot[1]: assetId=USDC, enabled=1                                   │    │
│  │           sumIn=100e6  publicDelta=-100e6  sumOut=0                 │    │
│  │           (existing note burned for withdrawal)                     │    │
│  │                                                                     │    │
│  │  slot[2]: disabled                                                  │    │
│  │  slot[3]: disabled                                                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  On-chain token transfers:                                                  │
│    1. SPL Transfer: 10 SOL from user_wsol_ata → vault_wsol_ata              │
│    2. SPL Transfer: 100 USDC from vault_usdc_ata → recipient_usdc_ata       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Roster Slots: Distinct Asset Routing

The roster provides `nRosterSlots = 4` private slots for routing value flows. Each enabled slot represents exactly one distinct asset.

### Uniqueness Invariant

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      ROSTER ASSET UNIQUENESS CONSTRAINT                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  INVARIANT: Among enabled roster slots, no two may share the same assetId. │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  VALID configurations:                                              │    │
│  │                                                                     │    │
│  │  [SOL, USDC, 0, 0]     enabled=[1,1,0,0]  ✓ (2 distinct assets)     │    │
│  │  [SOL, 0, 0, 0]        enabled=[1,0,0,0]  ✓ (1 asset)               │    │
│  │  [SOL, USDC, BONK, 0]  enabled=[1,1,1,0]  ✓ (3 distinct assets)     │    │
│  │  [0, 0, 0, 0]          enabled=[0,0,0,0]  ✓ (pure nullifier reveal) │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  INVALID configurations:                                            │    │
│  │                                                                     │    │
│  │  [SOL, SOL, 0, 0]      enabled=[1,1,0,0]  ✗ DUPLICATE!              │    │
│  │  [USDC, 0, USDC, 0]    enabled=[1,0,1,0]  ✗ DUPLICATE!              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  CIRCUIT ENFORCEMENT (Section 3 of transaction.circom):                     │
│                                                                             │
│  ```circom                                                                  │
│  // For all pairs (a, b) where a < b:                                       │
│  for (var a = 0; a < nRosterSlots - 1; a++) {                               │
│      for (var b = a + 1; b < nRosterSlots; b++) {                           │
│          // Check if assetIds are equal                                     │
│          rosterDupCheck[idx] = IsEqual();                                   │
│          rosterDupCheck[idx].in[0] <== rosterAssetId[a];                    │
│          rosterDupCheck[idx].in[1] <== rosterAssetId[b];                    │
│                                                                             │
│          // Only check if both slots are enabled                            │
│          rosterEnabledProd[idx] <== rosterEnabled[a] * rosterEnabled[b];    │
│                                                                             │
│          // If both enabled AND equal → constraint fails                    │
│          rosterEnabledProd[idx] * rosterDupCheck[idx].out === 0;            │
│      }                                                                      │
│  }                                                                          │
│  ```                                                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Why Uniqueness Matters

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    UNIQUENESS PREVENTS VALUE THEFT                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  WITHOUT uniqueness enforcement, an attacker could:                         │
│                                                                             │
│  Attack Setup:                                                              │
│    roster[0] = { assetId: SOL, enabled: true }                              │
│    roster[1] = { assetId: SOL, enabled: true }  ← DUPLICATE!                │
│                                                                             │
│  Attack Execution:                                                          │
│    1. Input note (100 SOL) routes to slot[0]                                │
│       → sumIn[0] = 100, sumIn[1] = 0                                        │
│                                                                             │
│    2. Output note (100 SOL) routes to slot[1]                               │
│       → sumOut[0] = 0, sumOut[1] = 100                                      │
│                                                                             │
│    3. Conservation check per slot:                                          │
│       slot[0]: 100 + 0 ≠ 0    ← FAILS                                       │
│       slot[1]: 0 + 0 ≠ 100    ← FAILS                                       │
│                                                                             │
│  Wait... this fails! So why do we need uniqueness?                          │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  THE REAL ATTACK (with public delta manipulation):                          │
│                                                                             │
│  Attack Setup:                                                              │
│    roster[0] = { assetId: SOL, enabled: true }                              │
│    roster[1] = { assetId: SOL, enabled: true }  ← DUPLICATE!                │
│    publicLine[0] = { assetId: SOL, amount: +100 }                           │
│    publicLine[1] = { assetId: SOL, amount: -100 }                           │
│                                                                             │
│  Attack Execution:                                                          │
│    1. publicLine[0] (+100) routes to slot[0]                                │
│    2. publicLine[1] (-100) routes to slot[1]                                │
│    3. Output note (100 SOL) routes to slot[0]                               │
│                                                                             │
│  Conservation (appears valid!):                                             │
│    slot[0]: 0 + 100 = 100  ✓                                                │
│    slot[1]: 0 + (-100) = -100... wait, sumOut can't be negative             │
│                                                                             │
│  Actually this still fails. The REAL danger is more subtle:                 │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  THE ACTUAL INVARIANT VIOLATION:                                            │
│                                                                             │
│  With duplicates, the one-hot selector dot-product check becomes            │
│  AMBIGUOUS. The prover could construct a valid-looking proof where:         │
│                                                                             │
│    - Note claims assetId = SOL                                              │
│    - Selector could validly point to slot[0] OR slot[1]                     │
│    - Different notes with same asset route to different slots               │
│    - Per-slot conservation holds but CROSS-SLOT value leaks                 │
│                                                                             │
│  The uniqueness constraint ensures:                                         │
│    For any assetId X, there exists EXACTLY ONE enabled slot with that ID.   │
│    Therefore, all value flows for asset X MUST go through the same slot.    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Roster Slot Value Aggregation

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      VALUE AGGREGATION PER SLOT                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  For each roster slot j, the circuit computes:                              │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  sumIn[j] = Σ (inNoteSlotSel[n][j] × inValue[n])                    │    │
│  │             n                                                       │    │
│  │                                                                     │    │
│  │  where inValue[n] = inNoteAmount[n] + yield[n]                      │    │
│  │        yield[n] = amount × (globalAcc - noteAcc) / 1e18             │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  publicDelta[j] = Σ (publicLineSlotSel[i][j] × publicAmount[i])     │    │
│  │                   i                                                 │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  sumOut[j] = Σ (outNoteSlotSel[n][j] × outNoteAmount[n])            │    │
│  │              n                                                      │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  CONSERVATION EQUATION (enforced per slot):                                 │
│                                                                             │
│       sumIn[j] + publicDelta[j] === sumOut[j]                               │
│                                                                             │
│  This ensures value is conserved for each asset independently.              │
│  Cross-asset transfers are impossible (sumIn for SOL can't feed             │
│  sumOut for USDC because they're in different slots).                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Disabled Slots and Padding

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        DISABLED SLOT HANDLING                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Slots with enabled=0 serve as padding:                                     │
│                                                                             │
│  Constraints on disabled slots:                                             │
│    - assetId must be 0: rosterAssetId[j] × (1 - rosterEnabled[j]) === 0     │
│    - No notes can route to it (OneHotValidator checks enabled flag)         │
│    - No public lines can route to it                                        │
│    - Conservation trivially holds: 0 + 0 = 0                                │
│                                                                             │
│  Transaction complexity is hidden:                                          │
│    - 1-asset tx: [SOL, 0, 0, 0] with enabled=[1,0,0,0]                      │
│    - 4-asset tx: [SOL, USDC, BONK, JUP] with enabled=[1,1,1,1]              │
│    - Both produce proofs with 4 roster slots                                │
│    - Observer can't distinguish (roster is private witness)                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Selector Relationship Diagram

All routing in the circuit is accomplished through one-hot selectors. This diagram shows how each component selects into others.

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                              COMPLETE SELECTOR ARCHITECTURE                                  │
├─────────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────────────────┐    │
│  │                         REWARD REGISTRY (nRewardLines = 8)                          │    │
│  │                              [PUBLIC INPUTS]                                        │    │
│  ├─────────────────────────────────────────────────────────────────────────────────────┤    │
│  │  [0]         [1]         [2]         [3]         [4]         [5]    [6]    [7]      │    │
│  │  SOL         USDC        jitoSOL     mSOL        BONK        WIF    JUP    RAY      │    │
│  │  acc=1.05e18 acc=1.02e18 acc=1.08e18 acc=1.06e18 acc=1.00e18 ...    ...    ...      │    │
│  └──────▲──────────▲──────────▲──────────▲──────────▲───────────────────────────────────┘    │
│         │          │          │          │          │                                       │
│         │ rosterRewardLineSel[j][k] : nRosterSlots × nRewardLines = 4×8 matrix             │
│         │ Constraint: sum(sel[j]) = rosterEnabled[j] (0 or 1 bits set)                     │
│         │ Constraint: dot(sel[j], rewardAssetId) = rosterAssetId[j]                        │
│         │                                                                                   │
│  ┌──────┴──────────┴──────────┴──────────┴──────────┴───────────────────────────────────┐   │
│  │                            ROSTER (nRosterSlots = 4)                                 │   │
│  │                              [PRIVATE WITNESS]                                       │   │
│  ├──────────────────────────────────────────────────────────────────────────────────────┤   │
│  │                                                                                      │   │
│  │    ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐            │   │
│  │    │  SLOT [0]   │   │  SLOT [1]   │   │  SLOT [2]   │   │  SLOT [3]   │            │   │
│  │    │  assetId    │   │  assetId    │   │  assetId    │   │  assetId    │            │   │
│  │    │  enabled    │   │  enabled    │   │  enabled    │   │  enabled    │            │   │
│  │    │  globalAcc  │◄──│  globalAcc  │◄──│  globalAcc  │◄──│  globalAcc  │◄── from    │   │
│  │    │             │   │             │   │             │   │             │   reward   │   │
│  │    │  sumIn ─────┼───┼─────────────┼───┼─────────────┼───┼─────────────┤   registry │   │
│  │    │  +          │   │  sumIn      │   │  sumIn      │   │  sumIn      │            │   │
│  │    │  publicDelta│   │  +          │   │  +          │   │  +          │            │   │
│  │    │  =          │   │  publicDelta│   │  publicDelta│   │  publicDelta│            │   │
│  │    │  sumOut     │   │  =          │   │  =          │   │  =          │            │   │
│  │    │             │   │  sumOut     │   │  sumOut     │   │  sumOut     │            │   │
│  │    └──────┬──────┘   └──────┬──────┘   └──────┬──────┘   └──────┬──────┘            │   │
│  │           │                 │                 │                 │                   │   │
│  └───────────┼─────────────────┼─────────────────┼─────────────────┼───────────────────┘   │
│              │                 │                 │                 │                       │
│     ┌────────┼────────┬────────┼────────┬────────┼────────┬────────┼────────┐              │
│     │        │        │        │        │        │        │        │        │              │
│     │        ▼        │        ▼        │        ▼        │        ▼        │              │
│     │  ┌──────────────┴────────────────┴─────────────────┴──────────────┐   │              │
│     │  │              PUBLIC LINES (nPublicLines = 2)                   │   │              │
│     │  │                    [PUBLIC INPUTS]                             │   │              │
│     │  ├────────────────────────────────────────────────────────────────┤   │              │
│     │  │                                                                │   │              │
│     │  │  ┌────────────────────┐       ┌────────────────────┐           │   │              │
│     │  │  │   PUBLIC LINE [0]  │       │   PUBLIC LINE [1]  │           │   │              │
│     │  │  │   assetId          │       │   assetId          │           │   │              │
│     │  │  │   amount (+/−)     │       │   amount (+/−)     │           │   │              │
│     │  │  │   enabled          │       │   enabled          │           │   │              │
│     │  │  └─────────┬──────────┘       └─────────┬──────────┘           │   │              │
│     │  │            │                            │                      │   │              │
│     │  │            │ publicLineSlotSel[i][j]    │                      │   │              │
│     │  │            │ 2×4 matrix                 │                      │   │              │
│     │  │            │                            │                      │   │              │
│     │  │            └──────────┬─────────────────┘                      │   │              │
│     │  │                       │                                        │   │              │
│     │  │  Constraint: sum(sel[i]) = publicLineEnabled[i]                │   │              │
│     │  │  Constraint: dot(sel[i], rosterAssetId) = publicAssetId[i]     │   │              │
│     │  │                       │                                        │   │              │
│     │  └───────────────────────┼────────────────────────────────────────┘   │              │
│     │                          │                                            │              │
│     │                          ▼                                            │              │
│     │              [Routes to matching roster slot]                         │              │
│     │                                                                       │              │
│     ▼                                                                       ▼              │
│  ┌──────────────────────────────────────┐    ┌──────────────────────────────────────┐     │
│  │      INPUT NOTES (nInputNotes = 4)   │    │     OUTPUT NOTES (nOutputNotes = 4)  │     │
│  │           [PRIVATE WITNESS]          │    │           [PRIVATE WITNESS]          │     │
│  ├──────────────────────────────────────┤    ├──────────────────────────────────────┤     │
│  │                                      │    │                                      │     │
│  │  [0]      [1]      [2]      [3]      │    │  [0]      [1]      [2]      [3]      │     │
│  │  note     note     note     note     │    │  note     note     note     note     │     │
│  │  assetId  assetId  assetId  assetId  │    │  assetId  assetId  assetId  assetId  │     │
│  │  amount   amount   amount   amount   │    │  amount   amount   amount   amount   │     │
│  │  noteAcc  noteAcc  noteAcc  noteAcc  │    │  outAcc   outAcc   outAcc   outAcc   │     │
│  │    │        │        │        │      │    │    │        │        │        │      │     │
│  │    │        │        │        │      │    │    │        │        │        │      │     │
│  │    └────────┴────────┴────────┴──────┼────┼────┴────────┴────────┴────────┘      │     │
│  │                   │                  │    │                   │                  │     │
│  │    inNoteSlotSel[n][j]               │    │    outNoteSlotSel[n][j]              │     │
│  │    4×4 matrix                        │    │    4×4 matrix                        │     │
│  │                                      │    │                                      │     │
│  │    Constraint:                       │    │    Constraint:                       │     │
│  │    sum(sel[n]) = inNoteEnabled[n]    │    │    sum(sel[n]) = outNoteEnabled[n]   │     │
│  │    (1 if amount≠0, else 0)           │    │    (1 if amount≠0, else 0)           │     │
│  │                                      │    │                                      │     │
│  │    Constraint:                       │    │    Constraint:                       │     │
│  │    dot(sel[n], rosterAssetId)        │    │    dot(sel[n], rosterAssetId)        │     │
│  │    = inNoteAssetId[n]                │    │    = outNoteAssetId[n]               │     │
│  │                                      │    │                                      │     │
│  └──────────────────────────────────────┘    └──────────────────────────────────────┘     │
│                                                                                           │
└───────────────────────────────────────────────────────────────────────────────────────────┘
```

### Selector Matrix Dimensions

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         SELECTOR MATRIX SUMMARY                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SELECTOR                    DIMENSIONS    SOURCE → TARGET                  │
│  ─────────────────────────────────────────────────────────────────────────  │
│                                                                             │
│  rosterRewardLineSel[j][k]   4 × 8         Roster slot → Reward line        │
│  publicLineSlotSel[i][j]     2 × 4         Public line → Roster slot        │
│  inNoteSlotSel[n][j]         4 × 4         Input note → Roster slot         │
│  outNoteSlotSel[n][j]        4 × 4         Output note → Roster slot        │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  TOTAL SELECTOR SIGNALS: 4×8 + 2×4 + 4×4 + 4×4 = 32 + 8 + 16 + 16 = 72     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### One-Hot Validator Constraints

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      OneHotValidator TEMPLATE                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  template OneHotValidator(n) {                                              │
│      signal input bits[n];       // The selector being validated            │
│      signal input enabled[n];    // Which target slots are valid            │
│      signal input values[n];     // Values at each target (e.g., assetId)   │
│      signal input expectedSum;   // How many bits should be set (0 or 1)    │
│      signal input expectedDot;   // What dot product should equal           │
│                                                                             │
│      // CONSTRAINT 1: Each bit is binary                                    │
│      for (i = 0; i < n; i++) {                                              │
│          bits[i] * (1 - bits[i]) === 0;                                     │
│      }                                                                      │
│                                                                             │
│      // CONSTRAINT 2: Can only select enabled targets                       │
│      for (i = 0; i < n; i++) {                                              │
│          bits[i] * (1 - enabled[i]) === 0;                                  │
│      }                                                                      │
│                                                                             │
│      // CONSTRAINT 3: Sum of bits equals expected (0 or 1)                  │
│      signal sum <== Σ bits[i];                                              │
│      sum === expectedSum;                                                   │
│                                                                             │
│      // CONSTRAINT 4: Dot product equals expected value                     │
│      signal dot <== Σ (bits[i] * values[i]);                                │
│      dot === expectedDot;                                                   │
│  }                                                                          │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  EXAMPLE: Input note with assetId=USDC selecting roster slot                │
│                                                                             │
│  rosterAssetId = [SOL, USDC, 0, 0]                                          │
│  rosterEnabled = [1, 1, 0, 0]                                               │
│  inNoteAssetId[n] = USDC                                                    │
│  inNoteEnabled[n] = 1 (amount ≠ 0)                                          │
│                                                                             │
│  Valid selector: inNoteSlotSel[n] = [0, 1, 0, 0]                            │
│                                                                             │
│  Checks:                                                                    │
│    ✓ All bits binary: 0,1,0,0                                               │
│    ✓ Only selects enabled slot: bit[1]=1, enabled[1]=1                      │
│    ✓ Sum = 1 = expectedSum (note is enabled)                                │
│    ✓ Dot = 0×SOL + 1×USDC + 0×0 + 0×0 = USDC = expectedDot                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Flow Through Selectors

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        VALUE EXTRACTION VIA SELECTORS                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. ROSTER GETS GLOBAL ACCUMULATOR FROM REWARD REGISTRY                     │
│  ──────────────────────────────────────────────────────────────────────     │
│                                                                             │
│  rosterGlobalAcc[j] = Σ (rosterRewardLineSel[j][k] × rewardAcc[k])          │
│                       k                                                     │
│                                                                             │
│  Example: roster slot 0 has assetId=SOL, selects rewardLine[0]              │
│    rosterRewardLineSel[0] = [1,0,0,0,0,0,0,0]                               │
│    rosterGlobalAcc[0] = 1×1.05e18 + 0×... = 1.05e18                         │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  2. INPUT NOTES GET GLOBAL ACCUMULATOR FROM ROSTER                          │
│  ──────────────────────────────────────────────────────────────────────     │
│                                                                             │
│  inSelectedGlobalAcc[n] = Σ (inNoteSlotSel[n][j] × rosterGlobalAcc[j])      │
│                           j                                                 │
│                                                                             │
│  This value is used to compute yield:                                       │
│    yield = amount × (inSelectedGlobalAcc - inNoteRewardAcc) / 1e18          │
│    inValue = amount + yield                                                 │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  3. OUTPUT NOTES SNAPSHOT GLOBAL ACCUMULATOR                                │
│  ──────────────────────────────────────────────────────────────────────     │
│                                                                             │
│  outSelectedGlobalAcc[n] = Σ (outNoteSlotSel[n][j] × rosterGlobalAcc[j])    │
│                            j                                                │
│                                                                             │
│  Constraint: outNoteRewardAcc[n] === outSelectedGlobalAcc[n]                │
│  (Output notes must record current accumulator for future yield calc)       │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  4. VALUE AGGREGATION PER ROSTER SLOT                                       │
│  ──────────────────────────────────────────────────────────────────────     │
│                                                                             │
│  sumInBySlot[j] = Σ (inNoteSlotSel[n][j] × inValue[n])                      │
│                   n                                                         │
│                                                                             │
│  publicBySlot[j] = Σ (publicLineSlotSel[i][j] × publicAmount[i])            │
│                    i                                                        │
│                                                                             │
│  sumOutBySlot[j] = Σ (outNoteSlotSel[n][j] × outNoteAmount[n])              │
│                    n                                                        │
│                                                                             │
│  CONSERVATION: sumInBySlot[j] + publicBySlot[j] === sumOutBySlot[j]         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Complete Constraint Summary

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       ALL CIRCUIT CONSTRAINTS                               │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SECTION 1: Version (prevents cross-circuit note spending)                  │
│    inNoteVersion[n] === 0                    ∀n ∈ [0, nInputNotes)          │
│    outNoteVersion[n] === 0                   ∀n ∈ [0, nOutputNotes)         │
│                                                                             │
│  SECTION 2: Boolean canonicalization                                        │
│    rosterEnabled[j] ∈ {0, 1}                 ∀j ∈ [0, nRosterSlots)         │
│    publicLineEnabled[i] ∈ {0, 1}             ∀i ∈ [0, nPublicLines)         │
│    rosterAssetId[j] × (1 - rosterEnabled[j]) === 0                          │
│    publicAssetId[i] × (1 - publicLineEnabled[i]) === 0                      │
│    publicAmount[i] × (1 - publicLineEnabled[i]) === 0                       │
│                                                                             │
│  SECTION 3: Roster asset uniqueness                                         │
│    ∀(a,b) where a<b: rosterEnabled[a] × rosterEnabled[b] ×                  │
│                      IsEqual(rosterAssetId[a], rosterAssetId[b]) === 0      │
│                                                                             │
│  SECTION 4: Roster → Reward line selection                                  │
│    OneHotValidator(rosterRewardLineSel[j])   ∀j ∈ [0, nRosterSlots)         │
│      .expectedSum = rosterEnabled[j]                                        │
│      .expectedDot = rosterAssetId[j]                                        │
│                                                                             │
│  SECTION 5: Public line → Roster selection                                  │
│    OneHotValidator(publicLineSlotSel[i])     ∀i ∈ [0, nPublicLines)         │
│      .expectedSum = publicLineEnabled[i]                                    │
│      .expectedDot = publicAssetId[i]                                        │
│                                                                             │
│  SECTION 6: Note → Roster selection + accumulator bounds                    │
│    OneHotValidator(inNoteSlotSel[n])         ∀n ∈ [0, nInputNotes)          │
│      .expectedSum = (inNoteAmount[n] ≠ 0)                                   │
│      .expectedDot = inNoteAssetId[n]                                        │
│    inNoteRewardAcc[n] ≤ inSelectedGlobalAcc[n] (if enabled)                 │
│                                                                             │
│    OneHotValidator(outNoteSlotSel[n])        ∀n ∈ [0, nOutputNotes)         │
│      .expectedSum = (outNoteAmount[n] ≠ 0)                                  │
│      .expectedDot = outNoteAssetId[n]                                       │
│    outNoteRewardAcc[n] === outSelectedGlobalAcc[n] (if enabled)             │
│                                                                             │
│  SECTION 7: Input note cryptographic validity                               │
│    commitment = NoteCommitment(version, assetId, amount, pk, ...)           │
│    nullifier = ComputeNullifier(nk, rho, commitment)                        │
│    MerkleProof(commitment, path) → root === commitmentRoot (if enabled)     │
│    inValue = amount + ComputeReward(amount, globalAcc, noteAcc)             │
│                                                                             │
│  SECTION 8: Output note validity                                            │
│    commitment = NoteCommitment(version, assetId, amount, pk, ...)           │
│    commitment === commitments[n] (public input)                             │
│    outNoteRho[n] === nullifiers[n] (if enabled and n < nInputNotes)         │
│                                                                             │
│  SECTION 9: Nullifier uniqueness (within tx)                                │
│    nullifiers[a] ≠ nullifiers[b]             ∀(a,b) where a<b               │
│                                                                             │
│  SECTION 10: Value conservation (per slot)                                  │
│    sumInBySlot[j] + publicBySlot[j] === sumOutBySlot[j]                     │
│                                              ∀j ∈ [0, nRosterSlots)         │
│                                                                             │
│  SECTION 11: Public input range binding                                     │
│    transactParamsHash fits in 254 bits                                      │
│    publicAssetId[i] fits in 254 bits                                        │
│    rewardAssetId[k] fits in 254 bits                                        │
│    rewardAcc[k] fits in 248 bits                                            │
│    nullifiers[n] fits in 254 bits                                           │
│    commitments[n] fits in 254 bits                                          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Sample Pool Configs Algorithm

The client must construct `rewardAssetId[8]` and `rewardAcc[8]` public inputs that include all assets used in the transaction, padded with decoy assets for plausible deniability.

```typescript
/**
 * Sample pool configs for nRewardLines circuit input.
 *
 * @param requiredAssetIds - Asset IDs the transaction actually uses (from notes)
 * @param allPoolConfigs   - All available on-chain pool configs
 * @param nRewardLines     - Circuit parameter (8 for transaction4)
 * @returns Array of (assetId, rewardAccumulator) tuples for circuit input
 */
function samplePoolConfigs(
  requiredAssetIds: Set<bigint>,
  allPoolConfigs: PoolConfig[],
  nRewardLines: number = 8
): { assetId: bigint; rewardAcc: bigint }[] {

  const result: { assetId: bigint; rewardAcc: bigint }[] = [];
  const usedAssetIds = new Set<bigint>();

  // ─────────────────────────────────────────────────────────────────────────
  // STEP 1: Include all required assets (MUST be present for proof validity)
  // ─────────────────────────────────────────────────────────────────────────
  for (const requiredId of requiredAssetIds) {
    const config = allPoolConfigs.find(c => c.assetId === requiredId);

    if (!config) {
      throw new Error(`Required asset ${requiredId} not found in pool configs`);
    }

    result.push({
      assetId: config.assetId,
      rewardAcc: config.rewardAccumulator,  // Current on-chain value
    });
    usedAssetIds.add(requiredId);
  }

  if (result.length > nRewardLines) {
    throw new Error(
      `Transaction uses ${result.length} assets but circuit only supports ${nRewardLines}`
    );
  }

  // ─────────────────────────────────────────────────────────────────────────
  // STEP 2: Fill remaining slots with decoy assets (plausible deniability)
  // ─────────────────────────────────────────────────────────────────────────
  const availableDecoys = allPoolConfigs.filter(
    c => !usedAssetIds.has(c.assetId) && c.isActive
  );

  // Shuffle for randomness (prevents pattern analysis)
  shuffleArray(availableDecoys);

  let decoyIndex = 0;
  while (result.length < nRewardLines && decoyIndex < availableDecoys.length) {
    const decoy = availableDecoys[decoyIndex++];
    result.push({
      assetId: decoy.assetId,
      rewardAcc: decoy.rewardAccumulator,
    });
    usedAssetIds.add(decoy.assetId);
  }

  // ─────────────────────────────────────────────────────────────────────────
  // STEP 3: Pad with zero entries if not enough pool configs exist
  // ─────────────────────────────────────────────────────────────────────────
  while (result.length < nRewardLines) {
    // Zero entries are valid but reduce anonymity set
    // Circuit handles assetId=0 as "no asset" (won't be selected by roster)
    result.push({
      assetId: 0n,
      rewardAcc: 0n,
    });
  }

  // ─────────────────────────────────────────────────────────────────────────
  // STEP 4: Shuffle final array to hide which entries are "real"
  // ─────────────────────────────────────────────────────────────────────────
  shuffleArray(result);

  return result;
}

/**
 * Build the roster-to-reward-line selectors based on sampled configs.
 *
 * @param rosterAssetIds    - Asset IDs in each roster slot (private)
 * @param rewardLineConfigs - Sampled pool configs (public)
 * @returns One-hot selector matrix [nRosterSlots][nRewardLines]
 */
function buildRosterRewardLineSelectors(
  rosterAssetIds: bigint[],          // e.g., [SOL, USDC, 0, 0] for 4 slots
  rosterEnabled: boolean[],           // e.g., [true, true, false, false]
  rewardLineConfigs: { assetId: bigint; rewardAcc: bigint }[]
): number[][] {

  const nRosterSlots = rosterAssetIds.length;
  const nRewardLines = rewardLineConfigs.length;

  const selectors: number[][] = [];

  for (let j = 0; j < nRosterSlots; j++) {
    const selector = new Array(nRewardLines).fill(0);

    if (rosterEnabled[j]) {
      // Find the reward line with matching assetId
      const matchIndex = rewardLineConfigs.findIndex(
        c => c.assetId === rosterAssetIds[j]
      );

      if (matchIndex === -1) {
        throw new Error(
          `Roster slot ${j} has assetId ${rosterAssetIds[j]} ` +
          `but no matching reward line was sampled`
        );
      }

      selector[matchIndex] = 1;  // One-hot: exactly one bit set
    }
    // If disabled, selector remains all zeros (valid per OneHotValidator)

    selectors.push(selector);
  }

  return selectors;
}
```

### Usage Example

```typescript
// User wants to: deposit 100 SOL, transfer 500 USDC privately
const requiredAssets = new Set([
  SOL_ASSET_ID,   // For the public deposit
  USDC_ASSET_ID,  // For the private transfer
]);

// Fetch all active pool configs from on-chain
const allPools = await fetchPoolConfigs(connection);
// e.g., [SOL, USDC, jitoSOL, mSOL, BONK, WIF, JUP, RAY, ...]

// Sample 8 configs for circuit input
const rewardLineConfigs = samplePoolConfigs(requiredAssets, allPools, 8);
// Result (shuffled):
// [
//   { assetId: BONK,    rewardAcc: 1.00e18 },  // decoy
//   { assetId: SOL,     rewardAcc: 1.05e18 },  // REQUIRED
//   { assetId: JUP,     rewardAcc: 1.01e18 },  // decoy
//   { assetId: USDC,    rewardAcc: 1.02e18 },  // REQUIRED
//   { assetId: mSOL,    rewardAcc: 1.08e18 },  // decoy
//   { assetId: jitoSOL, rewardAcc: 1.06e18 },  // decoy
//   { assetId: WIF,     rewardAcc: 1.00e18 },  // decoy
//   { assetId: RAY,     rewardAcc: 1.03e18 },  // decoy
// ]

// Build circuit public inputs
const publicInputs = {
  rewardAssetId: rewardLineConfigs.map(c => c.assetId),
  rewardAcc: rewardLineConfigs.map(c => c.rewardAcc),
  // ... other inputs
};

// Build private roster (only 2 slots enabled for this tx)
const rosterAssetIds = [SOL_ASSET_ID, USDC_ASSET_ID, 0n, 0n];
const rosterEnabled = [true, true, false, false];

// Build one-hot selectors linking roster slots to reward lines
const rosterRewardLineSel = buildRosterRewardLineSelectors(
  rosterAssetIds,
  rosterEnabled,
  rewardLineConfigs
);
// Result:
// [
//   [0,1,0,0,0,0,0,0],  // Slot 0 (SOL) → rewardLine[1]
//   [0,0,0,1,0,0,0,0],  // Slot 1 (USDC) → rewardLine[3]
//   [0,0,0,0,0,0,0,0],  // Slot 2 (disabled)
//   [0,0,0,0,0,0,0,0],  // Slot 3 (disabled)
// ]
```

### Anonymity Analysis

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ANONYMITY SET CALCULATION                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Given:                                                                     │
│    - Transaction actually uses: {SOL, USDC}                                 │
│    - Circuit public input: 8 reward line entries                            │
│    - Observer sees: [BONK, SOL, JUP, USDC, mSOL, jitoSOL, WIF, RAY]         │
│                                                                             │
│  Observer's uncertainty:                                                    │
│    - Could be any subset of the 8 assets                                    │
│    - Possible subsets: C(8,1) + C(8,2) + C(8,3) + C(8,4) = 162 combinations │
│    - True subset {SOL, USDC} is one of 28 possible 2-asset combinations     │
│                                                                             │
│  Enhancement strategies:                                                    │
│    1. Always include popular assets (SOL, USDC) as decoys                   │
│    2. Rotate decoy selection across transactions                            │
│    3. Match decoy distribution to network-wide usage patterns               │
│                                                                             │
│  Weakness:                                                                  │
│    - If only 3 pools exist on-chain, anonymity set = 3                      │
│    - Zero-padded entries (assetId=0) are distinguishable                    │
│    - Repeated decoy patterns across txs can leak information                │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Unified SOL: Appreciating Accumulator (Always Stale)

The Unified SOL Pool has a unique reward model: LST appreciation feeds directly into the reward accumulator.

### LST Appreciation as Yield Source

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    UNIFIED SOL APPRECIATION MODEL                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Unlike Token Pool (fees only), Unified SOL captures LST staking yield:     │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  YIELD SOURCES:                                                     │    │
│  │                                                                     │    │
│  │  1. Transaction Fees                                                │    │
│  │     └── pending_deposit_fees + pending_withdrawal_fees              │    │
│  │                                                                     │    │
│  │  2. LST Appreciation (staking rewards)                              │    │
│  │     └── pending_appreciation                                        │    │
│  │                                                                     │    │
│  │  Both feed into reward_accumulator at finalization:                 │    │
│  │     total_pending = fees + appreciation                             │    │
│  │     acc += total_pending × 1e18 / total_virtual_sol                 │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  HOW LST APPRECIATION IS CAPTURED:                                          │
│                                                                             │
│  Step 1: Stake pools earn rewards (each Solana epoch ~2.5 days)             │
│    jitoSOL stake pool: total_lamports increases                             │
│    Exchange rate: rate = total_lamports / pool_token_supply                 │
│                                                                             │
│  Step 2: harvest_lst_appreciation reads new rate                            │
│    old_rate = 1.050e9  (stored in LstConfig)                                │
│    new_rate = 1.055e9  (read from stake pool account)                       │
│                                                                             │
│  Step 3: Calculate appreciation in virtual SOL                              │
│    vault_balance = 1000 jitoSOL tokens                                      │
│    old_virtual = 1000 × 1.050 = 1050 virtual SOL                            │
│    new_virtual = 1000 × 1.055 = 1055 virtual SOL                            │
│    appreciation = 1055 - 1050 = 5 virtual SOL                               │
│                                                                             │
│  Step 4: Add to pending appreciation                                        │
│    pending_appreciation += 5e9 (5 SOL in lamports)                          │
│                                                                             │
│  Step 5: At finalize_unified_rewards                                        │
│    total_pending = fees + 5e9                                               │
│    reward_accumulator += total_pending × 1e18 / total_virtual_sol           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Always-Stale Public Accumulator

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 WHY THE ACCUMULATOR IS ALWAYS STALE                         │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  TIMELINE OF STALENESS:                                                     │
│                                                                             │
│  Solana    │ Epoch N              │ Epoch N+1            │ Epoch N+2       │
│  Epoch     │ (~2.5 days)          │ (~2.5 days)          │                 │
│  ──────────┼──────────────────────┼──────────────────────┼─────────────────│
│            │                      │                      │                 │
│  LST Rate  │ 1.050                │ 1.055                │ 1.060           │
│  (actual)  │   │                  │   │                  │   │             │
│            │   ▼ harvest()        │   ▼ harvest()        │   ▼             │
│            │                      │                      │                 │
│  Harvested │ 1.045 ────────────── │ 1.050 ────────────── │ 1.055 ───────   │
│  Rate      │ (frozen last epoch)  │ (frozen last epoch)  │                 │
│            │                      │                      │                 │
│  Reward    │ 1.00e18 ──────────── │ 1.02e18 ──────────── │ 1.05e18 ────    │
│  Accum     │ (frozen at finalize) │ (frozen at finalize) │                 │
│            │                      │                      │                 │
│  ──────────┼──────────────────────┼──────────────────────┼─────────────────│
│            │                      │                      │                 │
│  User      │ User reads acc=1.02e18 and generates proof                    │
│  Activity  │ Proof is valid until next finalize (~18 min minimum)          │
│            │                      │                      │                 │
└─────────────────────────────────────────────────────────────────────────────┘

  KEY INSIGHT: The accumulator is INTENTIONALLY behind reality.

  Current actual value:     yield earned but not yet finalized
  On-chain accumulator:     frozen value from last finalize_unified_rewards
  Gap:                      ~18 minutes to ~2.5 days of unrealized appreciation

  This gap is a FEATURE:
    1. Provides stable target for ZK proof generation
    2. Prevents front-running (can't claim yield not yet finalized)
    3. Batches appreciation across all depositors fairly
```

### Harvest → Finalize Epoch Cycle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    UNIFIED SOL EPOCH LIFECYCLE                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  reward_epoch = 1                                                           │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  PHASE 1: HARVEST (permissionless, once per LST)                    │    │
│  │  ─────────────────────────────────────────────────────────────────  │    │
│  │                                                                     │    │
│  │  harvest_lst_appreciation(jitoSOL):                                 │    │
│  │    1. Read stake pool exchange rate                                 │    │
│  │    2. Validate: rate updated in current Solana epoch                │    │
│  │    3. Validate: rate change ≤ MAX_RATE_CHANGE_BPS (0.5%)            │    │
│  │    4. Calculate appreciation since last harvest                     │    │
│  │    5. pending_appreciation += appreciation                          │    │
│  │    6. exchange_rate = new_rate                                      │    │
│  │    7. last_harvest_epoch = 1  ◄── marks as harvested                │    │
│  │                                                                     │    │
│  │  harvest_lst_appreciation(mSOL):                                    │    │
│  │    (same process for each registered LST)                           │    │
│  │    last_harvest_epoch = 1                                           │    │
│  │                                                                     │    │
│  │  harvest_lst_appreciation(WSOL):                                    │    │
│  │    (WSOL rate is always 1:1, no appreciation)                       │    │
│  │    last_harvest_epoch = 1                                           │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                              │
│                              ▼                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                     │    │
│  │  PHASE 2: FINALIZE (permissionless, once per epoch)                 │    │
│  │  ─────────────────────────────────────────────────────────────────  │    │
│  │                                                                     │    │
│  │  finalize_unified_rewards([all LST configs]):                       │    │
│  │                                                                     │    │
│  │  PRECONDITIONS:                                                     │    │
│  │    ✓ UPDATE_SLOT_INTERVAL elapsed (2700 slots, ~18 min)             │    │
│  │    ✓ ALL active LSTs have last_harvest_epoch == reward_epoch        │    │
│  │                                                                     │    │
│  │  ACTIONS:                                                           │    │
│  │    1. total_pending = pending_fees + pending_appreciation           │    │
│  │    2. For each LST:                                                 │    │
│  │         total_virtual_sol += vault_balance × exchange_rate          │    │
│  │         harvested_exchange_rate = exchange_rate  ◄── FREEZE         │    │
│  │    3. reward_accumulator += total_pending × 1e18 / total_virtual_sol│    │
│  │    4. pending_fees = 0                                              │    │
│  │    5. pending_appreciation = 0                                      │    │
│  │    6. reward_epoch += 1                                             │    │
│  │    7. last_finalized_slot = current_slot                            │    │
│  │                                                                     │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                              │                                              │
│                              ▼                                              │
│  reward_epoch = 2 (new epoch begins)                                        │
│                                                                             │
│  PUBLIC STATE NOW FROZEN:                                                   │
│    - reward_accumulator = 1.05e18 (for ZK proofs)                           │
│    - harvested_exchange_rate per LST (for deposit/withdraw conversions)    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Public Accumulator in Circuit

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              ACCUMULATOR USAGE IN TRANSACTION CIRCUIT                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  PUBLIC INPUT (from on-chain UnifiedSolPoolConfig):                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  rewardAssetId[k] = UNIFIED_SOL_ASSET_ID  (for some k in 0..7)      │    │
│  │  rewardAcc[k]     = 1.05e18  (frozen at last finalize)              │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  INPUT NOTE (being spent):                                                  │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  inNoteAssetId    = UNIFIED_SOL_ASSET_ID                            │    │
│  │  inNoteAmount     = 100e9  (100 virtual SOL)                        │    │
│  │  inNoteRewardAcc  = 1.00e18  (snapshot when note was created)       │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  YIELD CALCULATION:                                                         │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  globalAcc = 1.05e18 (from public input, always stale)              │    │
│  │  noteAcc   = 1.00e18 (from note, when deposited)                    │    │
│  │  accDiff   = 1.05e18 - 1.00e18 = 0.05e18                            │    │
│  │                                                                     │    │
│  │  yield = 100e9 × 0.05e18 / 1e18 = 5e9 (5 virtual SOL)               │    │
│  │  inValue = 100e9 + 5e9 = 105e9                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  CONSTRAINT: inNoteRewardAcc ≤ globalAcc                                    │
│    - Can't claim yield from the future                                      │
│    - Older notes have more yield (larger accDiff)                           │
│    - Notes created in current epoch have noteAcc ≈ globalAcc (minimal yield)│
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  OUTPUT NOTE (being created):                                               │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  outNoteAssetId   = UNIFIED_SOL_ASSET_ID                            │    │
│  │  outNoteAmount    = 105e9  (principal + realized yield)             │    │
│  │  outNoteRewardAcc = 1.05e18  (MUST equal current globalAcc)         │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  CONSTRAINT: outNoteRewardAcc === globalAcc                                 │
│    - Snapshots current accumulator for future yield calculation             │
│    - When this note is later spent, yield = newGlobalAcc - 1.05e18          │
│    - Prevents "backdating" notes to claim extra yield                       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Appreciation vs Fee Rewards

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                TOKEN POOL vs UNIFIED SOL POOL COMPARISON                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│                      TOKEN POOL              UNIFIED SOL POOL               │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  Yield Sources:       Fees only              Fees + LST Appreciation        │
│                                                                             │
│  pending_rewards:     deposit_fees           deposit_fees                   │
│                       + withdrawal_fees      + withdrawal_fees              │
│                       + funded_rewards       + pending_appreciation         │
│                                                                             │
│  Appreciation:        N/A (1:1 tokens)       LST exchange rate increase     │
│                                                                             │
│  Harvest step:        Not needed             Required before finalize       │
│                                                                             │
│  Exchange rate:       1:1 always             Variable per LST               │
│                                                                             │
│  Virtual units:       Token base units       Virtual SOL (normalized)       │
│                                                                             │
│  ════════════════════════════════════════════════════════════════════════   │
│                                                                             │
│  APPRECIATION MAGNITUDE (example):                                          │
│                                                                             │
│  jitoSOL typical APY: ~7%                                                   │
│  Per epoch (~2.5 days): 7% / 146 epochs ≈ 0.048%                            │
│  Per finalization (~18 min): 0.048% / 200 ≈ 0.00024%                        │
│                                                                             │
│  For 1,000,000 virtual SOL pool:                                            │
│    Per-finalization appreciation ≈ 2.4 SOL                                  │
│    → Distributed across all depositors via accumulator                      │
│                                                                             │
│  This appreciation would otherwise be captured by the protocol.             │
│  Instead, it flows to depositors proportionally.                            │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Compute Unit Benchmarks

Proof verification on Solana is compute-intensive. These benchmarks show CU usage for different verification strategies.

### Groth16 BN254 Verification

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    GROTH16 VERIFICATION CU BENCHMARKS                       │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  SINGLE GROTH16 PROOF (transaction4 circuit):                               │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  │ Operation                          │ CU Cost    │ % of Limit │           │
│  ├────────────────────────────────────┼────────────┼────────────┤           │
│  │ Proof deserialization              │    ~5,000  │     0.4%   │           │
│  │ Public input preparation (26 inputs)│   ~10,000 │     0.8%   │           │
│  │ alt_bn128 pairing (3 pairings)     │  ~190,000  │    15.8%   │           │
│  │ alt_bn128 G1/G2 mul (MSM)          │   ~70,000  │     5.8%   │           │
│  │ Final verification                 │    ~5,000  │     0.4%   │           │
│  ├────────────────────────────────────┼────────────┼────────────┤           │
│  │ TOTAL GROTH16 VERIFY               │  ~280,000  │    23.3%   │           │
│  └────────────────────────────────────┴────────────┴────────────┘           │
│                                                                             │
│  Post-verification operations:                                              │
│  │ Nullifier insertion (4 nullifiers) │   ~40,000  │     3.3%   │           │
│  │ Commitment insertion (4 commits)   │   ~60,000  │     5.0%   │           │
│  │ Token transfers (CPI to SPL)       │   ~50,000  │     4.2%   │           │
│  │ State updates                      │   ~30,000  │     2.5%   │           │
│  ├────────────────────────────────────┼────────────┼────────────┤           │
│  │ TOTAL SINGLE TX                    │  ~460,000  │    38.3%   │           │
│  └────────────────────────────────────┴────────────┴────────────┘           │
│                                                                             │
│  Solana CU limit: 1,200,000 per transaction                                 │
│  Headroom: ~740,000 CU (61.7%)                                              │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  TWO GROTH16 PROOFS (main tx + nullifier batch insert):                     │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  │ Operation                          │ CU Cost    │ % of Limit │           │
│  ├────────────────────────────────────┼────────────┼────────────┤           │
│  │ Transaction proof (transaction4)   │  ~280,000  │    23.3%   │           │
│  │ Nullifier batch proof (batch4)     │  ~200,000  │    16.7%   │           │
│  │ Indexed Merkle tree updates        │   ~80,000  │     6.7%   │           │
│  │ Token transfers + state            │   ~80,000  │     6.7%   │           │
│  ├────────────────────────────────────┼────────────┼────────────┤           │
│  │ TOTAL DUAL PROOF TX                │  ~640,000  │    53.3%   │           │
│  └────────────────────────────────────┴────────────┴────────────┘           │
│                                                                             │
│  Headroom: ~560,000 CU (46.7%)                                              │
│  Still fits in single Solana transaction                                    │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Batch FRI Verification (Future)

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    BATCH FRI VERIFICATION (THEORETICAL)                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  NOTE: FRI-based SNARKs (STARKs, Plonky2) not yet supported on Solana.      │
│  These are projected costs based on hash operations.                        │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  STARK VERIFICATION (hypothetical):                                         │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  │ Operation                          │ CU Cost    │ Notes                  │
│  ├────────────────────────────────────┼────────────┼────────────────────────│
│  │ FRI query (per query, ~40 queries) │   ~20,000  │ Merkle paths + hashes  │
│  │ Total FRI verification             │  ~800,000  │ 40 × 20k               │
│  │ Constraint evaluation              │  ~200,000  │ AIR polynomial checks  │
│  │ Public input hashing               │   ~50,000  │ Poseidon/Keccak        │
│  ├────────────────────────────────────┼────────────┼────────────────────────│
│  │ TOTAL STARK VERIFY                 │ ~1,050,000 │ 87.5% of limit         │
│  └────────────────────────────────────┴────────────┴────────────────────────┘
│                                                                             │
│  STARKs are larger proofs but avoid trusted setup.                          │
│  Not practical for Solana without syscall support for hash functions.       │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  BATCH GROTH16 (RECURSIVE AGGREGATION):                                     │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  Strategy: Aggregate N Groth16 proofs into 1 recursive proof                │
│                                                                             │
│  │ Batch Size │ Verification CU │ Per-Proof CU │ Savings vs Individual    │
│  ├────────────┼─────────────────┼──────────────┼──────────────────────────│
│  │     1      │      280,000    │    280,000   │ baseline                 │
│  │     2      │      290,000    │    145,000   │ 48% per proof            │
│  │     4      │      310,000    │     77,500   │ 72% per proof            │
│  │     8      │      350,000    │     43,750   │ 84% per proof            │
│  │    16      │      420,000    │     26,250   │ 91% per proof            │
│  └────────────┴─────────────────┴──────────────┴──────────────────────────┘
│                                                                             │
│  Aggregation adds ~10,000 CU per additional proof (amortized).              │
│  Requires off-chain recursive proving (expensive, ~10-60s per proof).       │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  GROTH16 BATCH VERIFICATION (NO RECURSION):                                 │
│  ────────────────────────────────────────────────────────────────────────   │
│                                                                             │
│  Random linear combination batching (same verifying key):                   │
│                                                                             │
│  │ Batch Size │ Verification CU │ CU per Proof │ Pairing Calls            │
│  ├────────────┼─────────────────┼──────────────┼──────────────────────────│
│  │     1      │      280,000    │    280,000   │ 3 pairings               │
│  │     2      │      350,000    │    175,000   │ 4 pairings (batched)     │
│  │     4      │      490,000    │    122,500   │ 6 pairings (batched)     │
│  │     8      │      770,000    │     96,250   │ 10 pairings (batched)    │
│  └────────────┴─────────────────┴──────────────┴──────────────────────────┘
│                                                                             │
│  Batch verification combines multiple proofs into fewer pairing operations. │
│  Formula: 2 + N pairings for N proofs (vs 3N for individual).               │
│  Requires same circuit (verifying key) for all proofs in batch.             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Solana Precompile Costs

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    SOLANA ALT_BN128 SYSCALL COSTS                           │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Solana provides native syscalls for BN254 operations:                      │
│                                                                             │
│  │ Syscall                    │ CU Cost  │ Used For                       │ │
│  ├────────────────────────────┼──────────┼────────────────────────────────│ │
│  │ sol_alt_bn128_addition     │     450  │ G1 point addition              │ │
│  │ sol_alt_bn128_multiplication│  12,000 │ G1 scalar multiplication       │ │
│  │ sol_alt_bn128_pairing      │  ~63,000 │ Per pairing (varies by pairs)  │ │
│  └────────────────────────────┴──────────┴────────────────────────────────┘ │
│                                                                             │
│  Pairing cost breakdown:                                                    │
│    Base cost: 43,000 CU                                                     │
│    Per pairing element: ~20,000 CU                                          │
│    Groth16 uses 3 pairings: 43,000 + 3×20,000 = ~103,000 CU                 │
│                                                                             │
│  Note: Actual costs may vary by Solana version and validator implementation.│
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  COMPARISON TO OTHER CHAINS:                                                │
│                                                                             │
│  │ Chain      │ Groth16 Cost   │ Unit         │ Notes                     │ │
│  ├────────────┼────────────────┼──────────────┼───────────────────────────│ │
│  │ Solana     │     ~280,000   │ CU           │ 23% of 1.2M limit         │ │
│  │ Ethereum   │     ~200,000   │ gas          │ ~1% of 30M limit          │ │
│  │ Polygon    │     ~200,000   │ gas          │ ~0.5% of 30M limit        │ │
│  │ zkSync     │      ~10,000   │ ergs         │ Native ZK, optimized      │ │
│  └────────────┴────────────────┴──────────────┴───────────────────────────┘ │
│                                                                             │
│  Solana's higher relative cost is due to:                                   │
│    1. Lower absolute CU limit (1.2M vs 30M gas)                             │
│    2. No precompile optimization for Groth16 specifically                   │
│    3. Emphasis on parallel execution over complex single-tx compute         │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Optimization Strategies

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                      CU OPTIMIZATION STRATEGIES                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. PROOF COMPRESSION (implemented)                                         │
│     - Groth16 proof: 256 bytes (compressed G1/G2 points)                    │
│     - Reduces deserialization overhead                                      │
│                                                                             │
│  2. MULTI-INSTRUCTION TRANSACTIONS                                          │
│     - Split verification across multiple instructions                       │
│     - Each instruction < 200K CU                                            │
│     - Total transaction can use full 1.2M CU                                │
│                                                                             │
│  3. LOOKUP TABLES (Address Lookup Tables)                                   │
│     - Reduces account serialization overhead                                │
│     - Saves ~1,000 CU per deduplicated account                              │
│                                                                             │
│  4. PINOCCHIO RUNTIME (implemented)                                         │
│     - No-std, zero-copy account access                                      │
│     - Minimal allocation overhead                                           │
│     - Saves ~50,000 CU vs Anchor                                            │
│                                                                             │
│  5. NULLIFIER BATCHING                                                      │
│     - Batch 4-16 nullifiers per tree update                                 │
│     - Amortizes Merkle proof verification                                   │
│     - Separate proof for tree integrity                                     │
│                                                                             │
│  6. FUTURE: DEDICATED PROVER NETWORK                                        │
│     - Off-chain recursive proof aggregation                                 │
│     - Single on-chain verification per block                                │
│     - Enables 100+ private txs per Solana slot                              │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Nullifier Indexed Merkle Tree

Zorb uses an **Indexed Merkle Tree** for nullifier tracking, inspired by [Aztec's design](https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree). This approach provides significant savings over traditional append-only Merkle trees while maintaining efficient non-membership proofs.

### Comparison to Light Protocol

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    MERKLE TREE APPROACH COMPARISON                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  LIGHT PROTOCOL (Concurrent Merkle Tree):                                   │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  • Append-only tree with changelog for concurrency                  │    │
│  │  • Each insert requires full 32-level path proof                    │    │
│  │  • Non-membership requires separate proof structure                 │    │
│  │  • High on-chain state: stores changelog buffer                     │    │
│  │  • Hashing cost: O(32 × log₂(n)) per insertion proof                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ZORB (Indexed Merkle Tree):                                                │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  • Sorted linked-list structure within tree                         │    │
│  │  • Leaves contain: (value, next_index, next_value)                  │    │
│  │  • Non-membership is implicit in tree structure                     │    │
│  │  • Low on-chain state: just current root                            │    │
│  │  • Batch insertion: N proofs share intermediate roots               │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                             │
│  ═══════════════════════════════════════════════════════════════════════    │
│                                                                             │
│  KEY DIFFERENCES:                                                           │
│                                                                             │
│  │ Aspect              │ Light CMT      │ Zorb IMT              │          │
│  ├─────────────────────┼────────────────┼───────────────────────│          │
│  │ Non-membership      │ Separate proof │ Built into structure  │          │
│  │ On-chain state      │ Changelog buf  │ Single root (32 bytes)│          │
│  │ Concurrent inserts  │ Native support │ Batch proof chaining  │          │
│  │ Constraint cost     │ ~40K per insert│ ~26.8K per insert     │          │
│  │ Subtree batching    │ Supported      │ N single insertions   │          │
│  └─────────────────────┴────────────────┴───────────────────────┘          │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Indexed Leaf Structure

Each leaf is a Poseidon hash of three field elements:

```
leafHash = Poseidon(value, next_index, next_value)

┌─────────────┬──────────────┬───────────────┐
│   value     │  next_index  │  next_value   │
│  (BN254 Fr) │   (uint32)   │   (BN254 Fr)  │
└─────────────┴──────────────┴───────────────┘
      │               │              │
      │               │              └── Value of the next larger element
      │               └── Index in tree of next larger element
      └── This nullifier's value (or 0 for empty)
```

**Sorted linked list visualization:**

```
Index:    0              1              2              3
       ┌──────┐       ┌──────┐       ┌──────┐       ┌──────┐
Value: │  0   │ ────► │ 42   │ ────► │ 100  │ ────► │ 255  │ ──► ∞
       │ →1   │       │ →2   │       │ →3   │       │ →MAX │
       └──────┘       └──────┘       └──────┘       └──────┘
```

To prove 75 is NOT in tree: Find "low leaf" (42, 2, 100) and verify 42 < 75 < 100.

### Batch Insertion Circuit

`nullifier-batch-N.circom` provides **N single insertions with root chaining** (NOT batch subtree insertion):

```
Root₀ ─── Insert N₁ ───► Root₁ ─── Insert N₂ ───► Root₂ ...
  │                        │                        │
  ▼                        ▼                        ▼
Merkle                   Merkle                   Merkle
proof for               proof for               proof for
low leaf₁              low leaf₂               low leaf₃
```

Each insertion uses the OUTPUT ROOT of the previous insertion as its INPUT ROOT, creating a chain of valid state transitions.

### Constraint Costs

| Circuit | Insertions | Constraints | Notes |
|---------|------------|-------------|-------|
| Single insertion | 1 | ~26,800 | Membership + ordering + 2 tree updates |
| nullifier-batch-4 | 4 | ~107,200 | 4 chained insertions |
| nullifier-batch-8 | 8 | ~214,400 | 8 chained insertions |
| nullifier-batch-16 | 16 | ~428,800 | 16 chained insertions |

Groth16 proof size is constant (256 bytes) regardless of circuit complexity.

### Why Chained Singles vs Batch Subtree?

- **Chained singles**: No sorting required, nullifiers placed at any available index, simple auditable logic
- **Batch subtree**: Would require pre-sorted nullifiers, complex rebalancing, nullifiers must be contiguous

For privacy transactions with 4 inputs → 4 nullifiers per tx, the chained approach is simpler and sufficient.

---

## Summary: Security Properties

| Property | Mechanism |
|----------|-----------|
| **Value Conservation** | Per-slot equation: `sumIn + publicDelta = sumOut` |
| **Asset Isolation** | Roster uniqueness prevents cross-asset routing |
| **Yield Realization** | `inValue = amount + (globalAcc - noteAcc) × amount / 1e18` |
| **Plausible Deniability** | 8 reward lines hide which 1-2 assets are actually used |
| **No Contention** | Frozen accumulator for ~18 min proof validity window |
| **Accumulator Monotonicity** | `noteAcc <= globalAcc` (can't claim future yield) |
| **Output Snapshot** | `outNoteAcc = globalAcc` (locks in yield entitlement) |
| **Double-Spend Prevention** | Indexed Merkle Tree with non-membership proofs |
| **Nullifier Atomicity** | Batch insertion proof chains N updates atomically |
