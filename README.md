# ZORB Solana Privacy Hackathon Submission 2026

> **Warning**: This software is unaudited and experimental. Use at your own risk. Do not use with funds you cannot afford to lose.

## Our Contributions

### 1. Unified SOL: Yield-Bearing Shielded SOL

SOL equivalents (wSOL, jitoSOL, mSOL, bSOL) aggregate into unified anonymity set for fungibility in-circuit, receive yield-bearing shielded SOL that earns staking yield in-circuit.

<p align="center">
  <img src="docs/unified-sol.svg" alt="Unified SOL Pool" width="700"/>
</p>

See [Yield Mechanism](docs/YIELD_MECHANISM.md) for details.

### 2. Rent-Free Nullifier Scheme

Commitment and nullifier scheme for private state involves maintaining a consistent nullifier set.

We push nullifiers into our indexed merkle tree, allowing for PDA closure some time later. Prior work is LightProtocol's concurrent merkle tree, but we use an [indexed merkle tree described by Aztec](https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree) for lower constraints (8-16x) due to lower height.

All transactions must present a non-membership proof against a provable nullifier epoch. This is delegatable to a proof server (and with GPUs, is extremely fast), as only nullifiers are exposed. 

Batch insertion and PDA closure amortizes cost of maintaining nullifier set to **transaction fees only**, no rent.

#### Why This Matters: Privacy Cash Protocol Analysis

