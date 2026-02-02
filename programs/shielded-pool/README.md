# Shielded Pool

A Solana program implementing the privacy hub for the Zorb shielded pool system with zero-knowledge proof verification.

## Overview

Shielded Pool is the **central orchestrator** of the Zorb privacy system. It:

- Verifies Groth16 ZK proofs for private transactions
- Maintains merkle trees for commitments, receipts, and nullifiers
- Routes deposits/withdrawals to asset-specific pool programs via CPI
- Manages fee calculations and validation
- Handles the 3-step transact flow for chunked proof uploads

**Program ID:** `zrbus1K97oD9wzzygehPBZMh5EVXPturZNgbfoTig5Z`

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    SHIELDED-POOL (Hub)                          │
│                                                                 │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │ CommitmentTree  │  │  ReceiptTree    │  │  NullifierTree  │  │
│  │   (Height 26)   │  │   (Height 26)   │  │   (Indexed)     │  │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘  │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   Groth16 Verifier                      │    │
│  │              Verify ZK proofs on-chain                  │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    Pool Router                          │    │
│  │     Route to Token Pool or Unified SOL Pool via CPI     │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
                 │                              │
                 ▼ CPI                          ▼ CPI
┌──────────────────────┐            ┌──────────────────────┐
│    TOKEN-POOL        │            │  UNIFIED-SOL-POOL    │
│  • 1:1 token vaults  │            │  • Multi-LST support │
│  • SPL tokens        │            │  • Exchange rates    │
└──────────────────────┘            └──────────────────────┘
```

## Structure

- `src/lib.rs` - Program entry point with panchor macro
- `src/instructions/` - Instruction handlers
  - `transact/` - 3-step transact flow (init, upload, execute, close)
  - `nullifier/` - Nullifier tree operations (insert, batch, epoch management)
  - `admin/` - Initialization, pool registration, authority management
  - `utility/` - Poseidon hash, logging, test helpers
- `src/state/` - Account definitions
  - `global_config.rs` - Global pool configuration
  - `pool_config.rs` - Per-asset pool routing configuration
  - `commitment_merkle_tree.rs` - Note commitment tree
  - `receipt_merkle_tree.rs` - Transaction receipt tree
  - `nullifier_indexed_tree.rs` - Nullifier tracking
  - `transact_session.rs` - Chunked proof upload session
- `src/groth16/` - ZK proof verification
- `src/pool_cpi.rs` - CPI helpers for pool programs
- `idl/` - Program IDL for client generation

## Building

```bash
# Build the program
cargo build-sbf -p shielded-pool

# Build for tests
cargo build -p shielded-pool
```

## Testing

The tests use LiteSVM for fast local testing without needing a full validator.

```bash
# Run all tests
cargo test -p shielded-pool

