//! Error types for the shielded pool program.
//!
//! # Error Code Ranges
//!
//! | Range | Category | Description |
//! |-------|----------|-------------|
//! | 0-32 | Core | Transaction processing, proofs, amounts, accounts |
//! | 33-39 | Restaking | Stake pool integration errors |
//! | 40-47 | Unified SOL | LST pool and epoch management |
//! | 48-67 | Nullifier Tree | Indexed tree insertion and verification |
//! | 68-78 | Pool Config | Pool routing and validation |
//! | 100-108 | Groth16 | ZK proof verification failures |
//!
//! # Error Code Reference
//!
//! ## Core Errors (0-32)
//! - 0: Unauthorized
//! - 1: TransactParamsHashMismatch
//! - 2: UnknownRoot
//! - 3: InvalidPublicAmountData
//! - 4-5: InsufficientFunds (mapped to ProgramError::InsufficientFunds)
//! - 6: InvalidProof
//! - 7: InvalidFee
//! - 8: InvalidExtAmount
//! - 9: PublicAmountCalculationError
//! - 10: ArithmeticOverflow
//! - 11: DepositLimitExceeded
//! - 12: InvalidFeeRate
//! - 13: (Reserved)
//! - 14: InvalidFeeAmount
//! - 15: RecipientMismatch
//! - 16: MerkleTreeFull
//! - 17: MissingAccounts
//! - 18: NullifierAlreadyUsed
//! - 19: InvalidMint
//! - 20: MaxMintsReached
//! - 21: AssetAlreadyRegistered
//! - 22: InvalidVault
//! - 23: InvalidAmount
//! - 24: InvalidWithdrawal
//! - 25: InvalidAssetId
//! - 26: InvalidDiscriminator
//! - 27: PoolPaused
//! - 28: ProofPayloadOverflow
//! - 29: AccumulatorEpochNotReady
//! - 30: InvalidSessionState
//! - 31: InvalidTokenConfig
//! - 32: InvalidProgramAccount
//!
//! ## Restaking Errors (33-39)
//! - 33: RestakingDisabled
//! - 34: RestakeThresholdNotMet
//! - 35: RestakeRateLimitExceeded
//! - 36: UnstakeRateLimitExceeded
//! - 37: InsufficientLiquidity
//! - 38: InvalidExchangeRate
//! - 39: InvalidStakePool
//!
//! ## Unified SOL Errors (40-47)
//! - 40: InvalidLstConfig
//! - 41: StaleExchangeRate
//! - 42: TransactionExpired
//! - 43: MissingLstConfigs
//! - 44: DuplicateLstConfig
//! - 45: LstNotHarvested
//! - 46: StaleHarvestEpoch
//! - 47: RelayerFeeExceedsAmount
//!
//! ## Nullifier Tree Errors (48-67)
//! - 48: InvalidPendingIndex
//! - 49: NullifierAlreadyInserted
//! - 50: InvalidLowNullifierProof
//! - 51: InvalidLowNullifierOrdering
//! - 52: NullifierTreeFull
//! - 53: UnknownNullifierRoot
//! - 54: NullifierNotInserted
//! - 55: PendingNullifiersNotInserted
//! - 56: InvalidNullifierNonMembershipProof
//! - 57: InvalidNullifier
//! - 58: InvalidOldRoot
//! - 59: InvalidBatchSize
//! - 60: InvalidNullifierBatchInsertProof
//! - 61: InvalidNullifierPda
//! - 62: InvalidEncryptedOutputHash
//! - 63: EpochTooOld
//! - 64: EpochStillProvable
//! - 65: InvalidEarliestEpoch
//! - 66: InvalidNullifierEpochRootPda
//! - 67: NullifierStillProvable
//!
//! ## Pool Config Errors (68-78)
//! - 68: RateDecreaseNotAllowed
//! - 69: InvalidAccountOwner
//! - 70: InvalidPoolConfig
//! - 71: InvalidNullifierIndex
//! - 72: InvalidNullifierTreePda
//! - 73: InvalidSystemProgram
//! - 74: InvalidPoolConfigPda
//! - 75: InvalidCommitmentTreePda
//! - 76: InvalidGlobalConfigPda
//! - 77: InvalidReceiptTreePda
//! - 78: RelayerFeeExceedsPoolFee
//! - 79: InvalidRecipient
//! - 80: InvalidHubAuthority
//! - 81: InvalidRelayer
//! - 82: InvalidPoolProgram
//! - 83: InvalidSlotConfiguration
//!
//! ## Escrow Errors (84-91)
//! - 84: EscrowProofHashMismatch
//! - 85: EscrowUnauthorizedRelayer
//! - 86: EscrowAlreadyConsumed
//! - 87: EscrowExpired
//! - 88: EscrowNotExpired
//! - 89: EscrowInsufficientBalance
//! - 90: InvalidEscrowAccount
//! - 91: EscrowMintMismatch
//! - 92: AssetIdComputationFailed
//! - 93: UnsupportedBatchSize
//! - 94: EpochAdvanceTooSoon
//!
//! ## Groth16 ZK Proof Errors (100-108)
//! - 100: InvalidG1Length
//! - 101: InvalidG2Length
//! - 102: InvalidPublicInputsLength
//! - 103: PublicInputGreaterThanFieldSize
//! - 104: PreparingInputsG1MulFailed
//! - 105: PreparingInputsG1AdditionFailed
//! - 106: ProofVerificationFailed
//! - 107: InvalidG1
//! - 108: InvalidG2

