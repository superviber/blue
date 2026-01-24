//! Environment isolation tool handlers
//!
//! Handles detection of external dependencies and generation of
//! isolated environment configurations for parallel agent execution.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::error::ServerError;

/// Detected external dependency
#[derive(Debug)]
struct Dependency {
    dep_type: String,
    name: String,
    mock_strategy: String,
    env_var: Option<String>,
}

/// Handle blue_env_detect
pub fn handle_detect(args: &Value, repo_path: &Path) -> Result<Value, ServerError> {
    let path = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| std::path::PathBuf::from(s))
        .unwrap_or_else(|| repo_path.to_path_buf());

    let (dependencies, env_files, iac_detected, docker_detected, mock_config) =
        detect_dependencies(&path);

    let hint = if dependencies.is_empty() {
        "No external dependencies detected. Project appears self-contained."
    } else {
        "Use blue_env_mock to generate isolated environment."
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &format!("{} external dependencies detected", dependencies.len()),
            Some(hint)
        ),
        "dependencies": dependencies.iter().map(|d| json!({
            "type": d.dep_type,
            "name": d.name,
            "mock_strategy": d.mock_strategy,
            "env_var": d.env_var
        })).collect::<Vec<_>>(),
        "env_files": env_files,
        "iac_detected": iac_detected,
        "docker_detected": docker_detected,
        "mock_config": mock_config,
        "dependency_count": dependencies.len()
    }))
}

/// Handle blue_env_mock
pub fn handle_mock(args: &Value, repo_path: &Path) -> Result<Value, ServerError> {
    let scan_path = args
        .get("cwd")
        .and_then(|v| v.as_str())
        .map(|s| std::path::PathBuf::from(s))
        .unwrap_or_else(|| repo_path.to_path_buf());

    let worktree_path = args
        .get("worktree_path")
        .and_then(|v| v.as_str())
        .map(|s| std::path::PathBuf::from(s))
        .unwrap_or_else(|| scan_path.clone());

    let agent_id = args
        .get("agent_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate a simple unique ID based on timestamp
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            format!("{:08x}", ts as u32)
        });

    let (dependencies, _, _, _, mock_config) = detect_dependencies(&scan_path);

    // Generate .env.isolated content
    let env_content = generate_env_isolated(&agent_id, &worktree_path, &dependencies, &mock_config);

    // Write file
    let env_file_path = worktree_path.join(".env.isolated");
    fs::write(&env_file_path, &env_content)
        .map_err(|e| ServerError::CommandFailed(format!("Failed to write .env.isolated: {}", e)))?;

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::success(
            &format!("Created .env.isolated with AGENT_ID={}", agent_id),
            Some("Source it before running: `source .env.isolated`")
        ),
        "agent_id": agent_id,
        "env_file": env_file_path.display().to_string(),
        "mock_config": mock_config,
        "isolation_mode": "mock"
    }))
}

fn detect_dependencies(
    path: &Path,
) -> (
    Vec<Dependency>,
    Vec<String>,
    Option<String>,
    bool,
    HashMap<String, String>,
) {
    let mut dependencies = Vec::new();
    let mut env_files = Vec::new();
    let mut mock_config = HashMap::new();
    let mut iac_detected = None;
    let mut docker_detected = false;

    // Check for .env files
    for env_file in &[".env", ".env.example", ".env.local"] {
        let env_path = path.join(env_file);
        if env_path.exists() {
            env_files.push(env_file.to_string());
            if let Ok(content) = fs::read_to_string(&env_path) {
                parse_env_file(&content, &mut dependencies, &mut mock_config);
            }
        }
    }

    // Check for IaC
    if path.join("cdk.json").exists() {
        iac_detected = Some("cdk".to_string());
        dependencies.push(Dependency {
            dep_type: "iac".to_string(),
            name: "AWS CDK".to_string(),
            mock_strategy: "localstack".to_string(),
            env_var: None,
        });
    } else if path.join("terraform").is_dir() || path.join("main.tf").exists() {
        iac_detected = Some("terraform".to_string());
        dependencies.push(Dependency {
            dep_type: "iac".to_string(),
            name: "Terraform".to_string(),
            mock_strategy: "localstack".to_string(),
            env_var: None,
        });
    } else if path.join("Pulumi.yaml").exists() {
        iac_detected = Some("pulumi".to_string());
        dependencies.push(Dependency {
            dep_type: "iac".to_string(),
            name: "Pulumi".to_string(),
            mock_strategy: "localstack".to_string(),
            env_var: None,
        });
    }

    // Check for Docker
    if path.join("docker-compose.yml").exists() || path.join("docker-compose.yaml").exists() {
        docker_detected = true;
    }

    // Check pyproject.toml
    if let Ok(content) = fs::read_to_string(path.join("pyproject.toml")) {
        if content.contains("boto3") && !dependencies.iter().any(|d| d.dep_type == "s3") {
            dependencies.push(Dependency {
                dep_type: "s3".to_string(),
                name: "AWS SDK".to_string(),
                mock_strategy: "moto".to_string(),
                env_var: None,
            });
        }
    }

    // Check package.json
    if let Ok(content) = fs::read_to_string(path.join("package.json")) {
        if content.contains("@aws-sdk") && !dependencies.iter().any(|d| d.dep_type == "s3") {
            dependencies.push(Dependency {
                dep_type: "s3".to_string(),
                name: "AWS SDK (JS)".to_string(),
                mock_strategy: "aws-sdk-mock".to_string(),
                env_var: None,
            });
        }
    }

    // Set default mock configs
    set_default_mock_config(&dependencies, &mut mock_config);

    (dependencies, env_files, iac_detected, docker_detected, mock_config)
}