# Run with output
cargo test -p shielded-pool -- --nocapture
```

## Instructions

### Transact Instructions (0-31)

The privacy-preserving transaction flow using chunked proof uploads:

| Disc | Instruction | Description |
|------|-------------|-------------|
| 0 | `InitTransactSession` | Create temporary session for chunked proof upload |
| 1 | `UploadTransactChunk` | Upload proof data in chunks (due to tx size limits) |
| 2 | `ExecuteTransact` | Execute shielded transaction using uploaded proof |
| 3 | `CloseTransactSession` | Close session account and reclaim rent |

### Utility Instructions (32-63)

| Disc | Instruction | Description |
|------|-------------|-------------|
| 32 | `PoseidonHash` | Compute Poseidon hash (utility) |
| 33 | `Log` | Emit structured events via CPI |
| 34 | `TestGroth16` | Test Groth16 proof verification |

### Nullifier Tree Instructions (64-127)

| Disc | Instruction | Description |
|------|-------------|-------------|
| 64 | `InitNullifierTree` | DEPRECATED - preserved for backwards compatibility |
| 65 | `AdvanceNullifierEpoch` | Advance epoch for batch finalization |
| 66 | `CloseInsertedNullifier` | Close nullifier PDA after insertion finalized |
| 67 | `SingleInsertNullifier` | Insert single nullifier into indexed tree |
| 68 | `NullifierBatchInsert` | Insert batch of nullifiers using ZK proof |
| 69 | `AdvanceEarliestProvableEpoch` | Advance earliest provable epoch |
| 70 | `CloseEpochRoot` | Close EpochRoot PDA after epoch no longer provable |

### Admin Instructions (192-255)

| Disc | Instruction | Description |
|------|-------------|-------------|
| 192 | `Initialize` | Initialize shielded pool with merkle trees |
| 193 | `SetPoolPaused` | Pause/unpause the pool |
| 194 | `RegisterTokenPool` | Register a token pool with the hub |
| 195 | `RegisterUnifiedSolPool` | Register unified SOL pool with the hub |
| 196 | `SetPoolConfigActive` | Enable/disable pool routing for an asset |
| 197 | `TransferAuthority` | Initiate two-step authority transfer |
| 198 | `AcceptAuthority` | Complete two-step authority transfer |

## Accounts

### GlobalConfig

Global pool configuration (singleton).

**Seeds:** `["global_config"]`

**Fields:**
```rust
authority: Pubkey,           // Pool authority (can pause, register pools)
pending_authority: Pubkey,   // For two-step transfer
is_paused: u8,               // 0 = active, 1 = paused
```

### PoolConfig

Per-asset routing configuration linking hub to pool programs.

**Seeds:** `["pool_config", asset_id]`

**Fields:**
```rust
pool_type: u8,               // 0 = Token, 1 = UnifiedSol
is_active: u8,               // Whether pool is active
pool_program: Pubkey,        // Program ID to CPI to
asset_id: [u8; 32],          // For matching proof.public_asset_ids
```

### TransactSession

Temporary account for chunked proof uploads.

**Seeds:** `["transact_session", authority, nonce]`

**Fields:**
```rust
authority: Pubkey,           // User who created the session
nonce: [u8; 8],              // For multiple concurrent sessions
created_slot: u64,           // For expiry tracking
data_len: u32,               // Total expected data length
// Variable-length body: proof + params + outputs
```

**Constraints:**
- Max data: 4096 bytes
- Session expiry: 216,000 slots (~24 hours)

### Merkle Trees

| Account | Discriminator | Purpose |
|---------|---------------|---------|
| `CommitmentMerkleTree` | 1 | Height 26 tree for note commitments |
| `ReceiptMerkleTree` | 2 | Height 26 tree for transaction receipts |
| `NullifierIndexedTree` | 10 | Indexed tree for nullifier non-membership proofs |
| `EpochRoot` | 12 | Stores epoch-based merkle roots for provability |

## 3-Step Transact Flow

Due to Solana transaction size limits, ZK proofs are uploaded in chunks:

```
1. InitTransactSession
   → Creates session account with expected data size
   → User signs to authorize session

2. UploadTransactChunk (repeated)
   → Upload proof data in chunks
   → Each chunk appends to session data

3. ExecuteTransact
   → Verify Groth16 proof from session data
   → Update commitment/receipt trees
   → Call pool program via CPI for asset operations
   → Emit events

4. CloseTransactSession (optional)
   → Reclaim rent from session account
```

## Pool Routing

The hub routes deposits/withdrawals to pool programs based on `asset_id`:

### Token Pool Routing

```
asset_id = poseidon(token_mint)
pool_type = 0 (Token)
```

### Unified SOL Pool Routing

```
asset_id = [0x00...0x01]  // Fixed constant
pool_type = 1 (UnifiedSol)
```

### CPI Flow

```
Hub (verifies proof) → calculates amount, fee
                     → CPI to pool program with (amount, expected_output)
Pool → executes transfer
     → returns fee via return data
