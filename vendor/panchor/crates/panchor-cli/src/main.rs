//! Panchor CLI - Build tool for Panchor-based Solana programs
//!
//! Commands:
//! - `panchor build` - Build all programs and generate IDLs
//! - `panchor idl build` - Generate IDLs only
//! - `panchor expand` - Expand macros and write to target/expand/

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "panchor")]
#[command(about = "Build tool for Panchor-based Solana programs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build all programs and generate IDLs
    Build {
        /// Skip IDL generation
        #[arg(long)]
        skip_idl: bool,
    },
    /// IDL-related commands
    Idl {
        #[command(subcommand)]
        command: IdlCommands,
    },
    /// Expand macros for all programs and write to target/expand/
    Expand,
    /// Configure standard features in all program Cargo.toml files
    SetFeatures,
}

#[derive(Subcommand)]
enum IdlCommands {
    /// Generate IDLs for all programs
    Build {
        /// Additional features to pass to cargo test (e.g., "devnet" or "mainnet")
        #[arg(short = 'F', long)]
        features: Option<String>,
    },
}

#[derive(Deserialize)]
struct CargoToml {
    package: Option<Package>,
    lib: Option<Lib>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
}

#[derive(Deserialize)]
struct Lib {
    name: Option<String>,
}

/// Information about a program in the workspace
struct ProgramInfo {
    /// The package name (from Cargo.toml [package].name)
    package_name: String,
    /// The library name (used for .so file and IDL)
    lib_name: String,
    /// Path to the program's Cargo.toml
    manifest_path: PathBuf,
    /// Path to the program's source directory
    source_dir: PathBuf,
    /// Whether this program has the idl-build feature
    has_idl_build: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build { skip_idl } => {
            build_programs()?;
            if !skip_idl {
                build_idls(None)?;
            }
        }
        Commands::Idl { command } => match command {
            IdlCommands::Build { features } => {
                build_idls(features.as_deref())?;
            }
        },
        Commands::Expand => {
            expand_programs()?;
        }
        Commands::SetFeatures => {
            set_features()?;
        }
    }

    Ok(())
}

/// Validate that a path is within the workspace boundary.
/// Returns true if the path is a descendant of workspace_root (after canonicalization).
fn is_within_workspace(path: &Path, workspace_root: &Path) -> bool {
    // Canonicalize both paths to resolve symlinks
    let Ok(canonical_path) = path.canonicalize() else {
        return false;
    };
    let Ok(canonical_root) = workspace_root.canonicalize() else {
        return false;
    };
    canonical_path.starts_with(&canonical_root)
}

/// Find the workspace root by looking for Cargo.toml with [workspace]
fn find_workspace_root() -> Result<PathBuf> {
    let mut current = std::env::current_dir()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml)?;
            if content.contains("[workspace]") {
                return Ok(current);
            }
        }

        if !current.pop() {
            anyhow::bail!("Could not find workspace root (Cargo.toml with [workspace])");
        }
    }
}

/// Find all programs in the workspace
fn find_programs(workspace_root: &Path) -> Result<Vec<ProgramInfo>> {
    let programs_dir = workspace_root.join("programs");
    let mut programs = Vec::new();

    if !programs_dir.exists() {
        // Try looking in the workspace for any crate with cdylib
        return find_programs_in_workspace(workspace_root);
    }

    for entry in WalkDir::new(&programs_dir)
        .min_depth(1)
        .max_depth(2)
        .follow_links(false) // Don't follow symlinks to prevent traversal
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_within_workspace(e.path(), workspace_root)) // Validate path boundary
    {
        let cargo_toml = entry.path().join("Cargo.toml");
        if cargo_toml.exists()
            && is_within_workspace(&cargo_toml, workspace_root)
            && let Some(info) = parse_program_info(&cargo_toml)?
        {
            programs.push(info);
        }
    }

    Ok(programs)
}

