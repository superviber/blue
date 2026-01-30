//! Lint tool handler
//!
//! Runs code quality checks and returns structured results with fix commands.
//! Supports Rust, JavaScript/TypeScript, Python, CDK, and Mermaid diagrams.

use std::path::Path;
use std::process::Command;

use regex::Regex;
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

    let rfcs_path = path.join(".blue/docs/rfcs");
    let docs_path = path.join(".blue/docs");

    // Header checks
    if (check_type == "all" || check_type == "headers") && rfcs_path.exists() {
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
    }

    // RFC 0043: Mermaid diagram checks
    if (check_type == "all" || check_type == "mermaid") && docs_path.exists() {
        let mermaid_results = run_mermaid_checks(&docs_path, fix);
        results.push(mermaid_results);
    }

    results
}

// ==================== RFC 0043: Mermaid Diagram Linting ====================

/// Mermaid diagram issue found during linting
#[derive(Debug)]
struct MermaidIssue {
    file: String,
    line: usize,
    severity: MermaidSeverity,
    message: String,
    auto_fixable: bool,
}

#[derive(Debug, Clone, Copy)]
enum MermaidSeverity {
    Error,
    Warning,
}

/// RFC 0043: Run Mermaid diagram checks on all markdown files in .blue/docs/
fn run_mermaid_checks(docs_path: &Path, fix: bool) -> LintResult {
    use std::fs;

    let mut issues: Vec<MermaidIssue> = Vec::new();
    let mut fixed_count = 0;

    // Recursively scan all .md files
    scan_markdown_files(docs_path, &mut |file_path| {
        if let Ok(content) = fs::read_to_string(file_path) {
            let file_issues = check_mermaid_blocks(&content, file_path);

            if fix && file_issues.iter().any(|i| i.auto_fixable) {
                // Apply auto-fixes (only theme declaration)
                let fixed_content = apply_mermaid_fixes(&content);
                if fixed_content != content {
                    if fs::write(file_path, &fixed_content).is_ok() {
                        fixed_count += 1;
                    }
                }
                // Re-check for remaining issues (non-fixable ones)
                let remaining = check_mermaid_blocks(&fixed_content, file_path);
                issues.extend(remaining.into_iter().filter(|i| !i.auto_fixable));
            } else {
                issues.extend(file_issues);
            }
        }
    });

    let error_count = issues.iter().filter(|i| matches!(i.severity, MermaidSeverity::Error)).count();
    let warning_count = issues.iter().filter(|i| matches!(i.severity, MermaidSeverity::Warning)).count();

    // Log details
    if !issues.is_empty() || fixed_count > 0 {
        for issue in &issues {
            let severity = match issue.severity {
                MermaidSeverity::Error => "error",
                MermaidSeverity::Warning => "warning",
            };
            tracing::info!(
                "Mermaid {}: {}:{} - {}",
                severity,
                issue.file,
                issue.line,
                issue.message
            );
        }
        if fixed_count > 0 {
            tracing::info!("Mermaid: auto-fixed {} file(s) with missing theme declaration", fixed_count);
        }
    }

    LintResult {
        project_type: ProjectType::RfcDocs,
        name: "mermaid",
        tool: "blue_lint",
        passed: error_count == 0,
        issue_count: error_count + warning_count,
        fix_command: "blue_lint --fix --check mermaid",
    }
}

/// Recursively scan markdown files
fn scan_markdown_files<F>(dir: &Path, callback: &mut F)
where
    F: FnMut(&Path),
{
    use std::fs;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_markdown_files(&path, callback);
            } else if let Some(ext) = path.extension() {
                if ext == "md" {
                    callback(&path);
                }
            }
        }
    }
}

/// Extract Mermaid code blocks from markdown content
fn extract_mermaid_blocks(content: &str) -> Vec<(usize, String)> {
    let mut blocks = Vec::new();
    let mut in_mermaid = false;
    let mut current_block = String::new();
    let mut block_start_line = 0;

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("```mermaid") {
            in_mermaid = true;
            block_start_line = line_num + 1;
            current_block.clear();
        } else if in_mermaid && trimmed == "```" {
            in_mermaid = false;
            blocks.push((block_start_line, current_block.clone()));
        } else if in_mermaid {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }

    blocks
}

