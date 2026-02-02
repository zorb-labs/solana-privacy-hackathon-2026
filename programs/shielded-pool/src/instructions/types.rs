use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::IdlType;
use pinocchio::pubkey::Pubkey;

use crate::state::NULLIFIER_TREE_HEIGHT;

/// Size of the public reward registry (circuit: nRewardLines).
/// On-chain yield accumulators for supported assets. Each entry is an (assetId, globalAcc) pair.
/// Used to calculate accrued rewards when spending notes.
pub const N_REWARD_LINES: usize = 8;

/// Number of public deposit/withdrawal lines (circuit: nPublicLines).
/// Each line represents a visible value flow between the shielded pool and public accounts.
/// Positive amounts = deposits into pool, negative = withdrawals from pool.
pub const N_PUBLIC_LINES: usize = 2;

/// Number of private asset routing slots (circuit: nRosterSlots).
/// The roster is a private array of asset slots used to route value flows.
/// Acts as a "mixing board" where input/output notes and public lines are routed.
pub const N_ROSTER_SLOTS: usize = 4;

/// Number of input notes in a transaction
pub const N_INS: usize = 4;

/// Number of output notes in a transaction
pub const N_OUTS: usize = 4;

/// Size of the Proof struct in bytes (for zero-copy access)
pub const PROOF_SIZE: usize = core::mem::size_of::<TransactProofData>();

/// Zero-knowledge proof for shielded transactions.
/// Public inputs must match the circuit's public signals.
///
/// This struct is Pod-compatible for zero-copy deserialization from account data.
/// The memory layout matches Borsh serialization (all fixed-size arrays, no length prefixes).
///
/// All fields are big-endian.
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, IdlType)]
pub struct TransactProofData {
    // === Groth16 proof elements ===
    /// Groth16 proof element A (G1 point, big-endian)
    pub proof_a: [u8; 32],
    /// Groth16 proof element B (G2 point, big-endian)
    pub proof_b: [u8; 64],
    /// Groth16 proof element C (G1 point, big-endian)
    pub proof_c: [u8; 32],

    // === Public inputs (must match circuit order) ===
    /// Commitment merkle tree root (big-endian)
    /// Circuit: commitmentRoot
    pub commitment_root: [u8; 32],
    /// Hash of transact params (big-endian)
    /// Circuit: transactParamsHash
    pub transact_params_hash: [u8; 32],
    /// Asset IDs for public flow (big-endian)
    /// Zero values indicate unused slots
    /// Circuit: publicAssetId[nPublicLines]
    pub public_asset_ids: [[u8; 32]; N_PUBLIC_LINES],
    /// Net change per asset (big-endian, signed)
    /// Positive=deposit, negative=withdraw, zero=unused
    /// Note: Differs from ext_amount in TransactParams:
    ///   - ext_amount = gross amount (fee charged on this)
    ///   - public_amounts = pool boundary crossing:
    ///       Deposits: net (after deposit fee)
    ///       Withdrawals: gross (before withdrawal fee)
    /// Circuit: publicAmount[nPublicLines]
    pub public_amounts: [[u8; 32]; N_PUBLIC_LINES],
    /// Nullifiers for spent notes (big-endian)
    /// Circuit: nullifiers[nInputNotes]
    pub nullifiers: [[u8; 32]; N_INS],
    /// Commitments for newly created notes (big-endian)
    /// Circuit: commitments[nOutputNotes]
    pub commitments: [[u8; 32]; N_OUTS],
    /// Global reward accumulator per reward line (big-endian)
    /// Circuit: rewardAcc[nRewardLines]
    pub reward_acc: [[u8; 32]; N_REWARD_LINES],
    /// Asset ID per reward line (big-endian)
    /// Circuit: rewardAssetId[nRewardLines]
    pub reward_asset_id: [[u8; 32]; N_REWARD_LINES],
}

