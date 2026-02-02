use std::process::Command;

fn main() {
    // Only build for tests
    if std::env::var("CARGO_CFG_TEST").is_ok() {
        println!("cargo:rerun-if-changed=src/");

        // Build the program using cargo build-sbf
        let output = Command::new("cargo")
            .args(["build-sbf", "--manifest-path", "Cargo.toml"])
            .output()
            .expect("Failed to build SBF program");

        if !output.status.success() {
            eprintln!("Failed to build SBF program:");
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }
    }
}