Hub → handles relayer fee distribution
```

## Events

Events are emitted via self-CPI through the `Log` instruction. Each event has a unique discriminator.

### Core Events (1-15)

| Disc | Event | Description |
|------|-------|-------------|
| 1 | `NewCommitment` | Note commitment added to tree |
| 2 | `NewNullifier` | Nullifier spent (note consumed) |
| 3 | `NewReceipt` | Transaction receipt recorded |
| 4 | `NullifierBatchInserted` | Batch of nullifiers inserted via ZK proof |
| 6 | `NullifierEpochAdvanced` | Nullifier epoch advanced, root snapshot created |
| 7 | `EarliestProvableNullifierEpochAdvanced` | Earliest provable nullifier epoch updated (enables GC) |
| 8 | `NullifierLeafInserted` | Per-nullifier leaf data during batch insert |
| 9 | `NullifierPdaClosed` | Nullifier PDA closed, rent reclaimed |
| 10 | `NullifierEpochRootClosed` | Epoch root PDA closed, rent reclaimed |

### Transfer Events (16-31)

| Disc | Event | Description |
|------|-------|-------------|
| 16 | `DepositEscrowCreated` | Deposit escrow created for relayer-assisted deposit |
| 17 | `DepositEscrowClosed` | Deposit escrow closed, tokens returned |

### Admin Events (48-63)

| Disc | Event | Description |
|------|-------|-------------|
| 48 | `PoolRegistered` | Pool registered with the hub |
| 49 | `AuthorityTransferInitiated` | Two-step authority transfer initiated |
| 50 | `AuthorityTransferCompleted` | Two-step authority transfer completed |
| 51 | `PoolPauseChanged` | Pool pause state changed (pause or unpause) |
| 52 | `PoolConfigActiveChanged` | Pool config active state changed for an asset |
| 53 | `PoolInitialized` | Pool initialized (genesis event) |

## Deployment

### Prerequisites

1. Solana CLI tools installed
2. Funded keypair for deployment

### Build and Deploy

```bash
# Build
cargo build-sbf -p shielded-pool

# Deploy to devnet
solana config set --url devnet
solana program deploy ./target/deploy/shielded_pool.so

# Deploy to mainnet
solana config set --url mainnet-beta
solana program write-buffer ./target/deploy/shielded_pool.so
solana program deploy --buffer <BUFFER_PUBKEY> --program-id <PROGRAM_KEYPAIR>
```

### Post-Deployment Initialization

1. **Initialize Global Config**:
   ```typescript
   await program.methods.initialize().accounts({
     authority: admin,
   }).rpc();
   ```

2. **Initialize Commitment Tree** (height 26):
   ```typescript
   await program.methods.initializeCommitmentTree({ height: 26 }).rpc();
   ```

3. **Initialize Receipt Tree** (height 26):
   ```typescript
   await program.methods.initializeReceiptTree({ height: 26 }).rpc();
   ```

4. **Initialize Nullifier Tree**:
   ```typescript
   await program.methods.initializeNullifierTree().rpc();
   ```

5. **Register Pool Programs**:
   ```typescript
   // Register token pool
   await program.methods.registerTokenPool({
     poolProgram: TOKEN_POOL_PROGRAM_ID,
     assetId: computeAssetId(tokenMint),
   }).rpc();

   // Register unified SOL pool
   await program.methods.registerUnifiedSolPool({
     poolProgram: UNIFIED_SOL_POOL_PROGRAM_ID,
   }).rpc();
   ```

### Upgrading

```bash
# Write new program to buffer
solana program write-buffer ./target/deploy/shielded_pool.so

# Upgrade the program
solana program upgrade <BUFFER_PUBKEY> <PROGRAM_ID> --upgrade-authority <AUTHORITY_KEYPAIR>
```

## Features

- `localnet` - Removes admin key restrictions for local testing
- `test-mode` - Bypasses ZK proof verification for testing

**Warning:** Never enable these features in production builds.

## Security Considerations

- Valid admin keypair required for privileged operations
- Use multisig for upgrade authority in production
- Test thoroughly on devnet before mainnet deployment
- ZK proofs verified on-chain using Groth16
- Two-step authority transfer prevents accidental transfers
- Session accounts expire after 24 hours

## Related Documentation

- Protocol specification: `../../../app/docs/protocol/specification.md`
- Transaction flow: `../../../app/docs/protocol/transact-flow.md`
- Nullifier tree: `../../../app/docs/protocol/nullifier-tree.md`
- Token pool: `../token-pool/README.md`
- Unified SOL pool: `../unified-sol-pool/README.md`