/// Check a single markdown file for Mermaid issues
fn check_mermaid_blocks(content: &str, file_path: &Path) -> Vec<MermaidIssue> {
    let mut issues = Vec::new();
    let file_name = file_path.to_string_lossy().to_string();

    for (line_num, block) in extract_mermaid_blocks(content) {
        // Check 1: Missing neutral theme (REQUIRED, auto-fixable)
        if !block.contains("'theme': 'neutral'") && !block.contains("\"theme\": \"neutral\"") {
            issues.push(MermaidIssue {
                file: file_name.clone(),
                line: line_num,
                severity: MermaidSeverity::Error,
                message: "Mermaid diagram must use neutral theme. Add: %%{init: {'theme': 'neutral'}}%%".into(),
                auto_fixable: true,
            });
        }

        // Check 2: Custom fill colors (PROHIBITED, NOT auto-fixable)
        if block.contains("fill:#") || block.contains("fill: #") {
            issues.push(MermaidIssue {
                file: file_name.clone(),
                line: line_num,
                severity: MermaidSeverity::Error,
                message: "Custom fill colors prohibited. Remove style directives manually; review semantic intent.".into(),
                auto_fixable: false,
            });
        }

        // Check 3: LR flow with >3 leaf nodes (ADVISORY)
        if block.contains("flowchart LR") || block.contains("graph LR") {
            let leaf_count = count_leaf_nodes(&block);
            if leaf_count > 3 {
                issues.push(MermaidIssue {
                    file: file_name.clone(),
                    line: line_num,
                    severity: MermaidSeverity::Warning,
                    message: format!(
                        "LR flow with {} leaf nodes may cause horizontal scrolling. Consider flowchart TB.",
                        leaf_count
                    ),
                    auto_fixable: false,
                });
            }
        }
    }

    issues
}

/// Count leaf nodes (terminal visual elements, not container subgraphs)
fn count_leaf_nodes(mermaid_content: &str) -> usize {
    // Match node definitions: ID[label], ID(label), ID[(label)], ID{{label}}, ID((label))
    // Exclude: subgraph, end, style, classDef, linkStyle, and arrows/edges
    let node_pattern = Regex::new(r"^\s*(\w+)\s*[\[\(\{<]").unwrap();
    let exclude_keywords = ["subgraph", "end", "style", "classDef", "linkStyle", "%%"];

    mermaid_content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            // Skip keywords
            if exclude_keywords.iter().any(|kw| trimmed.starts_with(kw)) {
                return false;
            }
            // Skip empty lines
            if trimmed.is_empty() {
                return false;
            }
            // Skip edge definitions (lines with --> or --- etc)
            if trimmed.contains("-->") || trimmed.contains("---") || trimmed.contains("-.->") {
                // But count if this line also defines a node
                return node_pattern.is_match(trimmed);
            }
            node_pattern.is_match(trimmed)
        })
        .count()
}