use pinocchio::program_error::ProgramError;

/// Groth16 ZK proof verification errors.
///
/// These errors use codes 100-108 and indicate failures during ZK proof
/// verification. They are useful for debugging proof generation issues.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Groth16Error {
    InvalidG1Length,
    InvalidG2Length,
    InvalidPublicInputsLength,
    PublicInputGreaterThanFieldSize,
    PreparingInputsG1MulFailed,
    PreparingInputsG1AdditionFailed,
    ProofVerificationFailed,
    /// G1 point decompression or deserialization failed
    InvalidG1,
    /// G2 point decompression failed
    InvalidG2,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ShieldedPoolError {
    Unauthorized,
    TransactParamsHashMismatch,
    UnknownRoot,
    InvalidPublicAmountData,
    InsufficientFundsForWithdrawal,
    InsufficientFundsForFee,
    InvalidProof,
    InvalidFee,
    InvalidExtAmount,
    PublicAmountCalculationError,
    ArithmeticOverflow,
    DepositLimitExceeded,
    InvalidFeeRate,
    InvalidFeeAmount,
    RecipientMismatch,
    MerkleTreeFull,
    MissingAccounts,
    NullifierAlreadyUsed,
    InvalidMint,
    MaxMintsReached,
    AssetAlreadyRegistered,
    InvalidVault,
    InvalidAmount,
    InvalidWithdrawal,
    InvalidAssetId,
    InvalidDiscriminator,
    PoolPaused,
    ProofPayloadOverflow,
    AccumulatorEpochNotReady,
    InvalidSessionState,
    InvalidTokenConfig,
    InvalidProgramAccount,
    // Restaking errors
    RestakingDisabled,
    RestakeThresholdNotMet,
    RestakeRateLimitExceeded,
    UnstakeRateLimitExceeded,
    InsufficientLiquidity,
    InvalidExchangeRate,
    InvalidStakePool,
    InvalidAccountData,
    // Unified SOL errors
    InvalidLstConfig,
    /// Exchange rate is stale - must call harvest_lst_appreciation in same slot
    StaleExchangeRate,
    /// Transaction slot expiry has passed
    TransactionExpired,
    /// advance_unified_epoch received wrong number of LST configs
    MissingLstConfigs,
    /// advance_unified_epoch received duplicate LST config
    DuplicateLstConfig,
    /// LST was not harvested this epoch (last_harvest_epoch != accumulator_epoch)
    LstNotHarvested,
    /// Deposit attempted with stale harvest (last_harvest_epoch != accumulator_epoch - 1)
    StaleHarvestEpoch,
    /// Relayer fee exceeds the withdrawal/transfer amount
    RelayerFeeExceedsAmount,
    // Indexed nullifier tree errors
    /// Nullifier pending_index doesn't match expected
    InvalidPendingIndex,
    /// Nullifier already inserted into indexed tree
    NullifierAlreadyInserted,
    /// Invalid low nullifier proof
    InvalidLowNullifierProof,
    /// Low nullifier ordering violated (L.value < N < L.next_value)
    InvalidLowNullifierOrdering,
    /// Nullifier tree is full
    NullifierTreeFull,
    /// Unknown nullifier tree root
    UnknownNullifierRoot,
    /// Nullifier has not been inserted into indexed tree yet
    NullifierNotInserted,
    /// Pending nullifiers must be inserted before advancing epoch
    PendingNullifiersNotInserted,
    /// Invalid nullifier non-membership ZK proof
    InvalidNullifierNonMembershipProof,
    /// Nullifier value mismatch (PDA value doesn't match instruction data)
    InvalidNullifier,
    /// Old root in ZK proof doesn't match current tree root
    InvalidOldRoot,
    /// Invalid batch size for ZK batch insertion
    InvalidBatchSize,
    /// Invalid nullifier batch insert proof
    InvalidNullifierBatchInsertProof,
    /// Nullifier PDA doesn't match provided nullifier value
    InvalidNullifierPda,
    /// Encrypted output hash mismatch (SHA256(encrypted_output) != transact_params.encrypted_output_hashes)
    InvalidEncryptedOutputHash,
    /// Cannot close nullifier - insertion epoch is still provable (>= earliest_provable_epoch)
    NullifierStillProvable,
    // Epoch root errors
    /// Epoch is older than earliest_provable_epoch
    EpochTooOld,
    /// Cannot close epoch root - epoch is still provable (>= earliest_provable_epoch)
    EpochStillProvable,
    /// Invalid earliest_provable_epoch value (must be >= current and <= current_epoch - MIN_PROVABLE_EPOCHS)
    InvalidEarliestEpoch,
    /// Invalid nullifier epoch root PDA (doesn't match expected derivation or discriminator)
    InvalidNullifierEpochRootPda,
    /// Exchange rate decrease not allowed (INV-VALUE-MONOTONICITY)
    RateDecreaseNotAllowed,
    /// Invalid account owner (account owned by unexpected program)
    InvalidAccountOwner,
    /// Invalid pool config account
    InvalidPoolConfig,
    /// Invalid nullifier index (out of bounds for N_INS)
    InvalidNullifierIndex,
    /// Invalid nullifier tree PDA (doesn't match expected derivation)
    InvalidNullifierTreePda,
    /// Invalid system program
    InvalidSystemProgram,
    /// Invalid pool config PDA (doesn't match expected derivation)
    InvalidPoolConfigPda,
    /// Invalid commitment tree PDA (doesn't match expected derivation)
    InvalidCommitmentTreePda,
    /// Invalid global config PDA (doesn't match expected derivation)
    InvalidGlobalConfigPda,
    /// Invalid receipt tree PDA (doesn't match expected derivation)
    InvalidReceiptTreePda,
    /// Relayer fee exceeds pool's maximum allowed fee (fee_rate × amount)
    RelayerFeeExceedsPoolFee,
    /// Epoch advance attempted before MIN_SLOTS_PER_NULLIFIER_EPOCH slots have passed
    EpochAdvanceTooSoon,
    /// Invalid recipient address (zero or system program)
    InvalidRecipient,
    /// Hub authority account does not match expected PDA
    InvalidHubAuthority,
    /// Invalid relayer pubkey (zero pubkey)
    InvalidRelayer,
    /// Pool program account does not match expected program ID
    InvalidPoolProgram,
    /// Slot configuration mismatch (inactive slot has non-zero proof values)
    InvalidSlotConfiguration,
    // Escrow errors
    /// Escrow proof_hash does not match SHA256(session_body)
    EscrowProofHashMismatch,
    /// Relayer is not authorized to consume this escrow
    EscrowUnauthorizedRelayer,
    /// Escrow has already been consumed
    EscrowAlreadyConsumed,
    /// Escrow has expired (current_slot > expiry_slot)
    EscrowExpired,
    /// Escrow has not expired yet (cannot reclaim before expiry)
    EscrowNotExpired,
    /// Escrow vault has insufficient balance for the deposit
    EscrowInsufficientBalance,
    /// Invalid escrow account (PDA mismatch or invalid discriminator)
    InvalidEscrowAccount,
    /// Escrow mint does not match the expected mint for the deposit
    EscrowMintMismatch,
    /// Asset ID computation failed (Poseidon hash error)
    AssetIdComputationFailed,
    /// Batch size not yet supported (verification key pending trusted setup)
    UnsupportedBatchSize,
}

