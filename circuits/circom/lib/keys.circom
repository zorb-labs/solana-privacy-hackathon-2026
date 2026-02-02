pragma circom 2.0.0;

// =============================================================================
// KEY DERIVATION
// =============================================================================
// Full key derivation chain from secret keys to public key:
//   (ask, nsk) -> ak, nk -> ivk -> pk
//
// Key Hierarchy:
//   ask (spend authorizing secret) -> ak = Poseidon(ask)
//   nsk (nullifier secret)         -> nk = Poseidon(nsk)
//   (ak, nk)                       -> ivk = Poseidon(ak, nk)
//   ivk                            -> pk = Poseidon(ivk)
//
// Constraint Costs (Poseidon(n) ≈ 168 + 75*n):
//   DeriveAk:   ~243 (Poseidon(1))
//   DeriveNk:   ~243 (Poseidon(1))
//   DeriveIvk:  ~318 (Poseidon(2))
//   DerivePk:   ~243 (Poseidon(1))
//   DeriveKeys: ~1,047 (all four combined)
// =============================================================================

include "circomlib/circuits/poseidon.circom";

// Derive authorization key from spend authorizing secret
// ak = Poseidon(ask)
template DeriveAk() {
    signal input ask;
    signal output ak;

    component hasher = Poseidon(1);
    hasher.inputs[0] <== ask;
    ak <== hasher.out;
}

// Derive nullifier key from nullifier secret
// nk = Poseidon(nsk)
template DeriveNk() {
    signal input nsk;
    signal output nk;

    component hasher = Poseidon(1);
    hasher.inputs[0] <== nsk;
    nk <== hasher.out;
}

// Derive incoming viewing key from (ak, nk)
// ivk = Poseidon(ak, nk)
template DeriveIvk() {
    signal input ak;
    signal input nk;
    signal output ivk;

    component hasher = Poseidon(2);
    hasher.inputs[0] <== ak;
    hasher.inputs[1] <== nk;
    ivk <== hasher.out;
}

// Derive public key from incoming viewing key
// pk = Poseidon(ivk)
template DerivePk() {
    signal input ivk;
    signal output pk;

    component hasher = Poseidon(1);
    hasher.inputs[0] <== ivk;
    pk <== hasher.out;
}

// Full key derivation chain from secret keys to public key
// (ask, nsk) -> ak, nk -> ivk -> pk
//
// This is the main template used in the transaction circuit.
// It exposes all intermediate keys for use in commitment and nullifier computation.
template DeriveKeys() {
    signal input ask;   // Spend authorizing secret key
    signal input nsk;   // Nullifier secret key

    signal output ak;   // Authorization key (public)
    signal output nk;   // Nullifier deriving key (public)
    signal output ivk;  // Incoming viewing key
    signal output pk;   // Public key (used in note commitments)

    component deriveAk = DeriveAk();
    deriveAk.ask <== ask;
    ak <== deriveAk.ak;

    component deriveNk = DeriveNk();
    deriveNk.nsk <== nsk;
    nk <== deriveNk.nk;

    component deriveIvk = DeriveIvk();
    deriveIvk.ak <== ak;
    deriveIvk.nk <== nk;
    ivk <== deriveIvk.ivk;

    component derivePk = DerivePk();
    derivePk.ivk <== ivk;
    pk <== derivePk.pk;
}
