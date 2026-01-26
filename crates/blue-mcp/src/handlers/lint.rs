//! Lint tool handler
//!
//! Runs code quality checks and returns structured results with fix commands.
//! Supports Rust, JavaScript/TypeScript, Python, and CDK.

use std::path::Path;
use std::process::Command;

use serde_json::{json, Value};

use crate::error::ServerError;

/// Detected project type
#[derive(Debug, Clone, Copy)]
enum ProjectType {
    Rust,
    JavaScript,
    Python,
    Cdk,
    RfcDocs,
}

impl ProjectType {
    fn as_str(&self) -> &'static str {
        match self {
            ProjectType::Rust => "rust",
            ProjectType::JavaScript => "javascript",
            ProjectType::Python => "python",
            ProjectType::Cdk => "cdk",
            ProjectType::RfcDocs => "rfc-docs",
        }
    }
}

/// Result of a single lint check
struct LintResult {
    project_type: ProjectType,
    name: &'static str,
    tool: &'static str,
    passed: bool,
    issue_count: usize,
    fix_command: &'static str,
}

/// Handle blue_lint
pub fn handle_lint(args: &Value, repo_path: &Path) -> Result<Value, ServerError> {
    let fix = args.get("fix").and_then(|v| v.as_bool()).unwrap_or(false);
    let check_type = args.get("check").and_then(|v| v.as_str()).unwrap_or("all");

    // Detect project types
    let project_types = detect_project_types(repo_path);

    if project_types.is_empty() {
        return Ok(json!({
            "status": "error",
            "message": blue_core::voice::error(
                "No supported project type detected",
                "Need Cargo.toml, package.json, pyproject.toml, or cdk.json"
            )
        }));
    }

    // Run checks for each project type
    let mut all_results: Vec<LintResult> = Vec::new();

    for project_type in &project_types {
        let results = match project_type {
            ProjectType::Rust => run_rust_checks(repo_path, fix, check_type),
            ProjectType::JavaScript => run_js_checks(repo_path, fix, check_type),
            ProjectType::Python => run_python_checks(repo_path, fix, check_type),
            ProjectType::Cdk => run_cdk_checks(repo_path, check_type),
            ProjectType::RfcDocs => run_rfc_checks(repo_path, fix, check_type),
        };
        all_results.extend(results);
    }

    // Calculate summary
    let total_issues: usize = all_results.iter().map(|r| r.issue_count).sum();
    let format_issues: usize = all_results
        .iter()
        .filter(|r| r.name == "format")
        .map(|r| r.issue_count)
        .sum();
    let lint_issues: usize = all_results
        .iter()
        .filter(|r| r.name == "lint" || r.name == "synth")
        .map(|r| r.issue_count)
        .sum();
    let all_passed = all_results.iter().all(|r| r.passed);

    // Build fix commands
    let fix_commands: Vec<&str> = all_results
        .iter()
        .filter(|r| !r.passed && !r.fix_command.starts_with('#'))
        .map(|r| r.fix_command)
        .collect();

    let fix_all_command = if fix_commands.is_empty() {
        None
    } else {
        Some(fix_commands.join(" && "))
    };

    let hint = if all_passed {
        "All checks passed! Ready for PR.".to_string()
    } else if let Some(ref cmd) = fix_all_command {
        format!("{} issues. Run `{}` to fix.", total_issues, cmd)
    } else {
        format!("{} issues found.", total_issues)
    };

    let message_title = if all_passed {
        "All checks passed".to_string()
    } else {
        format!("{} issues found", total_issues)
    };

    Ok(json!({
        "status": "success",
        "message": blue_core::voice::info(
            &message_title,
            Some(&hint)
        ),
        "project_types": project_types.iter().map(|t| t.as_str()).collect::<Vec<_>>(),
        "checks": all_results.iter().map(|r| json!({
            "project_type": r.project_type.as_str(),
            "name": r.name,
            "tool": r.tool,
            "passed": r.passed,
            "issue_count": r.issue_count,
            "fix_command": r.fix_command
        })).collect::<Vec<_>>(),
        "all_passed": all_passed,
        "summary": {
            "total_issues": total_issues,
            "format_issues": format_issues,
            "lint_issues": lint_issues
        },
        "fix_all_command": fix_all_command
    }))
}