impl From<Groth16Error> for ProgramError {
    fn from(error: Groth16Error) -> Self {
        // Groth16 errors use codes 100-108 for debugging ZK proof failures
        match error {
            Groth16Error::InvalidG1Length => ProgramError::Custom(100),
            Groth16Error::InvalidG2Length => ProgramError::Custom(101),
            Groth16Error::InvalidPublicInputsLength => ProgramError::Custom(102),
            Groth16Error::PublicInputGreaterThanFieldSize => ProgramError::Custom(103),
            Groth16Error::PreparingInputsG1MulFailed => ProgramError::Custom(104),
            Groth16Error::PreparingInputsG1AdditionFailed => ProgramError::Custom(105),
            Groth16Error::ProofVerificationFailed => ProgramError::Custom(106),
            Groth16Error::InvalidG1 => ProgramError::Custom(107),
            Groth16Error::InvalidG2 => ProgramError::Custom(108),
        }
    }
}

impl From<ShieldedPoolError> for ProgramError {
    fn from(error: ShieldedPoolError) -> Self {
        match error {
            ShieldedPoolError::Unauthorized => ProgramError::Custom(0),
            ShieldedPoolError::TransactParamsHashMismatch => ProgramError::Custom(1),
            ShieldedPoolError::UnknownRoot => ProgramError::Custom(2),
            ShieldedPoolError::InvalidPublicAmountData => ProgramError::Custom(3),
            ShieldedPoolError::InsufficientFundsForWithdrawal => ProgramError::InsufficientFunds,
            ShieldedPoolError::InsufficientFundsForFee => ProgramError::InsufficientFunds,
            ShieldedPoolError::InvalidProof => ProgramError::Custom(6),
            ShieldedPoolError::InvalidFee => ProgramError::Custom(7),
            ShieldedPoolError::InvalidExtAmount => ProgramError::Custom(8),
            ShieldedPoolError::PublicAmountCalculationError => ProgramError::Custom(9),
            ShieldedPoolError::ArithmeticOverflow => ProgramError::Custom(10),
            ShieldedPoolError::DepositLimitExceeded => ProgramError::Custom(11),
            ShieldedPoolError::InvalidFeeRate => ProgramError::Custom(12),
            ShieldedPoolError::InvalidFeeAmount => ProgramError::Custom(14),
            ShieldedPoolError::RecipientMismatch => ProgramError::Custom(15),
            ShieldedPoolError::MerkleTreeFull => ProgramError::Custom(16),
            ShieldedPoolError::MissingAccounts => ProgramError::Custom(17),
            ShieldedPoolError::NullifierAlreadyUsed => ProgramError::Custom(18),
            ShieldedPoolError::InvalidMint => ProgramError::Custom(19),
            ShieldedPoolError::MaxMintsReached => ProgramError::Custom(20),
            ShieldedPoolError::AssetAlreadyRegistered => ProgramError::Custom(21),
            ShieldedPoolError::InvalidVault => ProgramError::Custom(22),
            ShieldedPoolError::InvalidAmount => ProgramError::Custom(23),
            ShieldedPoolError::InvalidWithdrawal => ProgramError::Custom(24),
            ShieldedPoolError::InvalidAssetId => ProgramError::Custom(25),
            ShieldedPoolError::InvalidDiscriminator => ProgramError::Custom(26),
            ShieldedPoolError::PoolPaused => ProgramError::Custom(27),
            ShieldedPoolError::ProofPayloadOverflow => ProgramError::Custom(28),
            ShieldedPoolError::AccumulatorEpochNotReady => ProgramError::Custom(29),
            ShieldedPoolError::InvalidSessionState => ProgramError::Custom(30),
            ShieldedPoolError::InvalidTokenConfig => ProgramError::Custom(31),
            ShieldedPoolError::InvalidProgramAccount => ProgramError::Custom(32),
            // Restaking errors
            ShieldedPoolError::RestakingDisabled => ProgramError::Custom(33),
            ShieldedPoolError::RestakeThresholdNotMet => ProgramError::Custom(34),
            ShieldedPoolError::RestakeRateLimitExceeded => ProgramError::Custom(35),
            ShieldedPoolError::UnstakeRateLimitExceeded => ProgramError::Custom(36),
            ShieldedPoolError::InsufficientLiquidity => ProgramError::Custom(37),
            ShieldedPoolError::InvalidExchangeRate => ProgramError::Custom(38),
            ShieldedPoolError::InvalidStakePool => ProgramError::Custom(39),
            ShieldedPoolError::InvalidAccountData => ProgramError::InvalidAccountData,
            // Unified SOL errors
            ShieldedPoolError::InvalidLstConfig => ProgramError::Custom(40),
            ShieldedPoolError::StaleExchangeRate => ProgramError::Custom(41),
            ShieldedPoolError::TransactionExpired => ProgramError::Custom(42),
            ShieldedPoolError::MissingLstConfigs => ProgramError::Custom(43),
            ShieldedPoolError::DuplicateLstConfig => ProgramError::Custom(44),
            ShieldedPoolError::LstNotHarvested => ProgramError::Custom(45),
            ShieldedPoolError::StaleHarvestEpoch => ProgramError::Custom(46),
            ShieldedPoolError::RelayerFeeExceedsAmount => ProgramError::Custom(47),
            // Indexed nullifier tree errors
            ShieldedPoolError::InvalidPendingIndex => ProgramError::Custom(48),
            ShieldedPoolError::NullifierAlreadyInserted => ProgramError::Custom(49),
            ShieldedPoolError::InvalidLowNullifierProof => ProgramError::Custom(50),
            ShieldedPoolError::InvalidLowNullifierOrdering => ProgramError::Custom(51),
            ShieldedPoolError::NullifierTreeFull => ProgramError::Custom(52),
            ShieldedPoolError::UnknownNullifierRoot => ProgramError::Custom(53),
            ShieldedPoolError::NullifierNotInserted => ProgramError::Custom(54),
            ShieldedPoolError::PendingNullifiersNotInserted => ProgramError::Custom(55),
            ShieldedPoolError::InvalidNullifierNonMembershipProof => ProgramError::Custom(56),
            ShieldedPoolError::InvalidNullifier => ProgramError::Custom(57),
            ShieldedPoolError::InvalidOldRoot => ProgramError::Custom(58),
            ShieldedPoolError::InvalidBatchSize => ProgramError::Custom(59),
            ShieldedPoolError::InvalidNullifierBatchInsertProof => ProgramError::Custom(60),
            ShieldedPoolError::InvalidNullifierPda => ProgramError::Custom(61),
            ShieldedPoolError::InvalidEncryptedOutputHash => ProgramError::Custom(62),
            ShieldedPoolError::NullifierStillProvable => ProgramError::Custom(67),
            // Epoch root errors
            ShieldedPoolError::EpochTooOld => ProgramError::Custom(63),
            ShieldedPoolError::EpochStillProvable => ProgramError::Custom(64),
            ShieldedPoolError::InvalidEarliestEpoch => ProgramError::Custom(65),
            ShieldedPoolError::InvalidNullifierEpochRootPda => ProgramError::Custom(66),
            ShieldedPoolError::RateDecreaseNotAllowed => ProgramError::Custom(68),
            ShieldedPoolError::InvalidAccountOwner => ProgramError::Custom(69),
            ShieldedPoolError::InvalidPoolConfig => ProgramError::Custom(70),
            ShieldedPoolError::InvalidNullifierIndex => ProgramError::Custom(71),
            ShieldedPoolError::InvalidNullifierTreePda => ProgramError::Custom(72),
            ShieldedPoolError::InvalidSystemProgram => ProgramError::Custom(73),
            ShieldedPoolError::InvalidPoolConfigPda => ProgramError::Custom(74),
            ShieldedPoolError::InvalidCommitmentTreePda => ProgramError::Custom(75),
            ShieldedPoolError::InvalidGlobalConfigPda => ProgramError::Custom(76),
            ShieldedPoolError::InvalidReceiptTreePda => ProgramError::Custom(77),
            ShieldedPoolError::RelayerFeeExceedsPoolFee => ProgramError::Custom(78),
            ShieldedPoolError::InvalidRecipient => ProgramError::Custom(79),
            ShieldedPoolError::InvalidHubAuthority => ProgramError::Custom(80),
            ShieldedPoolError::InvalidRelayer => ProgramError::Custom(81),
            ShieldedPoolError::InvalidPoolProgram => ProgramError::Custom(82),
            ShieldedPoolError::InvalidSlotConfiguration => ProgramError::Custom(83),
            // Escrow errors
            ShieldedPoolError::EscrowProofHashMismatch => ProgramError::Custom(84),
            ShieldedPoolError::EscrowUnauthorizedRelayer => ProgramError::Custom(85),
            ShieldedPoolError::EscrowAlreadyConsumed => ProgramError::Custom(86),
            ShieldedPoolError::EscrowExpired => ProgramError::Custom(87),
            ShieldedPoolError::EscrowNotExpired => ProgramError::Custom(88),
            ShieldedPoolError::EscrowInsufficientBalance => ProgramError::Custom(89),
            ShieldedPoolError::InvalidEscrowAccount => ProgramError::Custom(90),
            ShieldedPoolError::EscrowMintMismatch => ProgramError::Custom(91),
            ShieldedPoolError::AssetIdComputationFailed => ProgramError::Custom(92),
            ShieldedPoolError::UnsupportedBatchSize => ProgramError::Custom(93),
            ShieldedPoolError::EpochAdvanceTooSoon => ProgramError::Custom(94),
        }
    }
}