/// Find programs by scanning the entire workspace for cdylib crates
fn find_programs_in_workspace(workspace_root: &Path) -> Result<Vec<ProgramInfo>> {
    let mut programs = Vec::new();

    for entry in WalkDir::new(workspace_root)
        .min_depth(2)
        .max_depth(4)
        .follow_links(false) // Don't follow symlinks to prevent traversal
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Cargo.toml")
        .filter(|e| is_within_workspace(e.path(), workspace_root)) // Validate path boundary
    {
        if let Some(info) = parse_program_info(entry.path())? {
            programs.push(info);
        }
    }

    Ok(programs)
}

/// Parse a Cargo.toml to extract program info if it's a Solana program (cdylib)
fn parse_program_info(cargo_toml: &Path) -> Result<Option<ProgramInfo>> {
    let content = fs::read_to_string(cargo_toml)?;

    // Check if this is a cdylib (Solana program)
    if !content.contains("cdylib") {
        return Ok(None);
    }

    let parsed: CargoToml = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;

    let package_name = parsed
        .package
        .as_ref()
        .map(|p| p.name.clone())
        .context("Missing [package] section")?;

    // Use lib.name if specified, otherwise use package.name with hyphens replaced
    let lib_name = parsed
        .lib
        .and_then(|l| l.name)
        .unwrap_or_else(|| package_name.replace('-', "_"));

    let manifest_path = cargo_toml.to_path_buf();
    let source_dir = cargo_toml
        .parent()
        .context("Invalid manifest path")?
        .join("src");

    // Check if the program has the idl-build feature
    let has_idl_build = content.contains("idl-build");

    Ok(Some(ProgramInfo {
        package_name,
        lib_name,
        manifest_path,
        source_dir,
        has_idl_build,
    }))
}

/// Build all Solana programs using cargo build-sbf
fn build_programs() -> Result<()> {
    let workspace_root = find_workspace_root()?;
    let programs = find_programs(&workspace_root)?;

    if programs.is_empty() {
        eprintln!("No programs found in workspace");
        return Ok(());
    }

    eprintln!("Building {} program(s)...", programs.len());

    for program in &programs {
        eprintln!("  Building {}...", program.lib_name);

        let status = Command::new("cargo")
            .args(["build-sbf", "--manifest-path"])
            .arg(&program.manifest_path)
            .current_dir(&workspace_root)
            .status()
            .context("Failed to run cargo build-sbf")?;

        if !status.success() {
            anyhow::bail!("Failed to build {}", program.lib_name);
        }
    }

    eprintln!("All programs built successfully");
    Ok(())
}

/// Build IDLs for all programs
fn build_idls(features: Option<&str>) -> Result<()> {
    let workspace_root = find_workspace_root()?;
    let all_programs = find_programs(&workspace_root)?;

    // Filter to only programs with idl-build feature
    let programs: Vec<_> = all_programs.iter().filter(|p| p.has_idl_build).collect();

    let skipped = all_programs.len() - programs.len();

    if programs.is_empty() {
        if skipped > 0 {
            eprintln!(
                "No programs with idl-build feature found ({} program(s) skipped)",
                skipped
            );
        } else {
            eprintln!("No programs found in workspace");
        }
        return Ok(());
    }

    // Create target/idl directory
    let idl_dir = workspace_root.join("target").join("idl");
    fs::create_dir_all(&idl_dir).context("Failed to create target/idl directory")?;

    let feature_str = features.map(|f| format!(" (features: {})", f)).unwrap_or_default();
    if skipped > 0 {
        eprintln!(
            "Generating IDLs for {} program(s){}({} skipped without idl-build)...",
            programs.len(),
            feature_str,
            skipped
        );
    } else {
        eprintln!("Generating IDLs for {} program(s){}...", programs.len(), feature_str);
    }

    // Build options with features if specified
    let options = panchor_idl_gen::IdlGenOptions {
        features: features.map(|s| s.to_string()),
        ..Default::default()
    };

    for program in &programs {
        let idl_path = idl_dir.join(format!("{}.json", program.lib_name));
        eprintln!("  Generating {}...", idl_path.display());

        panchor_idl_gen::generate_idl_to_file(
            &program.source_dir,
            &idl_path,
            options.clone(),
        )
        .with_context(|| format!("Failed to generate IDL for {}", program.lib_name))?;
    }

    eprintln!("All IDLs generated successfully");
    Ok(())
}