// Manual Borsh implementation for Proof (Pod struct)
// Since Proof only contains fixed-size arrays, Borsh serialization is identical to raw bytes.
impl BorshSerialize for TransactProofData {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

impl BorshDeserialize for TransactProofData {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; PROOF_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(*bytemuck::from_bytes(&bytes))
    }
}

/// Size of TransactParams in bytes (for zero-copy access)
pub const TRANSACT_PARAMS_SIZE: usize = core::mem::size_of::<TransactParams>();

/// Bounded parameters for shielded transactions.
/// Pod-compatible for zero-copy deserialization from session data.
///
/// These parameters are cryptographically bound by the ZK proof's transact_params_hash.
/// The proof signs over the hash of these parameters, preventing modification.
/// All fields are included in the transact_params_hash to prevent relayer malleability.
///
/// Note: Uses Pubkey type which is `[u8; 32]` in pinocchio, making this Pod-compatible.
///
/// WARNING: Field order is fixed for binary compatibility (repr(C) Pod struct).
/// Do not reorder fields without updating all serialization code (Rust + TypeScript).
///
/// TODO: Symmetric wallet + token account fields for both relayer and recipients
/// ──────────────────────────────────────────────────────────────────────────────
/// Currently:
///   - relayer: wallet address only (token account derived as canonical ATA)
///   - recipients: token account addresses only (owner not hash-bound)
///
/// Planned change (requires circuit update):
///   - relayer: Pubkey              → relayer wallet address
///   - relayer_token_accounts: [Pubkey; N_PUBLIC_LINES] → relayer token accounts per asset
///   - recipient_owners: [Pubkey; N_PUBLIC_LINES]       → recipient wallet addresses
///   - recipient_token_accounts: [Pubkey; N_PUBLIC_LINES] → recipient token accounts
///
/// This provides:
///   1. Both identity (wallet) AND destination (token account) hash-bound to proof
///   2. Symmetric validation for relayer and recipients
///   3. Defense-in-depth: validate token_account.owner == wallet at execution time
/// ──────────────────────────────────────────────────────────────────────────────
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, IdlType)]
pub struct TransactParams {
    // =========================================================================
    // PER-SLOT: Asset Identification (WHAT)
    // ===========================  ==============================================
    /// Poseidon hash of mint per public line.
    /// Must match proof's public_asset_ids. Prevents relayer from substituting different assets.
    pub asset_ids: [[u8; 32]; N_PUBLIC_LINES],
    /// Token mint addresses per public line.
    /// Program verifies Poseidon(mint) == asset_id.
    pub mints: [Pubkey; N_PUBLIC_LINES],

    // =========================================================================
    // PER-SLOT: Value Flow (HOW MUCH)
    // =========================================================================
    /// Net external amount change per public line:
    /// - Positive: deposit amount (tokens flowing into the pool)
    /// - Negative: withdrawal amount (tokens flowing out of the pool, net to recipient)
    /// - Zero: unused slot or pure shielded transfer
    pub ext_amounts: [i64; N_PUBLIC_LINES],
    /// Protocol fee in token base units per public line.
    /// Fee is charged on |ext_amount| (what crosses the boundary).
    pub fees: [u64; N_PUBLIC_LINES],

    // =========================================================================
    // PER-SLOT: Routing (WHERE)
    // =========================================================================
    /// Recipient SPL token account addresses for withdrawals per public line.
    /// The token account receives |ext_amount| - relayer_fee for that asset.
    /// For deposits or pure transfers, this should be ZERO_PUBKEY.
    /// Note: Unlike relayer (where we derive token account from wallet via ATA),
    /// recipients are specified as token account addresses directly, giving the
    /// prover flexibility to withdraw to any valid token account.
    pub recipients: [Pubkey; N_PUBLIC_LINES],
    /// Fee paid to relayer per public line in token base units.
    /// Derived from ext_amount: user_receives = |ext_amount| - relayer_fee.
    pub relayer_fees: [u64; N_PUBLIC_LINES],

