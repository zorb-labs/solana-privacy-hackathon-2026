//! IDL generator for Panchor-based Solana programs
//!
//! This library generates an IDL (Interface Definition Language) file compatible
//! with Anchor-style tooling by running the program's idl-build tests and
//! parsing the JSON output.

use anchor_lang_idl_spec as anchor;
use anyhow::{Context, Result};
use panchor_idl::{IdlPdaDefinition, PanchorIdl};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Options for IDL generation.
#[derive(Debug, Default, Clone)]
pub struct IdlGenOptions {
    /// Override the program name (defaults to Cargo.toml package name)
    pub name: Option<String>,
    /// Override the program version (defaults to Cargo.toml version)
    pub version: Option<String>,
    /// Override the program description
    pub description: Option<String>,
    /// Additional features to pass to cargo test (e.g., "devnet,mainnet")
    pub features: Option<String>,
}

/// Generate an IDL for a Panchor-based Solana program.
///
/// # Arguments
/// * `source_dir` - Path to the program's source directory (containing lib.rs)
/// * `options` - Optional overrides for name, version, description
///
/// # Returns
/// The generated `PanchorIdl` struct
pub fn generate_idl(source_dir: &Path, options: IdlGenOptions) -> Result<PanchorIdl> {
    let source_dir = if source_dir.is_absolute() {
        source_dir.to_path_buf()
    } else {
        std::env::current_dir()?.join(source_dir)
    };

    eprintln!("Reading metadata from Cargo.toml...");
    let cargo_meta = read_cargo_metadata(&source_dir)?;

    let program_name = options.name.unwrap_or(cargo_meta.name);
    let program_version = options.version.unwrap_or(cargo_meta.version);
    let program_description = options.description.or(cargo_meta.description);

    eprintln!("Program: {} v{}", program_name, program_version);

    eprintln!("Running IDL build tests...");
    let build_output = run_idl_build_tests(&source_dir, options.features.as_deref())?;

    // Use program ID from test output if available, otherwise fall back to source parsing
    let program_address = if let Some(id) = build_output.program_id {
        eprintln!("Found program ID from tests: {}", id);
        id
    } else {
        eprintln!("Searching for program ID in: {}", source_dir.display());
        let program_id = find_program_id(&source_dir)?;
        let addr = pubkey_to_base58(&program_id);
        eprintln!("Found program ID: {}", addr);
        addr
    };

    eprintln!(
        "Extracted {} instructions, {} accounts, {} types, {} events, {} errors, {} constants, {} pdas",
        build_output.instructions.len(),
        build_output.accounts.len(),
        build_output.types.len(),
        build_output.events.len(),
        build_output.errors.len(),
        build_output.constants.len(),
        build_output.pdas.len()
    );

    // Build alias map from type aliases (e.g., Bps -> u16, Numeric -> u128)
    let aliases = build_alias_map(&build_output.types);
    if !aliases.is_empty() {
        eprintln!("Found {} type aliases", aliases.len());
    }

    // Log excluded types (instruction data structs)
    if !build_output.excluded_types.is_empty() {
        eprintln!(
            "Excluding {} instruction data types from types array",
            build_output.excluded_types.len()
        );
    }

    // Apply alias substitution to instruction args
    let mut instructions = build_output.instructions;
    for inst in &mut instructions {
        substitute_aliases_in_fields(&mut inst.args, &aliases);
    }
    instructions.sort_by(|a, b| a.discriminator.cmp(&b.discriminator));

    // Filter out alias types and instruction data types, apply substitution to remaining type fields
    let excluded_set: std::collections::HashSet<&str> = build_output
        .excluded_types
        .iter()
        .map(|s| s.as_str())
        .collect();
    let mut types: Vec<anchor::IdlTypeDef> = build_output
        .types
        .into_iter()
        .filter(|t| !matches!(t.ty, anchor::IdlTypeDefTy::Type { .. }))
        .filter(|t| !excluded_set.contains(t.name.as_str()))
        .collect();

    for type_def in &mut types {
        if let anchor::IdlTypeDefTy::Struct {
            fields: Some(anchor::IdlDefinedFields::Named(fs)),
        } = &mut type_def.ty
        {
            substitute_aliases_in_fields(fs, &aliases);
        }
    }

    Ok(PanchorIdl {
        address: program_address,
        metadata: anchor::IdlMetadata {
            name: program_name,
            version: program_version,
            spec: "0.1.0".to_string(),
            description: program_description,
            repository: None,
            dependencies: vec![],
            contact: None,
            deployments: None,
        },
        docs: vec![],
        instructions,
        accounts: build_output.accounts,
        events: build_output.events,
        errors: build_output.errors,
        types,
        constants: build_output.constants,
        pdas: build_output.pdas,
    })
}

