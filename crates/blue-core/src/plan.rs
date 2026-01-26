//! Plan file parsing and generation
//!
//! RFC 0017: Plan files (.plan.md) are the authoritative source for RFC task tracking.
//! SQLite acts as a derived cache that is rebuilt on read when stale.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

/// A parsed plan file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanFile {
    pub rfc_title: String,
    pub status: PlanStatus,
    pub updated_at: String,
    pub tasks: Vec<PlanTask>,
}

/// Plan status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlanStatus {
    InProgress,
    Complete,
    UpdatingPlan,
}

impl PlanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlanStatus::InProgress => "in-progress",
            PlanStatus::Complete => "complete",
            PlanStatus::UpdatingPlan => "updating-plan",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace(' ', "-").as_str() {
            "in-progress" => Some(PlanStatus::InProgress),
            "complete" => Some(PlanStatus::Complete),
            "updating-plan" => Some(PlanStatus::UpdatingPlan),
            _ => None,
        }
    }
}

/// A task within a plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanTask {
    pub description: String,
    pub completed: bool,
}

/// Error type for plan operations
#[derive(Debug)]
pub enum PlanError {
    Io(std::io::Error),
    Parse(String),
    InvalidFormat(String),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Io(e) => write!(f, "IO error: {}", e),
            PlanError::Parse(msg) => write!(f, "Parse error: {}", msg),
            PlanError::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
        }
    }
}

impl std::error::Error for PlanError {}

impl From<std::io::Error> for PlanError {
    fn from(e: std::io::Error) -> Self {
        PlanError::Io(e)
    }
}

/// Parse a plan markdown file into a PlanFile struct
pub fn parse_plan_markdown(content: &str) -> Result<PlanFile, PlanError> {
    // Extract RFC title from header: # Plan: {title}
    let title_re = Regex::new(r"^# Plan: (.+)$").unwrap();
    let rfc_title = content
        .lines()
        .find_map(|line| {
            title_re
                .captures(line)
                .map(|c| c.get(1).unwrap().as_str().to_string())
        })
        .ok_or_else(|| PlanError::Parse("Missing '# Plan: {title}' header".to_string()))?;

    // Extract status from table: | **Status** | {status} |
    let status_re = Regex::new(r"\| \*\*Status\*\* \| ([^|]+) \|").unwrap();
    let status_str = content
        .lines()
        .find_map(|line| {
            status_re
                .captures(line)
                .map(|c| c.get(1).unwrap().as_str().trim().to_string())
        })
        .unwrap_or_else(|| "in-progress".to_string());

    let status = PlanStatus::from_str(&status_str).unwrap_or(PlanStatus::InProgress);

    // Extract updated_at from table: | **Updated** | {timestamp} |
    let updated_re = Regex::new(r"\| \*\*Updated\*\* \| ([^|]+) \|").unwrap();
    let updated_at = content
        .lines()
        .find_map(|line| {
            updated_re
                .captures(line)
                .map(|c| c.get(1).unwrap().as_str().trim().to_string())
        })
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    // Extract tasks from ## Tasks section
    let task_re = Regex::new(r"^- \[([ xX])\] (.+)$").unwrap();
    let mut tasks = Vec::new();
    let mut in_tasks_section = false;

    for line in content.lines() {
        if line.starts_with("## Tasks") {
            in_tasks_section = true;
            continue;
        }
        // Stop at next section
        if in_tasks_section && line.starts_with("## ") {
            break;
        }
        if in_tasks_section {
            if let Some(caps) = task_re.captures(line) {
                let completed = caps.get(1).unwrap().as_str() != " ";
                let description = caps.get(2).unwrap().as_str().to_string();
                tasks.push(PlanTask {
                    description,
                    completed,
                });
            }
        }
    }

    Ok(PlanFile {
        rfc_title,
        status,
        updated_at,
        tasks,
    })
}

/// Generate markdown content from a PlanFile
pub fn generate_plan_markdown(plan: &PlanFile) -> String {
    let mut md = String::new();

    // Title
    md.push_str(&format!("# Plan: {}\n\n", plan.rfc_title));

    // Metadata table
    md.push_str("| | |\n|---|---|\n");
    md.push_str(&format!("| **RFC** | {} |\n", plan.rfc_title));
    md.push_str(&format!("| **Status** | {} |\n", plan.status.as_str()));
    md.push_str(&format!("| **Updated** | {} |\n", plan.updated_at));
    md.push_str("\n");

    // Tasks section
    md.push_str("## Tasks\n\n");
    for task in &plan.tasks {
        let checkbox = if task.completed { "[x]" } else { "[ ]" };
        md.push_str(&format!("- {} {}\n", checkbox, task.description));
    }

    md
}

/// Get the path for a plan file given the RFC docs path, title, and number
pub fn plan_file_path(docs_path: &Path, rfc_title: &str, rfc_number: i32) -> PathBuf {
    let filename = format!("{:04}-{}.plan.md", rfc_number, rfc_title);
    docs_path.join("rfcs").join(filename)
}

/// Check if the SQLite cache is stale compared to the plan file
///
/// Returns true if the plan file exists and is newer than the cache mtime
pub fn is_cache_stale(plan_path: &Path, cache_mtime: Option<&str>) -> bool {
    if !plan_path.exists() {
        return false;
    }

    let Some(cache_mtime) = cache_mtime else {
        // No cache entry means stale
        return true;
    };

    // Get file modification time
    let Ok(metadata) = fs::metadata(plan_path) else {
        return false;
    };

    let Ok(modified) = metadata.modified() else {
        return false;
    };

    // Convert to RFC3339 for comparison
    let file_mtime: chrono::DateTime<chrono::Utc> = modified.into();
    let file_mtime_str = file_mtime.to_rfc3339();

    // Cache is stale if file is newer
    file_mtime_str > cache_mtime.to_string()
}