    // =========================================================================
    // GLOBAL: Transaction Metadata
    // =========================================================================
    /// Relayer wallet address that submits the transaction.
    /// Receives relayer_fees for their service.
    pub relayer: Pubkey,
    /// Slot expiry for this transaction.
    /// Transaction will fail if current slot > slot_expiry.
    /// Set to 0 to disable expiry check.
    pub slot_expiry: u64,
    /// SHA256 hashes of encrypted output ciphertexts.
    /// Binds encrypted outputs to the proof, preventing relayer malleability.
    /// Program verifies SHA256(encrypted_outputs[i]) == encrypted_output_hashes[i].
    pub encrypted_output_hashes: [[u8; 32]; N_OUTS],
}

// Manual Borsh implementation for TransactParams (Pod struct - just copy bytes)
impl BorshSerialize for TransactParams {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

impl BorshDeserialize for TransactParams {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; TRANSACT_PARAMS_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(*bytemuck::from_bytes(&bytes))
    }
}

/// Zero pubkey constant for transfers with no external recipient
pub const ZERO_PUBKEY: Pubkey = [0u8; 32];

impl TransactParams {
    /// Get total relayer fee across all assets
    #[inline]
    pub fn total_relayer_fee(&self) -> u64 {
        self.relayer_fees.iter().sum()
    }

    // === Per-asset accessors by index ===

    /// Get the recipient for asset at given index
    #[inline]
    pub fn recipient(&self, index: usize) -> Pubkey {
        self.recipients[index]
    }

    /// Get the ext_amount for asset at given index
    #[inline]
    pub fn ext_amount(&self, index: usize) -> i64 {
        self.ext_amounts[index]
    }

    /// Get the fee for asset at given index
    #[inline]
    pub fn fee(&self, index: usize) -> u64 {
        self.fees[index]
    }

    /// Get the mint for asset at given index
    #[inline]
    pub fn mint(&self, index: usize) -> Pubkey {
        self.mints[index]
    }

    /// Get the asset_id for asset at given index
    #[inline]
    pub fn asset_id(&self, index: usize) -> [u8; 32] {
        self.asset_ids[index]
    }

    /// Get the relayer_fee for asset at given index
    #[inline]
    pub fn relayer_fee(&self, index: usize) -> u64 {
        self.relayer_fees[index]
    }

    /// Find the first active asset index (non-zero ext_amount)
    pub fn primary_asset_index(&self) -> Option<usize> {
        (0..N_PUBLIC_LINES).find(|&i| self.ext_amounts[i] != 0)
    }
}

impl TransactProofData {
    // === Primary asset accessors (for backward compatibility) ===

    /// Get the primary (first) asset's asset_id
    #[inline]
    pub fn asset_id(&self) -> [u8; 32] {
        self.public_asset_ids[0]
    }

    /// Get the primary (first) asset's public_amount
    #[inline]
    pub fn public_amount(&self) -> [u8; 32] {
        self.public_amounts[0]
    }
}

// =============================================================================
// Nullifier Non-Membership Proof
// =============================================================================

/// Size of the NullifierNonMembershipProofData struct in bytes
pub const NULLIFIER_NM_PROOF_SIZE: usize = core::mem::size_of::<NullifierNonMembershipProofData>();

/// Groth16 proof for nullifier non-membership in the indexed merkle tree.
///
/// This proof verifies that all N_INS nullifiers are NOT present in the
/// nullifier indexed merkle tree (past epochs). Current epoch nullifiers
/// are checked via PDA existence.
///
/// Public inputs (verified in circuit):
/// - nullifier_root: Root of the nullifier indexed merkle tree
/// - nullifiers: The N_INS nullifier hashes (must match transact proof)
///
/// Private inputs (in circuit):
/// - For each nullifier: low element data + merkle proof
///
/// The circuit proves for each nullifier:
/// 1. low_value < nullifier < low_next_value (ordering/non-membership)
/// 2. Merkle proof of low element inclusion at nullifier_root
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, IdlType)]
pub struct NullifierNonMembershipProofData {
    /// Groth16 proof element A (G1 point, big-endian)
    pub proof_a: [u8; 32],
    /// Groth16 proof element B (G2 point, big-endian)
    pub proof_b: [u8; 64],
    /// Groth16 proof element C (G1 point, big-endian)
    pub proof_c: [u8; 32],
    /// Root of the nullifier indexed merkle tree (big-endian)
    /// Must be a known root (current or in root_history)
    pub nullifier_root: [u8; 32],
}