/// Expand macros for all programs using cargo-expand
fn expand_programs() -> Result<()> {
    let workspace_root = find_workspace_root()?;
    let programs = find_programs(&workspace_root)?;

    if programs.is_empty() {
        eprintln!("No programs found in workspace");
        return Ok(());
    }

    // Create target/expand directory
    let expand_dir = workspace_root.join("target").join("expand");
    fs::create_dir_all(&expand_dir).context("Failed to create target/expand directory")?;

    eprintln!("Expanding {} program(s)...", programs.len());

    for program in &programs {
        let output_path = expand_dir.join(format!("{}.rs", program.lib_name));
        eprintln!(
            "  Expanding {} -> {}...",
            program.package_name,
            output_path.display()
        );

        let output = Command::new("cargo")
            .args(["expand", "--lib", "--package"])
            .arg(&program.package_name)
            .current_dir(&workspace_root)
            .output()
            .context("Failed to run cargo expand. Is cargo-expand installed? (cargo install cargo-expand)")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to expand {}: {}", program.package_name, stderr);
        }

        fs::write(&output_path, &output.stdout)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;
    }

    eprintln!("All programs expanded successfully");
    Ok(())
}

/// Configure standard features in all program Cargo.toml files
fn set_features() -> Result<()> {
    let workspace_root = find_workspace_root()?;
    let programs = find_programs(&workspace_root)?;

    if programs.is_empty() {
        eprintln!("No programs found in workspace");
        return Ok(());
    }

    eprintln!("Configuring features for {} program(s)...", programs.len());

    for program in &programs {
        eprintln!("  Updating {}...", program.package_name);
        update_program_features(&program.manifest_path)?;
    }

    eprintln!("All program features configured successfully");
    Ok(())
}

/// Update a single program's Cargo.toml with standard features
fn update_program_features(manifest_path: &Path) -> Result<()> {
    use std::io::Write;

    let content = fs::read_to_string(manifest_path)?;
    let mut doc = content
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;

    // Ensure [features] section exists
    if !doc.contains_key("features") {
        doc["features"] = toml_edit::table();
    }

    let features = doc["features"]
        .as_table_mut()
        .context("features must be a table")?;

    // Set up standard features
    // idl-build: enable IDL generation
    if !features.contains_key("idl-build") {
        features["idl-build"] =
            toml_edit::value(toml_edit::Array::from_iter(["panchor/idl-build"]));
    }

    // solana-sdk: enable SDK helpers (for client code)
    if !features.contains_key("solana-sdk") {
        features["solana-sdk"] =
            toml_edit::value(toml_edit::Array::from_iter(["panchor/solana-sdk"]));
    }

    // Write atomically: write to temp file, then rename
    // This prevents file corruption if interrupted mid-write
    let temp_path = manifest_path.with_extension("toml.tmp");
    let mut file = fs::File::create(&temp_path)
        .with_context(|| format!("Failed to create temp file {}", temp_path.display()))?;
    file.write_all(doc.to_string().as_bytes())
        .with_context(|| format!("Failed to write {}", temp_path.display()))?;
    file.sync_all()
        .with_context(|| format!("Failed to sync {}", temp_path.display()))?;
    drop(file);

    fs::rename(&temp_path, manifest_path)
        .with_context(|| format!("Failed to rename {} to {}", temp_path.display(), manifest_path.display()))?;

    Ok(())
}