fn detect_project_types(path: &Path) -> Vec<ProjectType> {
    let mut types = Vec::new();

    if path.join("Cargo.toml").exists() {
        types.push(ProjectType::Rust);
    }
    if path.join("package.json").exists() {
        types.push(ProjectType::JavaScript);
    }
    if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        types.push(ProjectType::Python);
    }
    if path.join("cdk.json").exists() {
        types.push(ProjectType::Cdk);
    }
    // RFC 0017: Check for Blue RFC docs
    if path.join(".blue/docs/rfcs").exists() {
        types.push(ProjectType::RfcDocs);
    }

    types
}

fn command_exists(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_command(
    path: &Path,
    cmd: &str,
    args: &[&str],
    project_type: ProjectType,
    name: &'static str,
    tool: &'static str,
    fix_command: &'static str,
) -> LintResult {
    let output = Command::new(cmd).args(args).current_dir(path).output();

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined = format!("{}{}", stdout, stderr);
            let issue_count = count_issues(&combined, project_type, name);

            LintResult {
                project_type,
                name,
                tool,
                passed: output.status.success() && issue_count == 0,
                issue_count,
                fix_command,
            }
        }
        Err(_) => LintResult {
            project_type,
            name,
            tool,
            passed: false,
            issue_count: 1,
            fix_command,
        },
    }
}

fn count_issues(output: &str, project_type: ProjectType, check_name: &str) -> usize {
    match (project_type, check_name) {
        (ProjectType::Rust, "format") => {
            output.lines().filter(|l| l.starts_with("Diff in")).count()
        }
        (ProjectType::Rust, "lint") => {
            output
                .lines()
                .filter(|l| l.contains("warning:") && !l.contains("warnings emitted"))
                .count()
        }
        (ProjectType::JavaScript, "format") => {
            output
                .lines()
                .filter(|l| l.ends_with(".ts") || l.ends_with(".js") || l.ends_with(".tsx"))
                .count()
        }
        (ProjectType::JavaScript, "lint") => {
            output
                .lines()
                .filter(|l| l.contains("error") || l.contains("warning"))
                .count()
        }
        (ProjectType::Python, "format") => {
            output
                .lines()
                .filter(|l| l.contains("Would reformat"))
                .count()
        }
        (ProjectType::Python, "lint") => {
            output
                .lines()
                .filter(|l| l.contains(":") && !l.starts_with("Found"))
                .count()
        }
        (ProjectType::Cdk, "synth") => {
            if output.contains("Error:") || output.contains("error:") {
                1
            } else {
                0
            }
        }
        // RFC headers are counted directly in run_rfc_checks
        (ProjectType::RfcDocs, _) => 0,
        _ => 0,
    }
}

fn run_rust_checks(path: &Path, fix: bool, check_type: &str) -> Vec<LintResult> {
    let mut results = Vec::new();

    if check_type == "all" || check_type == "format" {
        let args: Vec<&str> = if fix {
            vec!["fmt"]
        } else {
            vec!["fmt", "--check"]
        };
        results.push(run_command(
            path,
            "cargo",
            &args,
            ProjectType::Rust,
            "format",
            "cargo fmt",
            "cargo fmt",
        ));
    }

    if check_type == "all" || check_type == "lint" {
        let args: Vec<&str> = if fix {
            vec!["clippy", "--fix", "--allow-dirty", "--allow-staged"]
        } else {
            vec!["clippy", "--", "-D", "warnings"]
        };
        results.push(run_command(
            path,
            "cargo",
            &args,
            ProjectType::Rust,
            "lint",
            "cargo clippy",
            "cargo clippy --fix --allow-dirty",
        ));
    }

    results
}

fn run_js_checks(path: &Path, fix: bool, check_type: &str) -> Vec<LintResult> {
    let mut results = Vec::new();

    if check_type == "all" || check_type == "format" {
        let args: Vec<&str> = if fix {
            vec!["prettier", "--write", "."]
        } else {
            vec!["prettier", "--check", "."]
        };
        results.push(run_command(
            path,
            "npx",
            &args,
            ProjectType::JavaScript,
            "format",
            "prettier",
            "npx prettier --write .",
        ));
    }

    if check_type == "all" || check_type == "lint" {
        let args: Vec<&str> = if fix {
            vec!["eslint", "--fix", "."]
        } else {
            vec!["eslint", "."]
        };
        results.push(run_command(
            path,
            "npx",
            &args,
            ProjectType::JavaScript,
            "lint",
            "eslint",
            "npx eslint --fix .",
        ));
    }

    results
}

