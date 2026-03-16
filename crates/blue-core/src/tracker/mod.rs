//! Issue Tracker Integration (RFC 0063)
//!
//! Provides a unified interface for interacting with external issue trackers
//! (Jira Cloud, future: Linear, Shortcut) for project management operations.
//!
//! Design: Git is the sole authority. Trackers are write-through projections.

pub mod credentials;
pub mod import;
mod jira;
pub mod lint;
pub mod sync;

pub use credentials::CredentialStore;
pub use import::ImportScan;
pub use jira::JiraCloudTracker;
pub use lint::{check_for_jira_credentials, LintSeverity, LintWarning};
pub use sync::{
    parse_jira_binding, parse_rfc_status, parse_rfc_title, rfc_status_to_jira, run_sync,
    update_jira_binding, DriftPolicy, DriftReport, JiraBinding, SyncAction, SyncConfig,
    SyncReport, SyncResult,
};

use serde::{Deserialize, Serialize};

/// Supported tracker types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackerType {
    Jira,
}

impl std::fmt::Display for TrackerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrackerType::Jira => write!(f, "jira"),
        }
    }
}

/// Tracker errors
#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
    #[error("HTTP request failed: {0}")]
    Http(String),

    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },

    #[error("Missing credentials: {0}")]
    MissingCredentials(String),

    #[error("Failed to parse response: {0}")]
    Parse(String),

    #[error("Issue not found: {key}")]
    NotFound { key: String },

    #[error("Project not found: {project}")]
    ProjectNotFound { project: String },
}

/// Issue type in the tracker
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueType {
    Epic,
    Task,
    Subtask,
    Story,
    Bug,
    Other(String),
}

impl std::fmt::Display for IssueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueType::Epic => write!(f, "Epic"),
            IssueType::Task => write!(f, "Task"),
            IssueType::Subtask => write!(f, "Sub-task"),
            IssueType::Story => write!(f, "Story"),
            IssueType::Bug => write!(f, "Bug"),
            IssueType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Issue status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueStatus {
    pub name: String,
    pub category: StatusCategory,
}

/// Jira status categories (maps to Jira's statusCategory)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusCategory {
    ToDo,
    InProgress,
    Done,
    Unknown(String),
}

/// An issue from the tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    pub issue_type: IssueType,
    pub status: IssueStatus,
    pub assignee: Option<String>,
    pub epic_key: Option<String>,
    pub labels: Vec<String>,
    pub description: Option<String>,
}

/// Options for creating an issue
#[derive(Debug, Clone)]
pub struct CreateIssueOpts {
    pub project: String,
    pub issue_type: IssueType,
    pub summary: String,
    pub description: Option<String>,
    pub epic_key: Option<String>,
    pub labels: Vec<String>,
    pub components: Vec<String>,
}

/// Options for creating a project
#[derive(Debug, Clone)]
pub struct CreateProjectOpts {
    pub key: String,
    pub name: String,
    /// "software" for Jira Software, "business" for Jira Work Management
    pub project_type: String,
    /// Optional lead account ID
    pub lead_account_id: Option<String>,
}

/// Options for transitioning an issue
#[derive(Debug, Clone)]
pub struct TransitionOpts {
    pub key: String,
    pub target_status: String,
}

/// Project info from the tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerProject {
    pub key: String,
    pub name: String,
    pub project_type: String,
}

/// Auth status for a tracker connection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub user: Option<String>,
    pub email: Option<String>,
    pub domain: String,
}

/// The IssueTracker trait — unified interface for external trackers
pub trait IssueTracker: Send + Sync {
    /// Check authentication status
    fn auth_status(&self) -> Result<AuthStatus, TrackerError>;

    /// List projects accessible to the authenticated user
    fn list_projects(&self) -> Result<Vec<TrackerProject>, TrackerError>;

    /// Create an issue
    fn create_issue(&self, opts: CreateIssueOpts) -> Result<Issue, TrackerError>;

    /// Get an issue by key
    fn get_issue(&self, key: &str) -> Result<Issue, TrackerError>;

    /// List issues in a project, optionally filtered by epic
    fn list_issues(
        &self,
        project: &str,
        epic_key: Option<&str>,
    ) -> Result<Vec<Issue>, TrackerError>;

    /// Transition an issue to a new status
    fn transition_issue(&self, opts: TransitionOpts) -> Result<(), TrackerError>;

    /// Delete an issue by key (used for test cleanup)
    fn delete_issue(&self, key: &str) -> Result<(), TrackerError>;

    /// Create a project. Returns Err if permissions are insufficient.
    fn create_project(&self, opts: CreateProjectOpts) -> Result<TrackerProject, TrackerError>;

    /// Check if a project exists by key
    fn project_exists(&self, project_key: &str) -> Result<bool, TrackerError> {
        match self.list_projects() {
            Ok(projects) => Ok(projects.iter().any(|p| p.key == project_key)),
            Err(_) => Ok(false),
        }
    }

    /// Get the tracker type
    fn tracker_type(&self) -> TrackerType;
}

/// Configuration for connecting to a tracker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerConfig {
    pub tracker_type: TrackerType,
    pub domain: String,
    pub project_key: String,
}

/// Credentials for a tracker connection
#[derive(Debug, Clone)]
pub struct TrackerCredentials {
    pub email: String,
    pub token: String,
}

/// Create a tracker instance from environment variables
pub fn create_tracker_from_env(
    domain: &str,
    email: &str,
    token: &str,
) -> Result<Box<dyn IssueTracker>, TrackerError> {
    // Currently only Jira Cloud is supported
    Ok(Box::new(JiraCloudTracker::new(
        domain.to_string(),
        email.to_string(),
        token.to_string(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_type_display() {
        assert_eq!(TrackerType::Jira.to_string(), "jira");
    }

    #[test]
    fn test_issue_type_display() {
        assert_eq!(IssueType::Epic.to_string(), "Epic");
        assert_eq!(IssueType::Task.to_string(), "Task");
        assert_eq!(IssueType::Other("Custom".into()).to_string(), "Custom");
    }
}
