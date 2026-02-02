pragma circom 2.0.0;

// =============================================================================
// REWARD COMPUTATION
// =============================================================================
// Templates for calculating accrued yield rewards.
//
// Constraint Costs:
//   DivByAccumulatorScale:  ~74 (Num2Bits(60) + LessThan(64) + multiplication)
//   ComputeReward:          ~78 (DivByAccumulatorScale + arithmetic)
// =============================================================================

include "circomlib/circuits/bitify.circom";
include "circomlib/circuits/comparators.circom";

include "./constants.circom";  // ACCUMULATOR_SCALE()

// Division by ACCUMULATOR_SCALE (1e18) for fixed-point arithmetic
// Proves: dividend = quotient * SCALE + remainder, where 0 <= remainder < SCALE
//
// The prover provides the remainder as a hint. The circuit verifies:
// 1. quotient * SCALE + remainder === dividend
// 2. remainder < SCALE (range check)
template DivByAccumulatorScale() {
    signal input dividend;
    signal input remainder;  // Private hint: dividend mod ACCUMULATOR_SCALE
    signal output out;       // The quotient = floor(dividend / ACCUMULATOR_SCALE)

    var SCALE = ACCUMULATOR_SCALE();

    // Compute quotient (unconstrained computation)
    signal quotient <-- (dividend - remainder) / SCALE;

    // Verify division: quotient * SCALE + remainder === dividend
    signal product <== quotient * SCALE;
    product + remainder === dividend;

    // Range check: remainder must be in [0, SCALE)
    // First, prove remainder fits in 60 bits (ACCUMULATOR_SCALE < 2^60)
    component remainderBits = Num2Bits(60);
    remainderBits.in <== remainder;

    // Then, prove remainder < SCALE exactly
    component remainderLt = LessThan(64);
    remainderLt.in[0] <== remainder;
    remainderLt.in[1] <== SCALE;
    remainderLt.out === 1;

    out <== quotient;
}

// Compute accrued reward for a note
// reward = floor(amount * (globalAccumulator - noteAccumulator) / ACCUMULATOR_SCALE)
//
// The accumulator difference represents the yield earned per unit since the note was created.
// Multiplying by amount and dividing by ACCUMULATOR_SCALE gives the actual reward.
template ComputeReward() {
    signal input amount;
    signal input globalAccumulator;    // Current global reward accumulator
    signal input noteAccumulator;      // Accumulator snapshot when note was created
    signal input remainder;            // Division hint: (amount * diff) mod ACCUMULATOR_SCALE
    signal output reward;
    signal output totalValue;          // amount + reward (for value conservation)

    // Compute accumulator difference
    signal accumulatorDiff <== globalAccumulator - noteAccumulator;

    // Compute unscaled reward: amount * accumulatorDiff
    signal unscaledReward <== amount * accumulatorDiff;

    // Scale down by ACCUMULATOR_SCALE
    component scaler = DivByAccumulatorScale();
    scaler.dividend <== unscaledReward;
    scaler.remainder <== remainder;

    reward <== scaler.out;
    totalValue <== amount + reward;
}
