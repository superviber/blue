//! Build script for blue-ollama
//!
//! Downloads Ollama binary for the target platform at build time.
//! Set BLUE_OLLAMA_PATH to skip download and use a local binary.
//! Set BLUE_SKIP_OLLAMA_DOWNLOAD=1 to skip download entirely.

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Ollama version to download
const OLLAMA_VERSION: &str = "v0.5.4";

/// Base URL for Ollama releases
const OLLAMA_RELEASE_URL: &str = "https://github.com/ollama/ollama/releases/download";

fn main() {
    println!("cargo:rerun-if-env-changed=BLUE_OLLAMA_PATH");
    println!("cargo:rerun-if-env-changed=BLUE_SKIP_OLLAMA_DOWNLOAD");

    // Skip if BLUE_OLLAMA_PATH is set (user provides their own binary)
    if env::var("BLUE_OLLAMA_PATH").is_ok() {
        println!("cargo:warning=Using BLUE_OLLAMA_PATH, skipping Ollama download");
        return;
    }

    // Skip if explicitly disabled
    if env::var("BLUE_SKIP_OLLAMA_DOWNLOAD").is_ok() {
        println!("cargo:warning=BLUE_SKIP_OLLAMA_DOWNLOAD set, skipping Ollama download");
        return;
    }

    // Get output directory
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = PathBuf::from(&out_dir);

    // Determine target platform
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    let (binary_name, download_url) = match (target_os.as_str(), target_arch.as_str()) {
        ("macos", _) => (
            "ollama",
            format!("{}/{}/ollama-darwin", OLLAMA_RELEASE_URL, OLLAMA_VERSION),
        ),
        ("linux", "x86_64") => (
            "ollama",
            format!(
                "{}/{}/ollama-linux-amd64",
                OLLAMA_RELEASE_URL, OLLAMA_VERSION
            ),
        ),
        ("linux", "aarch64") => (
            "ollama",
            format!(
                "{}/{}/ollama-linux-arm64",
                OLLAMA_RELEASE_URL, OLLAMA_VERSION
            ),
        ),
        ("windows", "x86_64") => (
            "ollama.exe",
            format!(
                "{}/{}/ollama-windows-amd64.exe",
                OLLAMA_RELEASE_URL, OLLAMA_VERSION
            ),
        ),
        _ => {
            println!(
                "cargo:warning=Unsupported platform: {}-{}",
                target_os, target_arch
            );
            println!("cargo:warning=Ollama will need to be installed manually");
            return;
        }
    };

    let binary_path = out_path.join(binary_name);

    // Check if already downloaded
    if binary_path.exists() {
        println!(
            "cargo:warning=Ollama binary already exists at {:?}",
            binary_path
        );
        write_binary_path(&out_path, &binary_path);
        return;
    }

    // Download the binary
    println!(
        "cargo:warning=Downloading Ollama {} for {}-{}",
        OLLAMA_VERSION, target_os, target_arch
    );
    println!("cargo:warning=URL: {}", download_url);

    match download_binary(&download_url, &binary_path) {
        Ok(_) => {
            println!("cargo:warning=Downloaded Ollama to {:?}", binary_path);

            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&binary_path) {
                    let mut perms = metadata.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&binary_path, perms);
                }
            }

            write_binary_path(&out_path, &binary_path);
        }
        Err(e) => {
            println!("cargo:warning=Failed to download Ollama: {}", e);
            println!("cargo:warning=Ollama will need to be installed manually");
            println!(
                "cargo:warning=Install with: brew install ollama (macOS) or see https://ollama.ai"
            );
        }
    }
}

/// Download binary using reqwest (blocking)
fn download_binary(url: &str, dest: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}: {}", response.status(), url).into());
    }

    let bytes = response.bytes()?;
    let mut file = File::create(dest)?;
    file.write_all(&bytes)?;

    Ok(())
}

/// Write the binary path to a file for runtime discovery
fn write_binary_path(out_dir: &Path, binary_path: &Path) {
    let path_file = out_dir.join("ollama_binary_path.txt");
    if let Ok(mut file) = File::create(&path_file) {
        let _ = writeln!(file, "{}", binary_path.display());
    }
}
