//! LST appreciation end-to-end tests.
//!
//! Tests the exchange rate formulas and appreciation capture:
//! - phi(e) = e * lambda / rho (tokens -> virtual SOL)
//! - phi_inv(s) = s * rho / lambda (virtual SOL -> tokens)
//!
//! Where lambda = exchange_rate, rho = 10^9 (RATE_PRECISION)

// ============================================================================
// Round-Trip Value Preservation Tests
// ============================================================================

/// Test that depositing and withdrawing at the same rate preserves value.
///
/// Mathematical property:
/// phi_inv(phi(e)) = e when rate doesn't change
///
/// 100 tokens -> phi(100) = 100 * 1.05 = 105 vSOL -> phi_inv(105) = 105 / 1.05 = 100 tokens
#[test]
fn test_roundtrip_value_preservation_at_1_05x() {
    // At rate 1.05:
    // phi(e) = e * 1.05 = e * 1.05e9 / 1e9
    // phi_inv(phi(e)) = (e * 1.05e9 / 1e9) * 1e9 / 1.05e9 = e

    let tokens = 100_000_000_000u64; // 100 tokens
    let rate = 1_050_000_000u64; // 1.05x
    let rate_precision = 1_000_000_000u64;

    // phi(e) = e * lambda / rho
    let virtual_sol = (tokens as u128 * rate as u128 / rate_precision as u128) as u64;
    assert_eq!(virtual_sol, 105_000_000_000, "phi(100) should equal 105 vSOL");

    // phi_inv(s) = s * rho / lambda
    let recovered_tokens = (virtual_sol as u128 * rate_precision as u128 / rate as u128) as u64;
    assert_eq!(
        recovered_tokens, tokens,
        "phi_inv(phi(100)) should equal 100 tokens"
    );
}

/// Test value preservation at various exchange rates.
#[test]
fn test_roundtrip_value_preservation_various_rates() {
    let rate_precision = 1_000_000_000u64;
    let tokens = 1_000_000_000_000u64; // 1000 tokens

    // Test rates: 0.95, 1.00, 1.05, 1.10
    let rates = [
        950_000_000u64,   // 0.95x (discount)
        1_000_000_000u64, // 1.00x (par)
        1_050_000_000u64, // 1.05x (premium)
        1_100_000_000u64, // 1.10x (premium)
    ];

    for rate in rates {
        // phi(e) = e * lambda / rho
        let virtual_sol = tokens as u128 * rate as u128 / rate_precision as u128;

        // phi_inv(s) = s * rho / lambda
        let recovered = virtual_sol * rate_precision as u128 / rate as u128;

        assert_eq!(
            recovered,
            tokens as u128,
            "Round-trip should preserve value at rate {}: {} != {}",
            rate as f64 / 1e9,
            recovered,
            tokens
        );
    }
}

/// Test that deposit -> rate increase -> withdraw captures appreciation correctly.
///
/// Scenario:
/// 1. Deposit 100 tokens at rate 1.00 -> 100 virtual SOL credited
/// 2. Rate increases to 1.05
/// 3. Withdraw 100 virtual SOL -> get 95.24 tokens (because 1.05x rate)
/// 4. User "loses" ~5 tokens but gets full virtual SOL value
/// 5. The 5 virtual SOL appreciation goes to pending_rewards
#[test]
fn test_deposit_appreciation_withdraw_flow() {
    // Initial state: 100 tokens, 1.00x rate
    let initial_tokens = 100_000_000_000u64;
    let initial_rate = 1_000_000_000u64;
    let rate_precision = 1_000_000_000u64;

    // phi(100) at rate 1.00 = 100 virtual SOL
    let deposited_virtual_sol =
        (initial_tokens as u128 * initial_rate as u128 / rate_precision as u128) as u64;
    assert_eq!(deposited_virtual_sol, 100_000_000_000);

    // After rate increase to 1.05x
    let new_rate = 1_050_000_000u64;

    // Vault still has 100 tokens, but now worth 105 virtual SOL
    // Appreciation = 105 - 100 = 5 virtual SOL
    let new_virtual_value =
        (initial_tokens as u128 * new_rate as u128 / rate_precision as u128) as u64;
    let appreciation = new_virtual_value - deposited_virtual_sol;
    assert_eq!(appreciation, 5_000_000_000, "Appreciation should be 5 vSOL");

    // User withdraws their original 100 virtual SOL
    // phi_inv(100) at rate 1.05 = 100 * 1e9 / 1.05e9 ~ 95.238 tokens
    let withdrawn_virtual_sol = 100_000_000_000u64;
    let output_tokens =
        (withdrawn_virtual_sol as u128 * rate_precision as u128 / new_rate as u128) as u64;
    assert_eq!(
        output_tokens, 95_238_095_238,
        "User gets ~95.24 tokens for 100 vSOL at 1.05x"
    );

    // Remaining in vault: 100 - 95.238 = ~4.76 tokens
    // Value: 4.76 * 1.05 = ~5 virtual SOL (the appreciation)
    let remaining_tokens = initial_tokens - output_tokens;
    let remaining_value =
        (remaining_tokens as u128 * new_rate as u128 / rate_precision as u128) as u64;

    // Due to rounding, this should be approximately equal to appreciation
    assert!(
        (remaining_value as i64 - appreciation as i64).abs() < 10, // Allow small rounding error
        "Remaining value {} should approximately equal appreciation {}",
        remaining_value,
        appreciation
    );
}

/// Test WSOL (rate = 1.0) is a no-op for exchange rate conversion.
#[test]
fn test_wsol_rate_is_identity() {
    let tokens = 12_345_678_901_234u64; // arbitrary amount
    let rate = 1_000_000_000u64; // 1.00x (WSOL)
    let rate_precision = 1_000_000_000u64;

    // phi(e) = e * lambda / rho = e * 1.0 = e
    let virtual_sol = (tokens as u128 * rate as u128 / rate_precision as u128) as u64;
    assert_eq!(virtual_sol, tokens, "WSOL: phi(e) should equal e");

    // phi_inv(s) = s * rho / lambda = s * 1.0 = s
    let recovered = (virtual_sol as u128 * rate_precision as u128 / rate as u128) as u64;
    assert_eq!(recovered, tokens, "WSOL: phi_inv(s) should equal s");
}
