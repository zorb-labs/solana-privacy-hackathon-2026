//! CLI wrapper for panchor-idl-gen library

use anyhow::Result;
use clap::Parser;
use panchor_idl_gen::{IdlGenOptions, generate_idl_to_file};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "panchor-idl-gen")]
#[command(about = "Generate IDL from Panchor-based Solana program source code")]
struct Args {
    #[arg(short, long, default_value = "src")]
    source: PathBuf,
    #[arg(short, long, default_value = "idl.json")]
    output: PathBuf,
    #[arg(short, long)]
    name: Option<String>,
    #[arg(short, long)]
    version: Option<String>,
    #[arg(long)]
    description: Option<String>,
    /// Additional features to pass to cargo test (e.g., "devnet" or "mainnet")
    #[arg(short, long)]
    features: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let options = IdlGenOptions {
        name: args.name,
        version: args.version,
        description: args.description,
        features: args.features,
    };

    generate_idl_to_file(&args.source, &args.output, options)
}
