pragma circom 2.0.0;

// =============================================================================
// ONE-HOT SELECTOR VALIDATION
// =============================================================================
// Templates for validating one-hot encoded selectors used in multi-asset routing.
//
// Constraint Costs:
//   AssertBool:       1
//   OneHotValidator:  3n + 2 (where n = array size)
// =============================================================================

// =============================================================================
// AssertBool
// =============================================================================
// Constrains a signal to be boolean (0 or 1).
//
// The constraint x * (x - 1) === 0 is satisfied only when x ∈ {0, 1}
// because the only roots of the polynomial x² - x = 0 are x = 0 and x = 1.
//
// Proof:
//   x² - x = 0
//   x(x - 1) = 0
//   x = 0 or x = 1
//
// Constraints: 1
//
template AssertBool() {
    signal input in;
    in * (in - 1) === 0;
}

// =============================================================================
// OneHotValidator
// =============================================================================
// Validates a one-hot selector with enabled-mask and dot-product binding.
//
// A one-hot selector is a binary array where at most one bit is set to 1.
// This template enforces four properties:
//
//   1. Binary:     bits[i] ∈ {0, 1} for all i
//   2. Enabled:    bits[i] = 1 ⟹ enabled[i] = 1 (can only select enabled slots)
//   3. Count:      Σ bits[i] = expectedSum (typically 0 or 1)
//   4. Binding:    Σ bits[i] * values[i] = expectedDot (selected value matches)
//
// The dot-product binding (property 4) is the key insight: if exactly one bit
// is set, the dot product equals the value at that position. By setting
// expectedDot to the item's assetId and values to the slot assetIds, we prove
// the selected slot has a matching assetId without revealing which slot.
//
// Parameters:
//   n - Array size (number of slots to select from)
//
// Inputs:
//   bits[n]      - One-hot selector (private witness)
//   enabled[n]   - Which slots are valid targets
//   values[n]    - Values to dot-product against (typically assetIds)
//   expectedSum  - Expected number of bits set (0 if disabled, 1 if enabled)
//   expectedDot  - Expected dot product (the item's assetId, or 0 if disabled)
//
// Constraints: 3n + 2
//   - n constraints for binary check (property 1)
//   - n intermediate signals + n constraints for enabled check (property 2)
//   - n intermediate signals for dot products
//   - 1 constraint for sum check (property 3)
//   - 1 constraint for dot product check (property 4)
//
template OneHotValidator(n) {
    signal input bits[n];
    signal input enabled[n];
    signal input values[n];
    signal input expectedSum;
    signal input expectedDot;

    signal enabledProducts[n];
    signal dotProducts[n];

    var sum = 0;
    var dot = 0;

    for (var i = 0; i < n; i++) {
        // Property 1: Each bit is binary {0, 1}
        // The constraint x * (x - 1) === 0 is satisfied only when x = 0 or x = 1
        // because the only roots of x² - x = 0 are x ∈ {0, 1}
        bits[i] * (bits[i] - 1) === 0;

        // Property 2: Can only select enabled slots
        // If bits[i] = 1, then enabled[i] must be 1
        // Equivalently: bits[i] * (1 - enabled[i]) = 0
        enabledProducts[i] <== bits[i] * (1 - enabled[i]);
        enabledProducts[i] === 0;

        // Accumulate for properties 3 and 4
        dotProducts[i] <== bits[i] * values[i];
        sum += bits[i];
        dot += dotProducts[i];
    }

    // Property 3: Correct number of bits set
    sum === expectedSum;

    // Property 4: Dot product matches expected value
    dot === expectedDot;
}
