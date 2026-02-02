use crate::errors::Groth16Error;
use ark_ff::PrimeField;
use num_bigint::BigUint;
use solana_bn254::prelude::{
    alt_bn128_g1_addition_be, alt_bn128_g1_multiplication_be, alt_bn128_pairing_be,
};

use solana_bn254::compression::prelude::{alt_bn128_g1_decompress, alt_bn128_g2_decompress};

/// BN254 base field modulus (p) for G1 point negation.
/// p = 21888242871839275222246405745257275088696311157297823662689037894645226208583
const BN254_FIELD_MODULUS: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

/// Raw compressed Groth16 proof elements.
/// This struct allows extracting proof bytes from any proof struct
/// for use with the shared verification helpers.
#[derive(Debug, Clone, Copy)]
pub struct CompressedGroth16Proof<'a> {
    /// G1 point, compressed (32 bytes, big-endian)
    pub proof_a: &'a [u8; 32],
    /// G2 point, compressed (64 bytes, big-endian)
    pub proof_b: &'a [u8; 64],
    /// G1 point, compressed (32 bytes, big-endian)
    pub proof_c: &'a [u8; 32],
}

#[derive(PartialEq, Eq, Debug)]
pub struct Groth16Verifyingkey<'a> {
    pub nr_pubinputs: usize,
    pub vk_alpha_g1: [u8; 64],
    pub vk_beta_g2: [u8; 128],
    pub vk_gamme_g2: [u8; 128],
    pub vk_delta_g2: [u8; 128],
    pub vk_ic: &'a [[u8; 64]],
}

#[derive(PartialEq, Eq, Debug)]
pub struct Groth16Verifier<'a, const NR_INPUTS: usize> {
    proof_a: &'a [u8; 64],
    proof_b: &'a [u8; 128],
    proof_c: &'a [u8; 64],
    public_inputs: &'a [[u8; 32]; NR_INPUTS],
    prepared_public_inputs: [u8; 64],
    verifyingkey: &'a Groth16Verifyingkey<'a>,
}

impl<const NR_INPUTS: usize> Groth16Verifier<'_, NR_INPUTS> {
    pub fn new<'a>(
        proof_a: &'a [u8; 64],
        proof_b: &'a [u8; 128],
        proof_c: &'a [u8; 64],
        public_inputs: &'a [[u8; 32]; NR_INPUTS],
        verifyingkey: &'a Groth16Verifyingkey<'a>,
    ) -> Result<Groth16Verifier<'a, NR_INPUTS>, Groth16Error> {
        if proof_a.len() != 64 {
            return Err(Groth16Error::InvalidG1Length);
        }

        if proof_b.len() != 128 {
            return Err(Groth16Error::InvalidG2Length);
        }

        if proof_c.len() != 64 {
            return Err(Groth16Error::InvalidG1Length);
        }

        if public_inputs.len() + 1 != verifyingkey.vk_ic.len() {
            return Err(Groth16Error::InvalidPublicInputsLength);
        }

        Ok(Groth16Verifier {
            proof_a,
            proof_b,
            proof_c,
            public_inputs,
            prepared_public_inputs: [0u8; 64],
            verifyingkey,
        })
    }

    pub fn prepare_inputs<const CHECK: bool>(&mut self) -> Result<(), Groth16Error> {
        let mut prepared_public_inputs = self.verifyingkey.vk_ic[0];

        for (i, input) in self.public_inputs.iter().enumerate() {
            if CHECK && !is_less_than_bn254_field_size_be(input) {
                return Err(Groth16Error::PublicInputGreaterThanFieldSize);
            }
            let mul_res = alt_bn128_g1_multiplication_be(
                &[&self.verifyingkey.vk_ic[i + 1][..], &input[..]].concat(),
            )
            .map_err(|_| Groth16Error::PreparingInputsG1MulFailed)?;
            prepared_public_inputs =
                alt_bn128_g1_addition_be(&[&mul_res[..], &prepared_public_inputs[..]].concat())
                    .map_err(|_| Groth16Error::PreparingInputsG1AdditionFailed)?[..]
                    .try_into()
                    .map_err(|_| Groth16Error::PreparingInputsG1AdditionFailed)?;
        }

        self.prepared_public_inputs = prepared_public_inputs;

        Ok(())
    }

    pub fn verify(&mut self) -> Result<bool, Groth16Error> {
        self.verify_common::<true>()
    }

    pub fn verify_unchecked(&mut self) -> Result<bool, Groth16Error> {
        self.verify_common::<false>()
    }

    fn verify_common<const CHECK: bool>(&mut self) -> Result<bool, Groth16Error> {
        self.prepare_inputs::<CHECK>()?;

        let pairing_input = [
            self.proof_a.as_slice(),
            self.proof_b.as_slice(),
            self.prepared_public_inputs.as_slice(),
            self.verifyingkey.vk_gamme_g2.as_slice(),
            self.proof_c.as_slice(),
            self.verifyingkey.vk_delta_g2.as_slice(),
            self.verifyingkey.vk_alpha_g1.as_slice(),
            self.verifyingkey.vk_beta_g2.as_slice(),
        ]
        .concat();

        let pairing_res = alt_bn128_pairing_be(pairing_input.as_slice())
            .map_err(|_| Groth16Error::ProofVerificationFailed)?;

        if pairing_res[31] != 1 {
            return Err(Groth16Error::ProofVerificationFailed);
        }
        Ok(true)
    }
}

