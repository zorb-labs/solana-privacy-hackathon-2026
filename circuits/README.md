# Circom Circuits

This package contains circom circuits for zero-knowledge proofs, specifically for privacy-preserving transactions with multi-asset support and yield accrual.

## Prerequisites

Before setting up the circuits, ensure you have:

1. **Node.js v18+** - For running snarkjs
2. **circom v2.x** - The circuit compiler (Rust binary, not npm package)
3. **bun** - Package manager (or use npm)

### Installing circom

circom must be installed from source:

```bash
# Clone and build circom
git clone https://github.com/iden3/circom.git
cd circom
cargo build --release
cargo install --path circom

# Verify installation
circom --version
```

Or if using this project's devenv.sh, circom is included automatically.

## Quick Start

The easiest way to set up everything:

```bash
# Install dependencies
bun install

# Run the setup script (downloads ptau, compiles circuit, generates keys)
./scripts/setup-circuits.sh
```

## Manual Setup

If you prefer to run steps individually:

### Step 1: Download Powers of Tau

The Powers of Tau (ptau) file is a cryptographic ceremony output required for generating proving keys. We use the Hermez Network's trusted setup (power 18, ~288 MB):

```bash
mkdir -p ptau
curl -L https://storage.googleapis.com/zkevm/ptau/powersOfTau28_hez_final_18.ptau \
  -o ptau/powersOfTau28_hez_final_18.ptau
```

The power determines maximum constraint count: 2^18 = 262,144 constraints.

### Step 2: Compile Circuits

```bash
# Compile transaction4 circuit (4 inputs, 4 outputs)
circom circom/transaction4.circom \
  --O1 --r1cs --wasm --sym \
  --output build/transaction4
```

This generates:
- `build/transaction4/transaction4.r1cs` - Constraint system
- `build/transaction4/transaction4_js/transaction4.wasm` - Witness generator
- `build/transaction4/transaction4.sym` - Debug symbols

### Step 3: Generate Proving Key (Phase 2 Setup)

```bash
# Initial zkey from ptau and r1cs
npx snarkjs groth16 setup \
  build/transaction4/transaction4.r1cs \
  ptau/powersOfTau28_hez_final_18.ptau \
  build/transaction4/transaction4_0000.zkey

# Make a contribution (adds entropy)
npx snarkjs zkey contribute \
  build/transaction4/transaction4_0000.zkey \
  build/transaction4/transaction4_0001.zkey \
  --name="First contribution" -v

# Apply random beacon (finalizes the key)
npx snarkjs zkey beacon \
  build/transaction4/transaction4_0001.zkey \
  build/transaction4/transaction4_final.zkey \
  "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f" \
  10 -n="Final Beacon"
```

### Step 4: Export Verification Key

```bash
npx snarkjs zkey export verificationkey \
  build/transaction4/transaction4_final.zkey \
  build/transaction4/transaction4_verification_key.json
```

### Step 5: Verify Setup (Optional but Recommended)

```bash
npx snarkjs zkey verify \
  build/transaction4/transaction4.r1cs \
  ptau/powersOfTau28_hez_final_18.ptau \
  build/transaction4/transaction4_final.zkey
```

## Running Tests

Once the circuit artifacts are generated:

```bash
# Run all tests
bun test

# Run specific test file
bun test src/transaction/transaction.test.ts

# Watch mode
bun test:watch
```

Note: Integration tests that require circuit artifacts will be skipped if the artifacts are not present.

## Build Commands

| Command | Description |
|---------|-------------|
| `bun run build` | Build TypeScript |
| `bun run build:circom` | Compile all .circom files |
| `bun run test` | Run all tests |
| `bun run test:watch` | Run tests in watch mode |
| `bun run clean` | Remove all build artifacts |

## Directory Structure

