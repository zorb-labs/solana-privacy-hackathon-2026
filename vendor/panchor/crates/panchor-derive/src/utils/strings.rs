//! String manipulation utilities for panchor-derive

/// Convert a `PascalCase` or camelCase string to `snake_case`
///
/// # Example
/// ```ignore
/// use panchor_derive::utils::strings::to_snake_case;
///
/// assert_eq!(to_snake_case("Pool"), "pool");
/// assert_eq!(to_snake_case("StakeAccount"), "stake_account");
/// ```
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Convert a `PascalCase` or camelCase string to `SCREAMING_SNAKE_CASE`
///
/// # Example
/// ```ignore
/// use panchor_derive::utils::strings::to_screaming_snake_case;
///
/// assert_eq!(to_screaming_snake_case("Pool"), "POOL");
/// assert_eq!(to_screaming_snake_case("StakeAccount"), "STAKE_ACCOUNT");
/// ```
pub fn to_screaming_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

/// Convert a `snake_case` or kebab-case string to `PascalCase`
///
/// # Example
/// ```ignore
/// use panchor_derive::utils::strings::to_pascal_case;
///
/// assert_eq!(to_pascal_case("my_program"), "MyProgram");
/// assert_eq!(to_pascal_case("stake-account"), "StakeAccount");
/// ```
pub fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("Pool"), "pool");
        assert_eq!(to_snake_case("StakeAccount"), "stake_account");
        assert_eq!(to_snake_case("Miner"), "miner");
        assert_eq!(to_snake_case("GlobalState"), "global_state");
        assert_eq!(to_snake_case("CreatorFees"), "creator_fees");
    }

    #[test]
    fn test_to_screaming_snake_case() {
        assert_eq!(to_screaming_snake_case("Pool"), "POOL");
        assert_eq!(to_screaming_snake_case("StakeAccount"), "STAKE_ACCOUNT");
        assert_eq!(to_screaming_snake_case("ABC"), "A_B_C");
    }

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("my_program"), "MyProgram");
        assert_eq!(to_pascal_case("stake-account"), "StakeAccount");
        assert_eq!(to_pascal_case("simple"), "Simple");
        assert_eq!(to_pascal_case("a_b_c"), "ABC");
    }
}