pub fn is_less_than_bn254_field_size_be(bytes: &[u8; 32]) -> bool {
    let bigint = BigUint::from_bytes_be(bytes);
    bigint < ark_bn254::Fr::MODULUS.into()
}

/// Negate the y-coordinate of a G1 point: -y = p - y (mod p)
/// Input/output are big-endian 32-byte field elements.
fn negate_y(y: &[u8; 32]) -> [u8; 32] {
    let p = BigUint::from_bytes_be(&BN254_FIELD_MODULUS);
    let y_val = BigUint::from_bytes_be(y);
    let zero = BigUint::from(0u32);
    let neg_y = if y_val == zero { y_val } else { &p - &y_val };

    // Convert back to 32-byte big-endian, zero-padded
    let bytes = neg_y.to_bytes_be();
    let mut result = [0u8; 32];
    let start = 32 - bytes.len();
    result[start..].copy_from_slice(&bytes);
    result
}

/// Decompresses a Groth16 proof and negates proof_a for verification.
///
/// Groth16 verification requires -A (negated proof_a). This function:
/// 1. Decompresses proof_a (G1) and negates it: (x, y) → (x, -y)
/// 2. Decompresses proof_b (G2)
/// 3. Decompresses proof_c (G1)
///
/// Returns (proof_a_neg, proof_b, proof_c) as uncompressed points.
#[inline(never)]
pub fn decompress_and_negate_proof(
    compressed: &CompressedGroth16Proof,
) -> Result<([u8; 64], [u8; 128], [u8; 64]), Groth16Error> {
    // Decompress proof_a (G1 point, 32 bytes compressed -> 64 bytes uncompressed)
    let proof_a_decompressed =
        alt_bn128_g1_decompress(compressed.proof_a).map_err(|_| Groth16Error::InvalidG1)?;

    // Negate proof_a: (x, y) → (x, p - y)
    // Decompressed format is x (32 bytes) || y (32 bytes) in big-endian
    let mut proof_a_neg = [0u8; 64];
    proof_a_neg[..32].copy_from_slice(&proof_a_decompressed[..32]); // x unchanged
    // AUDIT FIX (H-02): Use explicit error handling instead of unwrap.
    // This should never fail since proof_a_decompressed is guaranteed to be 64 bytes,
    // but explicit handling is safer than panicking.
    let y: [u8; 32] = proof_a_decompressed[32..64]
        .try_into()
        .map_err(|_| Groth16Error::InvalidG1)?;
    proof_a_neg[32..64].copy_from_slice(&negate_y(&y)); // -y = p - y

    // Decompress proof_b (G2 point, 64 bytes compressed -> 128 bytes uncompressed)
    let proof_b =
        alt_bn128_g2_decompress(compressed.proof_b).map_err(|_| Groth16Error::InvalidG2)?;

    // Decompress proof_c (G1 point, 32 bytes compressed -> 64 bytes uncompressed)
    let proof_c =
        alt_bn128_g1_decompress(compressed.proof_c).map_err(|_| Groth16Error::InvalidG1)?;

    Ok((proof_a_neg, proof_b, proof_c))
}