// Manual Borsh implementation for NullifierNonMembershipProofData (Pod struct)
impl BorshSerialize for NullifierNonMembershipProofData {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

impl BorshDeserialize for NullifierNonMembershipProofData {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; NULLIFIER_NM_PROOF_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(*bytemuck::from_bytes(&bytes))
    }
}

/// On-chain merkle non-membership proof data (alternative to ZK proof).
/// Used when ZK proof is not available or for testing.
///
/// This struct contains the low element data and merkle proof needed to
/// verify non-membership on-chain without a ZK proof.
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, IdlType)]
pub struct NullifierNonMembershipMerkleProof {
    /// Index of the low element in the tree
    pub low_index: u64,
    /// Value of the low element (the largest value < nullifier)
    pub low_value: [u8; 32],
    /// Next value of the low element (the smallest value > nullifier)
    pub low_next_value: [u8; 32],
    /// Next index of the low element
    pub low_next_index: u64,
    /// Merkle proof for the low element (26 hashes for tree height 26)
    pub merkle_proof: [[u8; 32]; NULLIFIER_TREE_HEIGHT as usize],
}

/// Size of the NullifierNonMembershipMerkleProof struct in bytes
pub const NULLIFIER_NM_MERKLE_PROOF_SIZE: usize =
    core::mem::size_of::<NullifierNonMembershipMerkleProof>();

// =============================================================================
// Nullifier Tree Insertion Types
// =============================================================================

/// Size of NullifierBatchInsertProof in bytes (1024 bytes)
pub const NULLIFIER_BATCH_INSERT_PROOF_SIZE: usize =
    core::mem::size_of::<NullifierBatchInsertProof>();

/// Groth16 proof for batch nullifier insertion into the indexed merkle tree.
///
/// This proof verifies that a batch of nullifiers can be correctly inserted:
/// - Each nullifier satisfies ordering: low_value < nullifier < low_next_value
/// - Low element merkle proofs are valid (for IN_TREE low elements)
/// - Low element updates are correct (pointers updated to new nullifier)
/// - New leaf appends are correct (inherits low's old pointers)
/// - Final root matches after all insertions
///
/// Public inputs (verified in circuit):
/// - old_root: Tree root before insertions
/// - new_root: Tree root after all insertions
/// - nullifiers[N]: The nullifier values to insert
/// - starting_index: First insertion index (tree.next_index)
///
/// Private inputs (in circuit):
/// - For each nullifier: low element data + merkle proof + type (IN_TREE/PENDING)
/// - initial_subtrees: Subtree siblings for incremental append
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, IdlType)]
pub struct NullifierBatchInsertProof {
    /// Groth16 proof element A (G1 point, big-endian)
    pub proof_a: [u8; 32],
    /// Groth16 proof element B (G2 point, big-endian)
    pub proof_b: [u8; 64],
    /// Groth16 proof element C (G1 point, big-endian)
    pub proof_c: [u8; 32],
    /// Tree root before insertions (must match tree.root)
    pub old_root: [u8; 32],
    /// Tree root after all insertions (will become tree.root)
    pub new_root: [u8; 32],
    /// Updated subtree siblings after all insertions
    /// These are copied directly to tree.subtrees
    pub new_subtrees: [[u8; 32]; NULLIFIER_TREE_HEIGHT as usize],
}

// Manual Borsh implementation for NullifierBatchInsertProof (Pod struct)
impl BorshSerialize for NullifierBatchInsertProof {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

impl BorshDeserialize for NullifierBatchInsertProof {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; NULLIFIER_BATCH_INSERT_PROOF_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(*bytemuck::from_bytes(&bytes))
    }
}