fn run_python_checks(path: &Path, fix: bool, check_type: &str) -> Vec<LintResult> {
    let mut results = Vec::new();
    let use_ruff = command_exists("ruff");

    if check_type == "all" || check_type == "format" {
        if use_ruff {
            let args: Vec<&str> = if fix {
                vec!["format", "."]
            } else {
                vec!["format", "--check", "."]
            };
            results.push(run_command(
                path,
                "ruff",
                &args,
                ProjectType::Python,
                "format",
                "ruff format",
                "ruff format .",
            ));
        } else if command_exists("black") {
            let args: Vec<&str> = if fix { vec!["."] } else { vec!["--check", "."] };
            results.push(run_command(
                path,
                "black",
                &args,
                ProjectType::Python,
                "format",
                "black",
                "black .",
            ));
        }
    }

    if (check_type == "all" || check_type == "lint")
        && use_ruff {
            let args: Vec<&str> = if fix {
                vec!["check", "--fix", "."]
            } else {
                vec!["check", "."]
            };
            results.push(run_command(
                path,
                "ruff",
                &args,
                ProjectType::Python,
                "lint",
                "ruff check",
                "ruff check --fix .",
            ));
        }

    results
}

fn run_cdk_checks(path: &Path, check_type: &str) -> Vec<LintResult> {
    let mut results = Vec::new();

    if (check_type == "all" || check_type == "lint") && command_exists("cdk") {
        results.push(run_command(
            path,
            "cdk",
            &["synth", "--quiet"],
            ProjectType::Cdk,
            "synth",
            "cdk synth",
            "# cdk synth is validation only",
        ));
    }

    results
}

/// RFC 0017: Run RFC header checks
fn run_rfc_checks(path: &Path, fix: bool, check_type: &str) -> Vec<LintResult> {
    use blue_core::{HeaderFormat, convert_inline_to_table_header, validate_rfc_header};
    use std::fs;

    let mut results = Vec::new();

    if check_type != "all" && check_type != "headers" {
        return results;
    }

    let rfcs_path = path.join(".blue/docs/rfcs");
    if !rfcs_path.exists() {
        return results;
    }

    let mut inline_count = 0;
    let mut missing_count = 0;
    let mut fixed_count = 0;

    // Scan RFC files (exclude .plan.md files)
    if let Ok(entries) = fs::read_dir(&rfcs_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext != "md" {
                    continue;
                }
            } else {
                continue;
            }

            // Skip .plan.md files
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".plan.md") {
                    continue;
                }
            }

            if let Ok(content) = fs::read_to_string(&path) {
                match validate_rfc_header(&content) {
                    HeaderFormat::Table => {
                        // Good - canonical format
                    }
                    HeaderFormat::Inline => {
                        if fix {
                            let converted = convert_inline_to_table_header(&content);
                            if let Ok(()) = fs::write(&path, converted) {
                                fixed_count += 1;
                            }
                        } else {
                            inline_count += 1;
                        }
                    }
                    HeaderFormat::Missing => {
                        missing_count += 1;
                    }
                }
            }
        }
    }

    let total_issues = if fix { 0 } else { inline_count + missing_count };

    results.push(LintResult {
        project_type: ProjectType::RfcDocs,
        name: "headers",
        tool: "blue_lint",
        passed: total_issues == 0,
        issue_count: total_issues,
        fix_command: "blue_lint --fix --check headers",
    });

    // Add details if there were issues or fixes
    if inline_count > 0 || missing_count > 0 || fixed_count > 0 {
        tracing::info!(
            "RFC headers: {} inline (non-canonical), {} missing, {} fixed",
            inline_count,
            missing_count,
            fixed_count
        );
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_detect_project_types_rust() {
        let dir = temp_dir().join("blue_lint_test_rust");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join("Cargo.toml"), "[package]").ok();

        let types = detect_project_types(&dir);
        assert!(types.iter().any(|t| matches!(t, ProjectType::Rust)));

        std::fs::remove_dir_all(&dir).ok();
    }
}