/// Generate an IDL and write it to a file.
pub fn generate_idl_to_file(
    source_dir: &Path,
    output_path: &Path,
    options: IdlGenOptions,
) -> Result<()> {
    let idl = generate_idl(source_dir, options)?;
    let json = serde_json::to_string_pretty(&idl)?;
    fs::write(output_path, json)?;
    eprintln!("IDL written to: {}", output_path.display());
    Ok(())
}

// ============================================================================
// Internal implementation
// ============================================================================

struct CargoMetadata {
    name: String,
    version: String,
    description: Option<String>,
}

fn find_crate_root(source_dir: &Path) -> Result<std::path::PathBuf> {
    let mut crate_root = source_dir.to_path_buf();
    while !crate_root.join("Cargo.toml").exists() {
        if !crate_root.pop() {
            anyhow::bail!(
                "Could not find Cargo.toml starting from {}",
                source_dir.display()
            );
        }
    }
    Ok(crate_root)
}

fn read_cargo_metadata(source_dir: &Path) -> Result<CargoMetadata> {
    let crate_root = find_crate_root(source_dir)?;
    let cargo_toml = fs::read_to_string(crate_root.join("Cargo.toml"))?;

    // Try to get lib.name first, fall back to package.name
    let lib_name = cargo_toml
        .lines()
        .skip_while(|line| !line.starts_with("[lib]"))
        .find(|line| line.starts_with("name = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|s| s.to_string());

    let package_name = cargo_toml
        .lines()
        .skip_while(|line| !line.starts_with("[package]"))
        .find(|line| line.starts_with("name = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|s| s.to_string());

    let name = lib_name
        .or(package_name)
        .context("Could not find lib.name or package.name in Cargo.toml")?;

    let version = cargo_toml
        .lines()
        .skip_while(|line| !line.starts_with("[package]"))
        .find(|line| line.starts_with("version = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "0.1.0".to_string());

    let description = cargo_toml
        .lines()
        .skip_while(|line| !line.starts_with("[package]"))
        .find(|line| line.starts_with("description = "))
        .and_then(|line| line.split('"').nth(1))
        .map(|s| s.to_string());

    Ok(CargoMetadata {
        name,
        version,
        description,
    })
}

fn find_program_id(source_dir: &Path) -> Result<[u8; 32]> {
    // Patterns to search for program ID declaration
    let patterns = ["declare_id!(\"", "pinocchio_pubkey::declare_id!(\""];

    for entry in WalkDir::new(source_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let content = fs::read_to_string(entry.path())?;
        for pattern in &patterns {
            if let Some(start) = content.find(pattern) {
                let start = start + pattern.len();
                if let Some(end) = content[start..].find("\")") {
                    let id_str = &content[start..start + end];
                    return bs58_decode(id_str)
                        .with_context(|| format!("Invalid program ID: {}", id_str));
                }
            }
        }
    }
    anyhow::bail!("Could not find declare_id! in source files")
}

fn pubkey_to_base58(key: &[u8; 32]) -> String {
    panchor_idl::pubkey_to_base58(key)
}

fn bs58_decode(s: &str) -> Result<[u8; 32]> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut result = vec![0u8; 32];
    let mut digits: Vec<u8> = Vec::new();

    for c in s.chars() {
        let value = ALPHABET
            .iter()
            .position(|&x| x == c as u8)
            .context("Invalid base58 character")?;

        let mut carry = value;
        for digit in &mut digits {
            carry += (*digit as usize) * 58;
            *digit = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            digits.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }

    let leading_ones = s.chars().take_while(|&c| c == '1').count();
    digits.extend(std::iter::repeat_n(0, leading_ones));

    digits.reverse();
    if digits.len() > 32 {
        anyhow::bail!("Decoded value too large for pubkey");
    }

    let start = 32 - digits.len();
    result[start..].copy_from_slice(&digits);
    Ok(result.try_into().unwrap())
}

struct IdlBuildOutput {
    program_id: Option<String>,
    instructions: Vec<anchor::IdlInstruction>,
    accounts: Vec<anchor::IdlAccount>,
    types: Vec<anchor::IdlTypeDef>,
    events: Vec<anchor::IdlEvent>,
    errors: Vec<anchor::IdlErrorCode>,
    constants: Vec<anchor::IdlConst>,
    pdas: Vec<IdlPdaDefinition>,
    excluded_types: Vec<String>,
}

fn run_idl_build_tests(source_dir: &Path, extra_features: Option<&str>) -> Result<IdlBuildOutput> {
    use std::process::Command;

    let crate_root = find_crate_root(source_dir)?;
    eprintln!("Running IDL build tests in: {}", crate_root.display());

    // Build the features string: always include idl-build, optionally add extra features
    let features = match extra_features {
        Some(f) => format!("idl-build,{}", f),
        None => "idl-build".to_string(),
    };

    eprintln!("Features: {}", features);

    let output = Command::new("cargo")
        .args([
            "test",
            "--features",
            &features,
            "__idl_build",
            "--",
            "--test-threads=1",
            "--nocapture",
        ])
        .current_dir(&crate_root)
        .env("RUSTFLAGS", "-A warnings")
        .output()
        .context("Failed to run cargo test")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if std::env::var("PANCHOR_IDL_DEBUG").is_ok() {
        eprintln!("cargo test stdout:\n{}", stdout);
        eprintln!("cargo test stderr:\n{}", stderr);
    }

    Ok(IdlBuildOutput {
        program_id: parse_program_id_from_output(&stdout),
        instructions: parse_instructions_from_output(&stdout)?,
        accounts: parse_accounts_from_output(&stdout),
        types: parse_types_from_output(&stdout),
        events: parse_events_from_output(&stdout),
        errors: parse_errors_from_output(&stdout),
        constants: parse_constants_from_output(&stdout),
        pdas: parse_pdas_from_output(&stdout),
        excluded_types: parse_excluded_types_from_output(&stdout),
    })
}

fn parse_program_id_from_output(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        if line.contains("--- IDL program_id ") && line.ends_with(" ---") {
            let start = line.find("--- IDL program_id ")? + "--- IDL program_id ".len();
            let end = line.rfind(" ---")?;
            if start < end {
                return Some(line[start..end].to_string());
            }
        }
    }
    None
}

fn parse_excluded_types_from_output(stdout: &str) -> Vec<String> {
    let mut excluded = Vec::new();
    for line in stdout.lines() {
        if line.contains("--- IDL exclude_type ")
            && line.ends_with(" ---")
            && let Some(start) = line.find("--- IDL exclude_type ")
        {
            let start = start + "--- IDL exclude_type ".len();
            if let Some(end) = line.rfind(" ---")
                && start < end
            {
                excluded.push(line[start..end].to_string());
            }
        }
    }
    excluded
}

fn parse_instructions_from_output(stdout: &str) -> Result<Vec<anchor::IdlInstruction>> {
    let mut in_json_block = false;
    let mut json_lines: Vec<&str> = Vec::new();

    for line in stdout.lines() {
        if line.contains("--- IDL begin instructions ---") {
            in_json_block = true;
            continue;
        }
        if line.contains("--- IDL end instructions ---") {
            in_json_block = false;
            continue;
        }
        if in_json_block {
            json_lines.push(line);
        }
    }

    if json_lines.is_empty() {
        anyhow::bail!("No IDL instructions found in test output.");
    }

    let json_str = json_lines.join("\n");
    serde_json::from_str(&json_str).context("Failed to parse IDL instructions JSON")
}

fn parse_accounts_from_output(stdout: &str) -> Vec<anchor::IdlAccount> {
    let mut accounts: Vec<anchor::IdlAccount> = Vec::new();
    let mut current_json_lines: Vec<&str> = Vec::new();
    let mut in_account_block = false;

    for line in stdout.lines() {
        if line.contains("--- IDL account ") && line.contains(" ---") {
            in_account_block = true;
            current_json_lines.clear();
            continue;
        }
        if line.contains("--- end ---") && in_account_block {
            in_account_block = false;
            if !current_json_lines.is_empty() {
                let json_str = current_json_lines.join("\n");
                if let Ok(account) = serde_json::from_str(&json_str) {
                    accounts.push(account);
                }
            }
            continue;
        }
        if in_account_block {
            current_json_lines.push(line);
        }
    }

    accounts.sort_by(|a, b| a.discriminator.cmp(&b.discriminator));
    accounts
}

fn parse_types_from_output(stdout: &str) -> Vec<anchor::IdlTypeDef> {
    let mut types: Vec<anchor::IdlTypeDef> = Vec::new();
    let mut current_json_lines: Vec<&str> = Vec::new();
    let mut in_type_block = false;

    for line in stdout.lines() {
        if line.contains("--- IDL type ") && line.contains(" ---") {
            in_type_block = true;
            current_json_lines.clear();
            continue;
        }
        if line.contains("--- end ---") && in_type_block {
            in_type_block = false;
            if !current_json_lines.is_empty() {
                let json_str = current_json_lines.join("\n");
                if let Ok(type_def) = serde_json::from_str(&json_str) {
                    types.push(type_def);
                } else {
                    eprintln!("Warning: Failed to parse type JSON: {}", json_str);
                }
            }
            continue;
        }
        if in_type_block {
            current_json_lines.push(line);
        }
    }

    types.sort_by(|a, b| a.name.cmp(&b.name));
    types
}

fn parse_events_from_output(stdout: &str) -> Vec<anchor::IdlEvent> {
    let mut events: Vec<anchor::IdlEvent> = Vec::new();
    let mut current_json_lines: Vec<&str> = Vec::new();
    let mut in_event_block = false;

    for line in stdout.lines() {
        if line.contains("--- IDL event ") && line.contains(" ---") {
            in_event_block = true;
            current_json_lines.clear();
            continue;
        }
        if line.contains("--- end ---") && in_event_block {
            in_event_block = false;
            if !current_json_lines.is_empty() {
                let json_str = current_json_lines.join("\n");
                if let Ok(event) = serde_json::from_str(&json_str) {
                    events.push(event);
                }
            }
            continue;
        }
        if in_event_block {
            current_json_lines.push(line);
        }
    }

    events.sort_by(|a, b| a.discriminator.cmp(&b.discriminator));
    events
}

fn parse_errors_from_output(stdout: &str) -> Vec<anchor::IdlErrorCode> {
    let mut in_json_block = false;
    let mut json_lines: Vec<&str> = Vec::new();

    for line in stdout.lines() {
        if line.contains("--- IDL begin errors ---") {
            in_json_block = true;
            continue;
        }
        if line.contains("--- IDL end errors ---") {
            in_json_block = false;
            continue;
        }
        if in_json_block {
            json_lines.push(line);
        }
    }

    if json_lines.is_empty() {
        return Vec::new();
    }

    let json_str = json_lines.join("\n");
    match serde_json::from_str::<Vec<anchor::IdlErrorCode>>(&json_str) {
        Ok(mut errors) => {
            errors.sort_by_key(|e| e.code);
            errors
        }
        Err(_) => Vec::new(),
    }
}

fn parse_constants_from_output(stdout: &str) -> Vec<anchor::IdlConst> {
    let mut constants: Vec<anchor::IdlConst> = Vec::new();
    let mut current_json_lines: Vec<&str> = Vec::new();
    let mut in_constant_block = false;

    for line in stdout.lines() {
        if line.contains("--- IDL constant ") && line.contains(" ---") {
            in_constant_block = true;
            current_json_lines.clear();
            continue;
        }
        if line.contains("--- end ---") && in_constant_block {
            in_constant_block = false;
            if !current_json_lines.is_empty() {
                let json_str = current_json_lines.join("\n");
                if let Ok(constant) = serde_json::from_str(&json_str) {
                    constants.push(constant);
                }
            }
            continue;
        }
        if in_constant_block {
            current_json_lines.push(line);
        }
    }

    constants.sort_by(|a, b| a.name.cmp(&b.name));
    constants
}

fn parse_pdas_from_output(stdout: &str) -> Vec<IdlPdaDefinition> {
    let mut pdas: Vec<IdlPdaDefinition> = Vec::new();
    let mut current_json_lines: Vec<&str> = Vec::new();
    let mut in_pda_block = false;

    for line in stdout.lines() {
        if line.contains("--- IDL pda ") && line.contains(" ---") {
            in_pda_block = true;
            current_json_lines.clear();
            continue;
        }
        if line.contains("--- end ---") && in_pda_block {
            in_pda_block = false;
            if !current_json_lines.is_empty() {
                let json_str = current_json_lines.join("\n");
                if let Ok(pda) = serde_json::from_str(&json_str) {
                    pdas.push(pda);
                } else {
                    eprintln!("Warning: Failed to parse PDA JSON: {}", json_str);
                }
            }
            continue;
        }
        if in_pda_block {
            current_json_lines.push(line);
        }
    }

    pdas.sort_by(|a, b| a.name.cmp(&b.name));
    pdas
}

/// Build a map of type aliases from the types list.
fn build_alias_map(types: &[anchor::IdlTypeDef]) -> HashMap<String, anchor::IdlType> {
    types
        .iter()
        .filter_map(|t| {
            if let anchor::IdlTypeDefTy::Type { alias } = &t.ty {
                Some((t.name.clone(), alias.clone()))
            } else {
                None
            }
        })
        .collect()
}

/// Substitute aliased types with their underlying types.
fn substitute_aliases(
    ty: &anchor::IdlType,
    aliases: &HashMap<String, anchor::IdlType>,
) -> anchor::IdlType {
    match ty {
        anchor::IdlType::Defined { name, generics } => {
            if let Some(alias) = aliases.get(name) {
                alias.clone()
            } else {
                anchor::IdlType::Defined {
                    name: name.clone(),
                    generics: generics.clone(),
                }
            }
        }
        anchor::IdlType::Option(inner) => {
            anchor::IdlType::Option(Box::new(substitute_aliases(inner, aliases)))
        }
        anchor::IdlType::Vec(inner) => {
            anchor::IdlType::Vec(Box::new(substitute_aliases(inner, aliases)))
        }
        anchor::IdlType::Array(inner, len) => {
            anchor::IdlType::Array(Box::new(substitute_aliases(inner, aliases)), len.clone())
        }
        _ => ty.clone(),
    }
}

/// Substitute aliases in all fields.
fn substitute_aliases_in_fields(
    fields: &mut [anchor::IdlField],
    aliases: &HashMap<String, anchor::IdlType>,
) {
    for field in fields {
        field.ty = substitute_aliases(&field.ty, aliases);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pubkey_roundtrip() {
        let original = [
            0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45,
            0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01,
            0x23, 0x45, 0x67, 0x89,
        ];
        let base58 = pubkey_to_base58(&original);
        let decoded = bs58_decode(&base58).unwrap();
        assert_eq!(original, decoded);
    }
}