/// Read and parse a plan file from disk
pub fn read_plan_file(plan_path: &Path) -> Result<PlanFile, PlanError> {
    let content = fs::read_to_string(plan_path)?;
    parse_plan_markdown(&content)
}

/// Write a plan file to disk
pub fn write_plan_file(plan_path: &Path, plan: &PlanFile) -> Result<(), PlanError> {
    let content = generate_plan_markdown(plan);
    fs::write(plan_path, content)?;
    Ok(())
}

/// Update a specific task in a plan file
pub fn update_task_in_plan(
    plan_path: &Path,
    task_index: usize,
    completed: bool,
) -> Result<PlanFile, PlanError> {
    let mut plan = read_plan_file(plan_path)?;

    if task_index >= plan.tasks.len() {
        return Err(PlanError::InvalidFormat(format!(
            "Task index {} out of bounds (max {})",
            task_index,
            plan.tasks.len()
        )));
    }

    plan.tasks[task_index].completed = completed;
    plan.updated_at = chrono::Utc::now().to_rfc3339();

    // Check if all tasks are complete
    if plan.tasks.iter().all(|t| t.completed) {
        plan.status = PlanStatus::Complete;
    }

    write_plan_file(plan_path, &plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plan_markdown() {
        let content = r#"# Plan: my-feature

| | |
|---|---|
| **RFC** | my-feature |
| **Status** | in-progress |
| **Updated** | 2026-01-26T10:30:00Z |

## Tasks

- [x] Completed task
- [ ] Pending task
- [X] Another completed task
"#;

        let plan = parse_plan_markdown(content).unwrap();
        assert_eq!(plan.rfc_title, "my-feature");
        assert_eq!(plan.status, PlanStatus::InProgress);
        assert_eq!(plan.tasks.len(), 3);
        assert!(plan.tasks[0].completed);
        assert!(!plan.tasks[1].completed);
        assert!(plan.tasks[2].completed);
        assert_eq!(plan.tasks[0].description, "Completed task");
        assert_eq!(plan.tasks[1].description, "Pending task");
    }

    #[test]
    fn test_generate_plan_markdown() {
        let plan = PlanFile {
            rfc_title: "test-feature".to_string(),
            status: PlanStatus::InProgress,
            updated_at: "2026-01-26T10:30:00Z".to_string(),
            tasks: vec![
                PlanTask {
                    description: "First task".to_string(),
                    completed: true,
                },
                PlanTask {
                    description: "Second task".to_string(),
                    completed: false,
                },
            ],
        };

        let md = generate_plan_markdown(&plan);
        assert!(md.contains("# Plan: test-feature"));
        assert!(md.contains("| **Status** | in-progress |"));
        assert!(md.contains("- [x] First task"));
        assert!(md.contains("- [ ] Second task"));
    }

    #[test]
    fn test_plan_file_path() {
        let docs_path = Path::new("/project/.blue/docs");
        let path = plan_file_path(docs_path, "my-feature", 7);
        assert_eq!(
            path,
            PathBuf::from("/project/.blue/docs/rfcs/0007-my-feature.plan.md")
        );
    }

    #[test]
    fn test_roundtrip() {
        let original = PlanFile {
            rfc_title: "roundtrip-test".to_string(),
            status: PlanStatus::Complete,
            updated_at: "2026-01-26T12:00:00Z".to_string(),
            tasks: vec![
                PlanTask {
                    description: "Task one".to_string(),
                    completed: true,
                },
                PlanTask {
                    description: "Task two".to_string(),
                    completed: true,
                },
            ],
        };

        let markdown = generate_plan_markdown(&original);
        let parsed = parse_plan_markdown(&markdown).unwrap();

        assert_eq!(parsed.rfc_title, original.rfc_title);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.tasks.len(), original.tasks.len());
        for (p, o) in parsed.tasks.iter().zip(original.tasks.iter()) {
            assert_eq!(p.description, o.description);
            assert_eq!(p.completed, o.completed);
        }
    }

    #[test]
    fn test_is_cache_stale_no_file() {
        let path = Path::new("/nonexistent/path.plan.md");
        assert!(!is_cache_stale(path, Some("2026-01-01T00:00:00Z")));
    }

    #[test]
    fn test_is_cache_stale_no_cache() {
        let dir = tempfile::tempdir().unwrap();
        let plan_path = dir.path().join("test.plan.md");
        std::fs::write(&plan_path, "# Plan: test\n").unwrap();

        assert!(is_cache_stale(&plan_path, None));
    }

    #[test]
    fn test_status_from_str() {
        assert_eq!(
            PlanStatus::from_str("in-progress"),
            Some(PlanStatus::InProgress)
        );
        assert_eq!(
            PlanStatus::from_str("In Progress"),
            Some(PlanStatus::InProgress)
        );
        assert_eq!(PlanStatus::from_str("complete"), Some(PlanStatus::Complete));
        assert_eq!(
            PlanStatus::from_str("updating-plan"),
            Some(PlanStatus::UpdatingPlan)
        );
        assert_eq!(PlanStatus::from_str("invalid"), None);
    }
}
