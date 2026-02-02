# pinocchio-ext

Extension traits and utilities for [Pinocchio](https://github.com/anza-xyz/pinocchio) - a lightweight Solana program framework.

## Features

- **`AccountLoaders`** - Zero-copy account loading with discriminator validation
- **`Discriminator`** - Account type identification trait

## Usage

```rust
use pinocchio_ext::prelude::*;

// Chain multiple assertions
signer.assert_signer()?.assert_writable()?;

// Load account data with AccountLoader
mine_info.load::<Mine>()?.inspect(|mine| {
    println!("Balance: {}", mine.treasury.balance);
})?;

// Mutable access
miner_info.load::<Miner>()?.inspect_mut(|miner| {
    miner.balance += amount;
})?;

// Validate PDA seeds
stake_info.assert_seeds(&[b"stake", mine.key().as_ref()], program_id)?;
```

## AccountLoaders Methods

| Method | Description |
|--------|-------------|
| `load::<T>()` | Load account as `AccountLoader<T>` with ownership + discriminator check |

## AccountLoader Methods

| Method | Description |
|--------|-------------|
| `load()` | Get immutable reference to account data |
| `load_mut()` | Get mutable reference to account data |
| `inspect(f)` | Access data immutably with automatic borrow management |
| `inspect_mut(f)` | Access data mutably with automatic borrow management |
| `map(f)` | Access data and return a value |
| `map_mut(f)` | Access data mutably and return a value |
| `try_inspect(f)` | Like `inspect` but closure can return error |
| `try_map(f)` | Like `map` but closure can return error |
| `try_inspect_mut(f)` | Like `inspect_mut` but closure can return error |
| `try_map_mut(f)` | Like `map_mut` but closure can return error |

## AccountAssertions Methods

| Method | Description |
|--------|-------------|
| `assert_signer()` | Verify account is a signer |
| `assert_writable()` | Verify account is writable |
| `assert_owner(pubkey)` | Verify account owner matches |
| `assert_key(pubkey)` | Verify account key matches |
| `assert_empty()` | Verify account is uninitialized |
| `assert_not_empty()` | Verify account is initialized |
| `assert_seeds(seeds, program_id)` | Verify account matches derived PDA |

## Discriminator Trait

Implement `Discriminator` for your account types to enable checked deserialization:

```rust
use pinocchio_ext::Discriminator;

impl Discriminator for MyAccount {
    const DISCRIMINATOR: u64 = 1;
}
```