impl NullifierBatchInsertProof {
    /// Interpret bytes as NullifierBatchInsertProof (zero-copy).
    ///
    /// Returns None if the slice is too small.
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<&Self> {
        if bytes.len() < NULLIFIER_BATCH_INSERT_PROOF_SIZE {
            return None;
        }
        Some(bytemuck::from_bytes(&bytes[..NULLIFIER_BATCH_INSERT_PROOF_SIZE]))
    }
}

/// Size of NullifierBatchInsertHeader in bytes (fixed portion)
pub const NULLIFIER_BATCH_INSERT_HEADER_SIZE: usize =
    core::mem::size_of::<NullifierBatchInsertHeader>();

/// Fixed header portion of NullifierBatchInsert instruction data.
///
/// This Pod struct represents the fixed-size portion for zero-copy access.
/// Variable-length nullifiers follow immediately after.
///
/// ## Wire Format
///
/// | Offset | Size | Field |
/// |--------|------|-------|
/// | 0 | 1 | batch_size: u8 |
/// | 1 | 1024 | proof: NullifierBatchInsertProof |
/// | 1025 | 32 * batch_size | nullifiers: [[u8; 32]] |
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, IdlType)]
pub struct NullifierBatchInsertHeader {
    /// Number of nullifiers to insert (1-64)
    pub batch_size: u8,
    /// The ZK proof and new tree state
    pub proof: NullifierBatchInsertProof,
}

impl NullifierBatchInsertHeader {
    /// Parse instruction data as NullifierBatchInsertHeader (zero-copy).
    ///
    /// Returns a tuple of (header, remaining_bytes) where remaining_bytes
    /// contains the nullifiers array.
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<(&Self, &[u8])> {
        if bytes.len() < NULLIFIER_BATCH_INSERT_HEADER_SIZE {
            return None;
        }
        let (header_bytes, remaining) = bytes.split_at(NULLIFIER_BATCH_INSERT_HEADER_SIZE);
        let header: &Self = bytemuck::from_bytes(header_bytes);
        Some((header, remaining))
    }

    /// Parse nullifiers from remaining bytes.
    ///
    /// Returns nullifiers slice or None if the remaining bytes are insufficient.
    #[inline]
    pub fn parse_nullifiers<'a>(&self, remaining: &'a [u8]) -> Option<&'a [[u8; 32]]> {
        let batch_size = self.batch_size as usize;
        let nullifiers_size = batch_size * 32;

        if remaining.len() < nullifiers_size {
            return None;
        }

        let nullifiers: &[[u8; 32]] =
            bytemuck::try_cast_slice(&remaining[..nullifiers_size]).ok()?;
        Some(nullifiers)
    }
}

/// Parsed instruction data for NullifierBatchInsert.
///
/// This struct provides typed access to all instruction data fields.
/// Use [`NullifierBatchInsertData::from_bytes`] to parse from raw instruction data.
pub struct NullifierBatchInsertData<'a> {
    /// Number of nullifiers to insert (1-64)
    pub batch_size: u8,
    /// The ZK proof and new tree state
    pub proof: &'a NullifierBatchInsertProof,
    /// Nullifier values to insert (big-endian field elements)
    pub nullifiers: &'a [[u8; 32]],
}

impl<'a> NullifierBatchInsertData<'a> {
    /// Parse instruction data into typed struct (zero-copy).
    ///
    /// Returns None if the data is malformed or too short.
    #[inline]
    pub fn from_bytes(bytes: &'a [u8]) -> Option<Self> {
        let (header, remaining) = NullifierBatchInsertHeader::from_bytes(bytes)?;
        let nullifiers = header.parse_nullifiers(remaining)?;

        Some(Self {
            batch_size: header.batch_size,
            proof: &header.proof,
            nullifiers,
        })
    }
}