/// Verifies a Groth16 proof with the given public inputs and verifying key.
///
/// This is the shared verification function used by all proof types.
/// Public inputs must already be assembled in the correct circuit order.
///
/// # Type Parameters
/// * `N` - Number of public inputs (e.g., 30 for transaction, 7/19/67 for nullifier batch)
///
/// # Arguments
/// * `compressed` - The compressed proof elements
/// * `public_inputs` - Public inputs in circuit order (big-endian)
/// * `vk` - The verification key for the circuit
///
/// # Returns
/// * `Ok(true)` if the proof is valid
/// * `Err(Groth16Error)` if verification fails
#[inline(never)]
pub fn verify_groth16<const N: usize>(
    compressed: &CompressedGroth16Proof,
    public_inputs: &[[u8; 32]; N],
    vk: &Groth16Verifyingkey,
) -> Result<bool, Groth16Error> {
    let (proof_a, proof_b, proof_c) = decompress_and_negate_proof(compressed)?;

    let mut verifier = Groth16Verifier::new(&proof_a, &proof_b, &proof_c, public_inputs, vk)?;

    verifier.verify()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_negate_y_zero() {
        let zero = [0u8; 32];
        let result = negate_y(&zero);
        assert_eq!(result, zero, "negate(0) should be 0");
    }

    #[test]
    fn test_negate_y_one() {
        let mut one = [0u8; 32];
        one[31] = 1;
        let result = negate_y(&one);

        // -1 mod p = p - 1
        let p = BigUint::from_bytes_be(&BN254_FIELD_MODULUS);
        let expected = &p - BigUint::from(1u32);
        let expected_bytes = expected.to_bytes_be();
        let mut expected_arr = [0u8; 32];
        expected_arr[32 - expected_bytes.len()..].copy_from_slice(&expected_bytes);
        assert_eq!(result, expected_arr, "negate(1) should be p-1");
    }

    #[test]
    fn test_negate_y_double_negation() {
        // Double negation should return original
        let mut y = [0u8; 32];
        y[31] = 42;
        let neg_y = negate_y(&y);
        let double_neg = negate_y(&neg_y);
        assert_eq!(double_neg, y, "negate(negate(y)) should equal y");
    }

    #[test]
    fn test_is_less_than_field_size_zero() {
        let zero = [0u8; 32];
        assert!(
            is_less_than_bn254_field_size_be(&zero),
            "0 should be < Fr modulus"
        );
    }

    #[test]
    fn test_is_less_than_field_size_one() {
        let mut one = [0u8; 32];
        one[31] = 1;
        assert!(
            is_less_than_bn254_field_size_be(&one),
            "1 should be < Fr modulus"
        );
    }

    #[test]
    fn test_is_less_than_field_size_at_modulus() {
        // Fr modulus should NOT be valid (must be strictly less)
        let fr_modulus: BigUint = ark_bn254::Fr::MODULUS.into();
        let bytes = fr_modulus.to_bytes_be();
        let mut arr = [0u8; 32];
        arr[32 - bytes.len()..].copy_from_slice(&bytes);
        assert!(
            !is_less_than_bn254_field_size_be(&arr),
            "Fr modulus should NOT be < Fr modulus"
        );
    }

    #[test]
    fn test_is_less_than_field_size_above_modulus() {
        // Fr modulus + 1 should NOT be valid
        let fr_modulus: BigUint = ark_bn254::Fr::MODULUS.into();
        let above = &fr_modulus + BigUint::from(1u32);
        let bytes = above.to_bytes_be();
        let mut arr = [0u8; 32];
        arr[32 - bytes.len()..].copy_from_slice(&bytes);
        assert!(
            !is_less_than_bn254_field_size_be(&arr),
            "Fr modulus + 1 should NOT be < Fr modulus"
        );
    }
}