fn parse_env_file(
    content: &str,
    deps: &mut Vec<Dependency>,
    mock_config: &mut HashMap<String, String>,
) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, _)) = line.split_once('=') {
            let key = key.trim();

            if (key.contains("AWS") || key.contains("S3")) && !deps.iter().any(|d| d.dep_type == "s3")
            {
                deps.push(Dependency {
                    dep_type: "s3".to_string(),
                    name: "AWS S3".to_string(),
                    mock_strategy: "moto".to_string(),
                    env_var: Some(key.to_string()),
                });
                mock_config.insert("STORAGE_BACKEND".to_string(), "file".to_string());
            }

            if (key.contains("DATABASE") || key.contains("DB_") || key.contains("POSTGRES"))
                && !deps.iter().any(|d| d.dep_type == "database")
            {
                deps.push(Dependency {
                    dep_type: "database".to_string(),
                    name: "Database".to_string(),
                    mock_strategy: "sqlite_temp".to_string(),
                    env_var: Some(key.to_string()),
                });
                mock_config.insert("DB_PATH".to_string(), ".blue/test.db".to_string());
            }

            if key.contains("REDIS") && !deps.iter().any(|d| d.dep_type == "redis") {
                deps.push(Dependency {
                    dep_type: "redis".to_string(),
                    name: "Redis".to_string(),
                    mock_strategy: "fakeredis".to_string(),
                    env_var: Some(key.to_string()),
                });
                mock_config.insert("REDIS_URL".to_string(), "memory://".to_string());
            }
        }
    }
}

fn set_default_mock_config(deps: &[Dependency], mock_config: &mut HashMap<String, String>) {
    for dep in deps {
        match dep.dep_type.as_str() {
            "s3" => {
                mock_config.entry("STORAGE_BACKEND".to_string()).or_insert("file".to_string());
                mock_config.entry("LOCAL_STORAGE_PATH".to_string()).or_insert(".blue/storage".to_string());
                mock_config.entry("MOCK_S3".to_string()).or_insert("true".to_string());
            }
            "database" => {
                mock_config.entry("DB_PATH".to_string()).or_insert(".blue/test.db".to_string());
                mock_config.entry("MOCK_DATABASE".to_string()).or_insert("true".to_string());
            }
            "redis" => {
                mock_config.entry("REDIS_URL".to_string()).or_insert("memory://".to_string());
                mock_config.entry("MOCK_REDIS".to_string()).or_insert("true".to_string());
            }
            _ => {}
        }
    }
    mock_config.entry("BLUE_ISOLATION_MODE".to_string()).or_insert("mock".to_string());
}

fn generate_env_isolated(
    agent_id: &str,
    worktree_path: &Path,
    dependencies: &[Dependency],
    mock_config: &HashMap<String, String>,
) -> String {
    let mut lines = vec![
        "# Blue Environment Isolation".to_string(),
        "# Auto-generated - do not commit".to_string(),
        format!("# Worktree: {}", worktree_path.display()),
        "".to_string(),
        "# Agent Identification".to_string(),
        format!("BLUE_AGENT_ID={}", agent_id),
        "BLUE_ISOLATION_MODE=mock".to_string(),
        "".to_string(),
    ];

    if !mock_config.is_empty() {
        lines.push("# Mock Configurations".to_string());
        for (key, value) in mock_config {
            if key != "BLUE_ISOLATION_MODE" {
                lines.push(format!("{}={}", key, value));
            }
        }
        lines.push("".to_string());
    }

    lines.push("# Worktree-specific paths".to_string());
    lines.push(format!("BLUE_WORKTREE_PATH={}", worktree_path.display()));
    lines.push(format!("BLUE_DATA_DIR={}/.blue", worktree_path.display()));

    if !dependencies.is_empty() {
        lines.push("".to_string());
        lines.push("# Detected Dependencies".to_string());
        for dep in dependencies {
            lines.push(format!("# - {}: {} (mock: {})", dep.dep_type, dep.name, dep.mock_strategy));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_detect_empty_project() {
        let dir = temp_dir().join("blue_env_test");
        std::fs::create_dir_all(&dir).ok();

        let (deps, _, _, _, _) = detect_dependencies(&dir);
        assert!(deps.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }
}
