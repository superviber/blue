//! Jira import scan — discover epics/tasks and generate RFC stubs (RFC 0065)
//!
//! `blue jira import --dry-run` scans a Jira project and outputs proposed
//! RFC stubs and epic YAML files without writing anything.

use super::{IssueTracker, IssueType, StatusCategory};

/// A proposed RFC stub generated from a Jira task
#[derive(Debug, Clone)]
pub struct RfcStub {
    pub jira_key: String,
    pub title: String,
    pub status: &'static str,
    pub description: Option<String>,
    pub epic_key: Option<String>,
}

/// A proposed epic YAML generated from a Jira epic
#[derive(Debug, Clone)]
pub struct EpicStub {
    pub jira_key: String,
    pub title: String,
    pub status: &'static str,
    pub child_count: usize,
}

/// Result of scanning a Jira project
#[derive(Debug)]
pub struct ImportScan {
    pub project: String,
    pub domain: String,
    pub epics: Vec<EpicStub>,
    pub rfcs: Vec<RfcStub>,
    pub skipped: usize,
}

impl ImportScan {
    /// Scan a Jira project and produce proposed stubs
    pub fn run(
        tracker: &dyn IssueTracker,
        project: &str,
        domain: &str,
    ) -> Result<Self, super::TrackerError> {
        let issues = tracker.list_issues(project, None)?;

        let mut epics = Vec::new();
        let mut rfcs = Vec::new();
        let mut skipped = 0;

        // First pass: collect epics
        for issue in &issues {
            if issue.issue_type == IssueType::Epic {
                let children = tracker.list_issues(project, Some(&issue.key))?;
                epics.push(EpicStub {
                    jira_key: issue.key.clone(),
                    title: issue.summary.clone(),
                    status: map_status(&issue.status.category),
                    child_count: children.len(),
                });
            }
        }

        // Second pass: collect tasks as RFC stubs
        for issue in &issues {
            match &issue.issue_type {
                IssueType::Epic => continue, // already handled
                IssueType::Task | IssueType::Story | IssueType::Bug => {
                    rfcs.push(RfcStub {
                        jira_key: issue.key.clone(),
                        title: issue.summary.clone(),
                        status: map_status(&issue.status.category),
                        description: issue.description.clone(),
                        epic_key: issue.epic_key.clone(),
                    });
                }
                _ => {
                    skipped += 1;
                }
            }
        }

        Ok(ImportScan {
            project: project.to_string(),
            domain: domain.to_string(),
            epics,
            rfcs,
            skipped,
        })
    }

    /// Render the scan as a dry-run report
    pub fn render_report(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!(
            "# Import Scan: {} @ {}\n\n",
            self.project, self.domain
        ));

        // Epics
        out.push_str(&format!("## Epics ({})\n\n", self.epics.len()));
        for epic in &self.epics {
            out.push_str(&format!(
                "- **{}** — {} (status: {}, {} children)\n",
                epic.jira_key, epic.title, epic.status, epic.child_count
            ));
        }

        // RFC stubs
        out.push_str(&format!("\n## RFC Stubs ({})\n\n", self.rfcs.len()));
        for rfc in &self.rfcs {
            let epic_info = rfc
                .epic_key
                .as_deref()
                .map(|k| format!(" (epic: {})", k))
                .unwrap_or_default();
            out.push_str(&format!(
                "- **{}** — {} (status: {}){}\n",
                rfc.jira_key, rfc.title, rfc.status, epic_info
            ));
        }

        if self.skipped > 0 {
            out.push_str(&format!(
                "\n_{} issues skipped (subtasks, etc.)_\n",
                self.skipped
            ));
        }

        // Sample RFC stub
        if let Some(first) = self.rfcs.first() {
            out.push_str("\n## Sample RFC Stub\n\n```markdown\n");
            out.push_str(&render_rfc_stub(first));
            out.push_str("```\n");
        }

        out
    }
}

fn map_status(category: &StatusCategory) -> &'static str {
    match category {
        StatusCategory::ToDo => "draft",
        StatusCategory::InProgress => "draft",
        StatusCategory::Done => "accepted",
        StatusCategory::Unknown(_) => "draft",
    }
}

/// Render a single RFC stub as markdown
pub fn render_rfc_stub(stub: &RfcStub) -> String {
    let desc = stub.description.as_deref().unwrap_or("Imported from Jira.");

    let epic_line = stub
        .epic_key
        .as_deref()
        .map(|k| format!("| **Epic** | {} |\n", k))
        .unwrap_or_default();

    format!(
        r#"# RFC NNNN: {}

| | |
|---|---|
| **Status** | Imported |
| **Date** | {} |
| **Jira** | {} |
{}
---

## Summary

{}

## Test Plan

- [ ] TBD

---

*"Right then. Let's get to it."*

— Blue
"#,
        stub.title,
        chrono::Utc::now().format("%Y-%m-%d"),
        stub.jira_key,
        epic_line,
        desc,
    )
}