We analyzed [Privacy Cash](https://privacy.cash) (`9fhQBbumKEFuXtMBDw8AaQyAjCorLGJQiS3skWZdQyQD`), a live Solana privacy protocol, to quantify nullifier PDA costs. The protocol has **spent &#126;214 SOL (&#126;$42,800) on nullifier PDA rent** that remains permanently locked:

| Metric | Value |
|--------|-------|
| **Period** | Aug 5, 2025 – Jan 22, 2026 |
| **Transactions** | 112,312 |
| **Nullifier PDAs created** | 224,536 |
| **Rent per nullifier PDA** | &#126;0.000954 SOL (64 bytes) |
| **Total nullifier rent locked** | **&#126;214 SOL (&#126;$42,800)** |

Nullifier PDAs cannot be closed—they must persist forever to prevent double-spends. Our indexed Merkle tree design with epoch-based batch insertion and PDA closure eliminates this permanent rent burden, reducing costs to transaction fees only.

*Analysis performed over 112,312 transactions:*
- First: [`3SYDtthD...uN44`](https://solscan.io/tx/3SYDtthDLD83gDgSKAGLX3nLnhLmng1VeRTNNcrB4dNXqwYsNTUP35HBurwDx5xM4bCguMBQui8BmHGfPsd5uN44) (Aug 5, 2025)
- Last: [`586nTb9p...XBt`](https://solscan.io/tx/586nTb9p6sZWBPzqgimVgFYGx6uUwpmhY8eSMeqYeyQCscgDUHgcfnKmAFc9EqCKcQyG12MPJ4KsQXK6RuWeSXBt) (Jan 22, 2026)

View protocol analytics: [Privacy Cash on OrbMarkets](https://orbmarkets.io/protocol/9fhQBbumKEFuXtMBDw8AaQyAjCorLGJQiS3skWZdQyQD)



### 3. Multi-Asset Transact with Public-Slot Routing

Multi-asset split-join circuit handles value conservation of transacting with optional *yield accumulation*.

**`nRewardLines`** public yield accumulators, **`nRosterSlots`** private routing slots, and **`nPublicLines`** deposit/withdrawal lines interconnect via one-hot selectors—notes and public lines select into roster slots, which fetch accumulators from reward lines. Per-slot value conservation is enforced while the mapping remains hidden, creating plausible deniability.

See [Circuit Routing Architecture](docs/CIRCUIT_ROUTING.md) and [Yield Mechanism](docs/YIELD_MECHANISM.md) for details.

---

## Live Demo

**Devnet Application**: [https://devnet.zorb.cash/](https://devnet.zorb.cash/)

### Pinocchio/Panchor Solana Programs

Three Rust programs built with [Pinocchio](https://github.com/febo/pinocchio) (low-CU runtime) and [Panchor](vendor/panchor/) (lightweight `no_std` framework):

| Program | Location | Description |
|---------|----------|-------------|
| **Shielded Pool** | `programs/shielded-pool/` | Hub/router that verifies Groth16 proofs, manages commitment Merkle tree, tracks nullifiers, and dispatches to pool programs via CPI |
| **Token Pool** | `programs/token-pool/` | Handles SPL token deposits/withdrawals with epoch-based yield distribution |
| **Unified SOL Pool** | `programs/unified-sol-pool/` | Manages LST (jitoSOL, mSOL, etc.) with exchange rate conversion and staking yield capture |

### Groth16 over BN254 Circuits

Circom circuits for zero-knowledge proof generation:

| Circuit | Location | Purpose |
|---------|----------|---------|
| `transaction.circom` | `circuits/circom/` | Main shielded transaction (4-in-4-out UTXO model) |
| `nullifier-*.circom` | `circuits/circom/` | Batch nullifier insertion for indexed Merkle tree |
| `lib/*.circom` | `circuits/circom/lib/` | Shared templates: Poseidon hashing, Merkle proofs, reward computation |

#### Core Circuit Templates

| Template | File | Constraints | Description |
|----------|------|-------------|-------------|
| `MerkleProof(levels)` | `lib/merkle.circom` | &#126;246 × levels | Verifies Merkle inclusion proof, returning computed root |
| `NoteCommitment()` | `lib/notes.circom` | &#126;768 | Computes `Poseidon(domain, version, assetId, amount, pk, blinding, rewardAcc, rho)` |
| `ComputeNullifier()` | `lib/notes.circom` | &#126;393 | Position-independent nullifier: `Poseidon(nk, rho, commitment)` |
| `ComputeReward()` | `lib/rewards.circom` | &#126;78 | Calculates `floor(amount × (globalAcc - noteAcc) / 1e18)` |
| `IndexedMerkleTreeNonMembership(H)` | `lib/indexed-merkle-tree.circom` | &#126;7,244 | Proves value NOT in tree via low-element ordering + Merkle proof |
| `OneHotValidator(n)` | `lib/one-hot.circom` | 3n + 2 | Validates one-hot selector with enabled-mask and dot-product binding |

Key cryptographic primitives:
- **Commitment scheme**: Poseidon hash over BN254 scalar field
- **Nullifier derivation**: Position-independent (Orchard-style) using `rho` field
- **Merkle tree**: 32-level indexed Merkle tree (Aztec-style)
- **Reward accumulator**: Fixed-point arithmetic for yield distribution

## Devnet Deployment

These may be changed without notice.

| Network | Program | Address |
|---------|---------|---------|
| Devnet | Shielded Pool | `GkMmgCdkA5YRXi3BEUSgtGLC3m4iiT926GUVkfqauMU6` |
| Devnet | Token Pool | `7py6sKLtEk7TcHvpBeD16ccfF4ypRsY6HkpJqN9oSC3S` |
| Devnet | Unified SOL | `3G9QUkFQL7jMiUSYsL6z1CzfvPXirumN3B7a3pLHqAXf` |

---

## Security Properties

| Property | Mechanism |
|----------|-----------|
| **Value Conservation** | Per-slot equation: `sumIn + publicDelta = sumOut` |
| **Asset Isolation** | Roster uniqueness prevents cross-asset routing |
| **Yield Realization** | `reward = amount × (globalAcc - noteAcc) / 1e18` |
| **Plausible Deniability** | 8 reward lines hide which 1-2 assets are used |
| **No State Contention** | Frozen accumulator for &#126;18 min proof validity |
| **Double-Spend Prevention** | Indexed Merkle Tree with non-membership proofs |
| **Nullifier Atomicity** | Batch insertion chains N updates atomically |

---

## Project Structure

```
.
├── programs/
│   ├── shielded-pool/     # Hub program with ZK verification
│   ├── token-pool/        # SPL token deposit/withdraw
│   └── unified-sol-pool/  # LST with exchange rates
├── circuits/
│   ├── circom/            # Circom ZK circuits
│   └── src/               # TypeScript circuit utilities
├── crates/
│   ├── zorb-program-ids/  # Centralized program IDs
│   └── zorb-pool-interface/ # Shared pool interface
└── vendor/
    └── panchor/           # Lightweight Solana framework
```

## Building

```bash
# Build all programs
cargo build-sbf

# Build with devnet program IDs
cargo build-sbf --features devnet

# Run tests
cargo test
```

## Technology

- **Zero-Knowledge Proofs**: Groth16 with BN254 curve
- **Framework**: [Panchor](vendor/panchor/) - lightweight no_std Solana framework
- **Runtime**: Pinocchio for low-CU operations

## License

Apache-2.0
