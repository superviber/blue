//! Build script for Blue CLI
//!
//! Downloads Goose binary for the target platform during build.
//! Binary is placed in OUT_DIR and copied to target dir post-build.

use std::env;
use std::fs;
use std::path::PathBuf;

#[allow(unused_imports)]
use std::io::Write;

const GOOSE_VERSION: &str = "1.21.1";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=BLUE_SKIP_DOWNLOAD");
    println!("cargo:rerun-if-env-changed=BLUE_GOOSE_PATH");

    // Skip download if explicitly disabled (for CI caching)
    if env::var("BLUE_SKIP_DOWNLOAD").is_ok() {
        println!("cargo:warning=Skipping Goose download (BLUE_SKIP_DOWNLOAD set)");
        return;
    }

    // Use pre-downloaded binary if specified
    if let Ok(path) = env::var("BLUE_GOOSE_PATH") {
        println!("cargo:warning=Using pre-downloaded Goose from {}", path);
        copy_goose_binary(&PathBuf::from(path));
        return;
    }

    // Check if we already have the binary
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let goose_binary = out_dir.join(if cfg!(windows) { "goose.exe" } else { "goose" });

    if goose_binary.exists() {
        println!("cargo:warning=Goose binary already exists");
        copy_goose_binary(&goose_binary);
        return;
    }

    // Download Goose for target platform
    if let Err(e) = download_goose() {
        println!("cargo:warning=Failed to download Goose: {}", e);
        println!("cargo:warning=blue agent will check for system Goose at runtime");
    }
}

fn download_goose() -> Result<(), Box<dyn std::error::Error>> {
    let target = env::var("TARGET")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let (url, archive_name) = get_goose_url(&target)?;

    println!(
        "cargo:warning=Downloading Goose {} for {}",
        GOOSE_VERSION, target
    );

    // Download to OUT_DIR
    let archive_path = out_dir.join(&archive_name);
    download_file(&url, &archive_path)?;

    // Extract binary
    let goose_binary = extract_goose(&archive_path, &out_dir, &target)?;

    // Copy to cargo output location
    copy_goose_binary(&goose_binary);

    Ok(())
}

fn get_goose_url(target: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let base = format!(
        "https://github.com/block/goose/releases/download/v{}",
        GOOSE_VERSION
    );

    let (archive, name) = match target {
        // macOS ARM64 (M1/M2/M3/M4)
        t if t.contains("aarch64") && t.contains("apple") => (
            format!("{}/goose-aarch64-apple-darwin.tar.bz2", base),
            "goose-aarch64-apple-darwin.tar.bz2".to_string(),
        ),
        // macOS x86_64
        t if t.contains("x86_64") && t.contains("apple") => (
            format!("{}/goose-x86_64-apple-darwin.tar.bz2", base),
            "goose-x86_64-apple-darwin.tar.bz2".to_string(),
        ),
        // Linux x86_64
        t if t.contains("x86_64") && t.contains("linux") => (
            format!("{}/goose-x86_64-unknown-linux-gnu.tar.bz2", base),
            "goose-x86_64-unknown-linux-gnu.tar.bz2".to_string(),
        ),
        // Linux ARM64
        t if t.contains("aarch64") && t.contains("linux") => (
            format!("{}/goose-aarch64-unknown-linux-gnu.tar.bz2", base),
            "goose-aarch64-unknown-linux-gnu.tar.bz2".to_string(),
        ),
        // Windows x86_64
        t if t.contains("x86_64") && t.contains("windows") => (
            format!("{}/goose-x86_64-pc-windows-gnu.zip", base),
            "goose-x86_64-pc-windows-gnu.zip".to_string(),
        ),
        _ => return Err(format!("Unsupported target: {}", target).into()),
    };

    Ok((archive, name))
}

fn download_file(url: &str, dest: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // Use curl for simplicity - available on all platforms
    let status = std::process::Command::new("curl")
        .args(["-L", "-o"])
        .arg(dest)
        .arg(url)
        .status()?;

    if !status.success() {
        return Err(format!("curl failed with status: {}", status).into());
    }

    Ok(())
}

fn extract_goose(
    archive: &PathBuf,
    out_dir: &PathBuf,
    target: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let binary_name = if target.contains("windows") {
        "goose.exe"
    } else {
        "goose"
    };

    if archive.to_string_lossy().ends_with(".tar.bz2") {
        // Extract tar.bz2
        let status = std::process::Command::new("tar")
            .args(["-xjf"])
            .arg(archive)
            .arg("-C")
            .arg(out_dir)
            .status()?;

        if !status.success() {
            return Err("tar extraction failed".into());
        }
    } else if archive.to_string_lossy().ends_with(".zip") {
        // Extract zip
        let status = std::process::Command::new("unzip")
            .args(["-o"])
            .arg(archive)
            .arg("-d")
            .arg(out_dir)
            .status()?;

        if !status.success() {
            return Err("unzip extraction failed".into());
        }
    }

    // Find the goose binary (might be in a subdirectory)
    let binary_path = find_binary(out_dir, binary_name)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&binary_path, perms)?;
    }

    Ok(binary_path)
}

fn find_binary(dir: &PathBuf, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Check direct path first
    let direct = dir.join(name);
    if direct.exists() {
        return Ok(direct);
    }

    // Search subdirectories
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if let Ok(found) = find_binary(&path, name) {
                return Ok(found);
            }
        } else if path.file_name().map(|n| n == name).unwrap_or(false) {
            return Ok(path);
        }
    }

    Err(format!("Binary {} not found in {:?}", name, dir).into())
}

fn copy_goose_binary(source: &PathBuf) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Tell Cargo where the binary is
    println!("cargo:rustc-env=GOOSE_BINARY_PATH={}", source.display());

    // Also copy to a known location in OUT_DIR for runtime discovery
    let dest = out_dir.join("goose");
    if source != &dest {
        if let Err(e) = fs::copy(source, &dest) {
            println!("cargo:warning=Failed to copy Goose binary: {}", e);
        }
    }
}