/// Render an epic YAML stub
pub fn render_epic_yaml(epic: &EpicStub) -> String {
    let slug = epic
        .title
        .to_lowercase()
        .replace(' ', "-")
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "");

    format!(
        r#"epic_id: {}
jira_key: {}
status: {}
created: {}
"#,
        slug,
        epic.jira_key,
        epic.status,
        chrono::Utc::now().format("%Y-%m-%d"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_rfc_stub_basic() {
        let stub = RfcStub {
            jira_key: "PROJ-42".to_string(),
            title: "User Authentication".to_string(),
            status: "draft",
            description: Some("Implement OAuth2 login".to_string()),
            epic_key: None,
        };

        let md = render_rfc_stub(&stub);
        assert!(md.contains("# RFC NNNN: User Authentication"));
        assert!(md.contains("| **Status** | Imported |"));
        assert!(md.contains("| **Jira** | PROJ-42 |"));
        assert!(md.contains("Implement OAuth2 login"));
        assert!(!md.contains("**Epic**"));
    }

    #[test]
    fn test_render_rfc_stub_with_epic() {
        let stub = RfcStub {
            jira_key: "PROJ-99".to_string(),
            title: "Backend API".to_string(),
            status: "draft",
            description: None,
            epic_key: Some("PROJ-10".to_string()),
        };

        let md = render_rfc_stub(&stub);
        assert!(md.contains("| **Epic** | PROJ-10 |"));
        assert!(md.contains("Imported from Jira.")); // default description
    }

    #[test]
    fn test_render_rfc_stub_no_description() {
        let stub = RfcStub {
            jira_key: "TEST-1".to_string(),
            title: "Minimal".to_string(),
            status: "accepted",
            description: None,
            epic_key: None,
        };

        let md = render_rfc_stub(&stub);
        assert!(md.contains("Imported from Jira."));
    }

    #[test]
    fn test_render_epic_yaml() {
        let epic = EpicStub {
            jira_key: "PROJ-50".to_string(),
            title: "User Auth Overhaul".to_string(),
            status: "active",
            child_count: 5,
        };

        let yaml = render_epic_yaml(&epic);
        assert!(yaml.contains("epic_id: user-auth-overhaul"));
        assert!(yaml.contains("jira_key: PROJ-50"));
        assert!(yaml.contains("status: active"));
        assert!(yaml.contains("created:"));
    }

    #[test]
    fn test_render_epic_yaml_special_chars() {
        let epic = EpicStub {
            jira_key: "TEST-1".to_string(),
            title: "Fix Bug #123 (Critical)".to_string(),
            status: "draft",
            child_count: 0,
        };

        let yaml = render_epic_yaml(&epic);
        // Special chars should be stripped from slug
        assert!(yaml.contains("epic_id: fix-bug-123-critical"));
    }

    #[test]
    fn test_import_scan_render_report() {
        let scan = ImportScan {
            project: "PROJ".to_string(),
            domain: "test.atlassian.net".to_string(),
            epics: vec![EpicStub {
                jira_key: "PROJ-1".to_string(),
                title: "Epic One".to_string(),
                status: "active",
                child_count: 2,
            }],
            rfcs: vec![
                RfcStub {
                    jira_key: "PROJ-10".to_string(),
                    title: "Task One".to_string(),
                    status: "draft",
                    description: None,
                    epic_key: Some("PROJ-1".to_string()),
                },
                RfcStub {
                    jira_key: "PROJ-11".to_string(),
                    title: "Task Two".to_string(),
                    status: "accepted",
                    description: None,
                    epic_key: None,
                },
            ],
            skipped: 1,
        };

        let report = scan.render_report();
        assert!(report.contains("# Import Scan: PROJ @ test.atlassian.net"));
        assert!(report.contains("## Epics (1)"));
        assert!(report.contains("**PROJ-1** — Epic One"));
        assert!(report.contains("## RFC Stubs (2)"));
        assert!(report.contains("**PROJ-10** — Task One"));
        assert!(report.contains("(epic: PROJ-1)"));
        assert!(report.contains("1 issues skipped"));
        assert!(report.contains("## Sample RFC Stub"));
    }

    #[test]
    fn test_map_status() {
        assert_eq!(map_status(&StatusCategory::ToDo), "draft");
        assert_eq!(map_status(&StatusCategory::InProgress), "draft");
        assert_eq!(map_status(&StatusCategory::Done), "accepted");
        assert_eq!(
            map_status(&StatusCategory::Unknown("custom".to_string())),
            "draft"
        );
    }
}