/// Apply auto-fixes to Mermaid blocks (theme declaration only)
fn apply_mermaid_fixes(content: &str) -> String {
    let mut result = String::new();
    let mut in_mermaid = false;
    let mut block_has_theme = false;
    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```mermaid") {
            in_mermaid = true;
            block_has_theme = false;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_mermaid && trimmed == "```" {
            in_mermaid = false;
            result.push_str(line);
            result.push('\n');
            continue;
        }

        if in_mermaid {
            // Check if this block already has theme
            if line.contains("'theme'") || line.contains("\"theme\"") {
                block_has_theme = true;
            }

            // If this is the first content line and no theme yet, insert it
            if !block_has_theme && !trimmed.is_empty() && !trimmed.starts_with("%%{init") {
                // Check if it's a flowchart/graph declaration
                if trimmed.starts_with("flowchart") || trimmed.starts_with("graph") ||
                   trimmed.starts_with("sequenceDiagram") || trimmed.starts_with("classDiagram") ||
                   trimmed.starts_with("stateDiagram") || trimmed.starts_with("erDiagram") ||
                   trimmed.starts_with("gantt") || trimmed.starts_with("pie") {
                    result.push_str("%%{init: {'theme': 'neutral'}}%%\n");
                    block_has_theme = true;
                }
            }
        }

        result.push_str(line);
        result.push('\n');
    }

    // Remove trailing newline if original didn't have one
    if !content.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::path::PathBuf;

    #[test]
    fn test_detect_project_types_rust() {
        let dir = temp_dir().join("blue_lint_test_rust");
        std::fs::create_dir_all(&dir).ok();
        std::fs::write(dir.join("Cargo.toml"), "[package]").ok();

        let types = detect_project_types(&dir);
        assert!(types.iter().any(|t| matches!(t, ProjectType::Rust)));

        std::fs::remove_dir_all(&dir).ok();
    }

    // ==================== RFC 0043: Mermaid Tests ====================

    #[test]
    fn test_extract_mermaid_blocks() {
        let content = r#"
# Test Document

Some text here.

```mermaid
flowchart TB
    A[Start] --> B[End]
```

More text.

```mermaid
graph LR
    X --> Y
```
"#;

        let blocks = extract_mermaid_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].1.contains("flowchart TB"));
        assert!(blocks[1].1.contains("graph LR"));
    }

    #[test]
    fn test_count_leaf_nodes() {
        let mermaid = r#"
flowchart LR
    subgraph SV["Control Plane"]
        A[Service A]
        B[Service B]
    end
    C[Client] --> A
    D[External] --> B
"#;
        // Should count A, B, C, D = 4 nodes (subgraph is not a leaf)
        let count = count_leaf_nodes(mermaid);
        assert_eq!(count, 4);
    }

    #[test]
    fn test_count_leaf_nodes_simple() {
        // Nodes with explicit labels are counted
        let mermaid = r#"
flowchart LR
    A[Node A]
    B[Node B]
    C[Node C]
    A --> B --> C
"#;
        let count = count_leaf_nodes(mermaid);
        assert_eq!(count, 3);
    }

    #[test]
    fn test_count_leaf_nodes_inline_edges_only() {
        // Pure edge definitions without node shapes don't count
        // This matches Mermaid's implicit node creation syntax
        let mermaid = r#"
flowchart LR
    A --> B --> C
"#;
        let count = count_leaf_nodes(mermaid);
        // Implicit nodes (no brackets) are not counted by our heuristic
        // This is intentional: LR warning is about visual complexity, and
        // implicit nodes are typically simpler diagrams
        assert_eq!(count, 0);
    }

    #[test]
    fn test_check_mermaid_blocks_missing_theme() {
        let content = r#"
```mermaid
flowchart TB
    A --> B
```
"#;
        let issues = check_mermaid_blocks(content, &PathBuf::from("test.md"));
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.message.contains("neutral theme")));
        assert!(issues.iter().any(|i| i.auto_fixable));
    }

    #[test]
    fn test_check_mermaid_blocks_has_theme() {
        let content = r#"
```mermaid
%%{init: {'theme': 'neutral'}}%%
flowchart TB
    A --> B
```
"#;
        let issues = check_mermaid_blocks(content, &PathBuf::from("test.md"));
        // Should have no theme-related issues
        assert!(!issues.iter().any(|i| i.message.contains("neutral theme")));
    }

    #[test]
    fn test_check_mermaid_blocks_fill_color() {
        let content = r#"
```mermaid
%%{init: {'theme': 'neutral'}}%%
flowchart TB
    A --> B
    style A fill:#e8f5e9
```
"#;
        let issues = check_mermaid_blocks(content, &PathBuf::from("test.md"));
        assert!(issues.iter().any(|i| i.message.contains("fill colors prohibited")));
        // fill color issues should NOT be auto-fixable
        assert!(issues.iter().filter(|i| i.message.contains("fill")).all(|i| !i.auto_fixable));
    }

    #[test]
    fn test_check_mermaid_blocks_lr_warning() {
        let content = r#"
```mermaid
%%{init: {'theme': 'neutral'}}%%
flowchart LR
    A[Node A]
    B[Node B]
    C[Node C]
    D[Node D]
    E[Node E]
```
"#;
        let issues = check_mermaid_blocks(content, &PathBuf::from("test.md"));
        // Should warn about LR with >3 nodes
        assert!(issues.iter().any(|i|
            matches!(i.severity, MermaidSeverity::Warning) &&
            i.message.contains("horizontal scrolling")
        ));
    }

    #[test]
    fn test_check_mermaid_blocks_lr_ok() {
        let content = r#"
```mermaid
%%{init: {'theme': 'neutral'}}%%
flowchart LR
    A --> B --> C
```
"#;
        let issues = check_mermaid_blocks(content, &PathBuf::from("test.md"));
        // Should NOT warn about LR with <=3 nodes
        assert!(!issues.iter().any(|i| i.message.contains("horizontal scrolling")));
    }

    #[test]
    fn test_apply_mermaid_fixes() {
        let content = r#"```mermaid
flowchart TB
    A --> B
```"#;
        let fixed = apply_mermaid_fixes(content);
        assert!(fixed.contains("%%{init: {'theme': 'neutral'}}%%"));
        assert!(fixed.contains("flowchart TB"));
    }

    #[test]
    fn test_apply_mermaid_fixes_already_has_theme() {
        let content = r#"```mermaid
%%{init: {'theme': 'neutral'}}%%
flowchart TB
    A --> B
```"#;
        let fixed = apply_mermaid_fixes(content);
        // Should not add duplicate theme
        let theme_count = fixed.matches("theme").count();
        assert_eq!(theme_count, 1);
    }
}