```
packages/circuits/
├── circom/                        # Circom circuit source files
│   ├── transaction.circom         # Base transaction template
│   ├── transaction4.circom        # 4-input/4-output circuit
│   ├── transaction16.circom       # 16-input/2-output circuit
│   ├── merkleProof.circom         # Merkle proof verification
│   ├── merkleTree.circom          # Merkle tree operations
│   └── lib/                       # Modular template library
│       ├── keys/                  # Key derivation (Zcash Sapling-style)
│       │   ├── derive-ak.circom
│       │   ├── derive-nk.circom
│       │   ├── derive-ivk.circom
│       │   ├── derive-pk.circom
│       │   └── derive-keys.circom
│       ├── notes/                 # Note commitment and nullifiers
│       │   ├── note-commitment.circom
│       │   ├── compute-nullifier.circom
│       │   └── enforce-nullifier-uniqueness.circom
│       ├── rewards/               # Yield accrual system
│       │   ├── div-by-1e18.circom
│       │   └── compute-reward.circom
│       ├── assets/                # Multi-asset support
│       │   ├── index-select.circom
│       │   ├── enforce-asset-uniqueness.circom
│       │   └── ...
│       └── conservation/          # Value conservation
│           └── value-conservation.circom
├── src/                           # TypeScript source
│   ├── crypto/                    # Cryptographic utilities
│   │   ├── poseidon.ts            # Poseidon hash (matches circomlibjs)
│   │   ├── encryption.ts          # Note encryption (X25519/XChaCha20)
│   │   └── field.ts               # BN254 field operations
│   └── transaction/               # Transaction circuit helpers
│       └── test-utils.ts          # Test input generators
├── test/                          # Unit tests
│   └── unit/
│       ├── keys.test.ts           # Key derivation tests
│       └── rewards.test.ts        # Reward computation tests
├── scripts/                       # Build and setup scripts
│   ├── setup-circuits.sh          # One-command setup script
│   ├── build-circuits.ts          # Compile circuits
│   └── ...
├── build/                         # Generated artifacts (gitignored)
│   └── transaction4/
│       ├── transaction4.r1cs
│       ├── transaction4_js/transaction4.wasm
│       ├── transaction4_final.zkey
│       └── transaction4_verification_key.json
└── ptau/                          # Powers of Tau files (gitignored)
    └── powersOfTau28_hez_final_18.ptau
```

## Circuit Overview

The transaction circuit implements privacy-preserving transactions with:

- **Multi-asset support**: Up to 4 different asset types per transaction
- **Note model**: UTXO-based notes with commitments and nullifiers
- **Key derivation**: Zcash Sapling-style key hierarchy (ask → ak → ivk → pk)
- **Yield accrual**: Notes can accumulate rewards based on a global accumulator
- **Value conservation**: Ensures inputs + public_amount = outputs

### Public Inputs

The circuit exposes these public signals for on-chain verification:
- `root` - Merkle tree root (commitment tree membership)
- `transactParamsHash` - Hash of transaction parameters
- `publicAssetIds[2]` - Public asset IDs for deposits/withdrawals
- `publicAmounts[2]` - Public amounts (positive = deposit, negative = withdrawal)
- `inputNullifier[nIns]` - Nullifiers for spent notes
- `outputCommitment[nOuts]` - Commitments for new notes

### Constraint Count

| Circuit | Inputs | Outputs | Constraints |
|---------|--------|---------|-------------|
| transaction4 | 4 | 4 | ~74,000 |
| transaction16 | 16 | 2 | ~200,000 |

## Troubleshooting

### "Signal not found" errors

The circuit artifacts are likely outdated. Recompile:
```bash
./scripts/setup-circuits.sh
```

### "circuit too big for this power of tau ceremony"

You need a larger ptau file. The default power 18 supports up to 262,144 constraints. For larger circuits:
```bash
# Download power 20 (supports up to 1,048,576 constraints)
curl -L https://storage.googleapis.com/zkevm/ptau/powersOfTau28_hez_final_20.ptau \
  -o ptau/powersOfTau28_hez_final_20.ptau
```

### Tests timing out

Some proof generation tests have 120s timeouts. If tests timeout:
1. Ensure you're using Node.js (not Bun) for snarkjs WASM
2. Check that circuit artifacts exist in `build/transaction4/`
3. Run vitest directly: `npx vitest run`

### circom not found

Ensure circom is installed and in your PATH:
```bash
which circom
circom --version
```

## Development

### Adding a New Circuit

1. Create a `.circom` file in `circom/`
2. Add compilation to `scripts/build-circuits.ts`
3. Generate keys using snarkjs
4. Add tests

### Modifying Templates

Templates are organized by concern in `circom/lib/`. After modifying:
1. Recompile: `circom circom/transaction4.circom ...`
2. Regenerate keys (ptau → r1cs → zkey → vkey)
3. Run tests to verify
