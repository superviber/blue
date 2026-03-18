//! Jira Cloud tracker implementation
//!
//! REST API client for Jira Cloud (v3 API).
//! Auth: Basic auth with email + API token.

use serde::Deserialize;

use super::{
    AuthStatus, CreateIssueOpts, Issue, IssueStatus, IssueTracker, IssueType, StatusCategory,
    TrackerError, TrackerProject, TrackerType, TransitionOpts,
};

/// Jira Cloud tracker implementation
pub struct JiraCloudTracker {
    domain: String,
    email: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl JiraCloudTracker {
    /// Create a new Jira Cloud tracker client
    pub fn new(domain: String, email: String, token: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("blue/0.1")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            domain,
            email,
            token,
            client,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://{}/rest/api/3{}", self.domain, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::blocking::RequestBuilder {
        self.client
            .request(method, self.api_url(path))
            .basic_auth(&self.email, Some(&self.token))
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
    }

    fn get(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.request(reqwest::Method::GET, path)
    }

    fn post(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.request(reqwest::Method::POST, path)
    }

    fn delete_req(&self, path: &str) -> reqwest::blocking::RequestBuilder {
        self.request(reqwest::Method::DELETE, path)
    }

    fn check_response(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<reqwest::blocking::Response, TrackerError> {
        let status = response.status();
        if status.is_success() {
            Ok(response)
        } else if status.as_u16() == 404 {
            let text = response.text().unwrap_or_default();
            Err(TrackerError::NotFound { key: text })
        } else if status.as_u16() == 401 {
            Err(TrackerError::MissingCredentials(
                "Invalid or expired API token".to_string(),
            ))
        } else {
            let error_text = response.text().unwrap_or_default();
            Err(TrackerError::Api {
                status: status.as_u16(),
                message: error_text,
            })
        }
    }
}

impl IssueTracker for JiraCloudTracker {
    fn auth_status(&self) -> Result<AuthStatus, TrackerError> {
        let response = self
            .get("/myself")
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let user: JiraUser = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        Ok(AuthStatus {
            authenticated: true,
            user: Some(user.display_name),
            email: Some(user.email_address.unwrap_or_default()),
            domain: self.domain.clone(),
        })
    }

    fn list_projects(&self) -> Result<Vec<TrackerProject>, TrackerError> {
        let response = self
            .get("/project")
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let projects: Vec<JiraProject> = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        Ok(projects
            .into_iter()
            .map(|p| TrackerProject {
                key: p.key,
                name: p.name,
                project_type: p.project_type_key.unwrap_or_else(|| "unknown".to_string()),
            })
            .collect())
    }

    fn create_issue(&self, opts: CreateIssueOpts) -> Result<Issue, TrackerError> {
        let issue_type_name = opts.issue_type.to_string();

        let mut fields = serde_json::json!({
            "project": { "key": opts.project },
            "summary": opts.summary,
            "issuetype": { "name": issue_type_name },
        });

        if let Some(desc) = &opts.description {
            // Jira v3 uses Atlassian Document Format (ADF) for description
            fields["description"] = serde_json::json!({
                "type": "doc",
                "version": 1,
                "content": [{
                    "type": "paragraph",
                    "content": [{
                        "type": "text",
                        "text": desc
                    }]
                }]
            });
        }

        if let Some(epic_key) = &opts.epic_key {
            // Team-managed projects use parent field for epic linkage
            fields["parent"] = serde_json::json!({ "key": epic_key });
        }

        if !opts.labels.is_empty() {
            fields["labels"] = serde_json::json!(opts.labels);
        }

        if !opts.components.is_empty() {
            let components: Vec<_> = opts.components.iter()
                .map(|name| serde_json::json!({"name": name}))
                .collect();
            fields["components"] = serde_json::json!(components);
        }

        let body = serde_json::json!({ "fields": fields });

        let response = self
            .post("/issue")
            .json(&body)
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let created: JiraCreatedIssue = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        // Fetch the full issue to return complete data
        self.get_issue(&created.key)
    }

    fn get_issue(&self, key: &str) -> Result<Issue, TrackerError> {
        let response = self
            .get(&format!("/issue/{}", key))
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        if response.status().as_u16() == 404 {
            return Err(TrackerError::NotFound {
                key: key.to_string(),
            });
        }

        let response = self.check_response(response)?;

        let issue: JiraIssue = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        Ok(issue.into_issue())
    }

    fn list_issues(
        &self,
        project: &str,
        epic_key: Option<&str>,
    ) -> Result<Vec<Issue>, TrackerError> {
        let jql = match epic_key {
            Some(epic) => format!("project = {} AND parent = {}", project, epic),
            None => format!("project = {} ORDER BY created DESC", project),
        };

        // Jira deprecated /rest/api/3/search — use /rest/api/3/search/jql
        let body = serde_json::json!({
            "jql": jql,
            "maxResults": 100,
            "fields": ["summary", "issuetype", "status", "assignee", "parent", "labels", "description"],
        });

        let response = self
            .post("/search/jql")
            .json(&body)
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let result: JiraSearchResult = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        Ok(result.issues.into_iter().map(|i| i.into_issue()).collect())
    }

    fn transition_issue(&self, opts: TransitionOpts) -> Result<(), TrackerError> {
        // First, get available transitions
        let response = self
            .get(&format!("/issue/{}/transitions", opts.key))
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let transitions: JiraTransitions = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        // Find the transition matching the target status
        let transition = transitions
            .transitions
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(&opts.target_status))
            .ok_or_else(|| TrackerError::Api {
                status: 400,
                message: format!(
                    "No transition to '{}' available. Available: {}",
                    opts.target_status,
                    transitions
                        .transitions
                        .iter()
                        .map(|t| t.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            })?;

        let body = serde_json::json!({
            "transition": { "id": transition.id }
        });

        let response = self
            .post(&format!("/issue/{}/transitions", opts.key))
            .json(&body)
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        self.check_response(response)?;
        Ok(())
    }

    fn delete_issue(&self, key: &str) -> Result<(), TrackerError> {
        let response = self
            .delete_req(&format!("/issue/{}", key))
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        self.check_response(response)?;
        Ok(())
    }

    fn create_project(
        &self,
        opts: super::CreateProjectOpts,
    ) -> Result<TrackerProject, TrackerError> {
        // Get lead account ID — use provided or fall back to current user
        let lead_account_id = if let Some(id) = opts.lead_account_id {
            id
        } else {
            let response = self
                .get("/myself")
                .send()
                .map_err(|e| TrackerError::Http(e.to_string()))?;
            let response = self.check_response(response)?;
            let user: serde_json::Value = response
                .json()
                .map_err(|e| TrackerError::Parse(e.to_string()))?;
            user["accountId"]
                .as_str()
                .ok_or_else(|| TrackerError::Parse("No accountId in /myself".to_string()))?
                .to_string()
        };

        let body = serde_json::json!({
            "key": opts.key,
            "name": opts.name,
            "projectTypeKey": opts.project_type,
            "leadAccountId": lead_account_id,
        });

        let response = self
            .post("/project")
            .json(&body)
            .send()
            .map_err(|e| TrackerError::Http(e.to_string()))?;

        let response = self.check_response(response)?;

        let created: JiraProject = response
            .json()
            .map_err(|e| TrackerError::Parse(e.to_string()))?;

        Ok(TrackerProject {
            key: created.key,
            name: created.name,
            project_type: created.project_type_key.unwrap_or(opts.project_type),
        })
    }

    fn tracker_type(&self) -> TrackerType {
        TrackerType::Jira
    }
}

// Jira API response types

#[derive(Debug, Deserialize)]
struct JiraUser {
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "emailAddress")]
    email_address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JiraProject {
    key: String,
    name: String,
    #[serde(rename = "projectTypeKey")]
    project_type_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JiraCreatedIssue {
    key: String,
}

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraIssueFields,
}

#[derive(Debug, Deserialize)]
struct JiraIssueFields {
    summary: String,
    #[serde(rename = "issuetype")]
    issue_type: JiraIssueType,
    status: JiraStatus,
    assignee: Option<JiraAssignee>,
    parent: Option<JiraParent>,
    labels: Option<Vec<String>>,
    description: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct JiraIssueType {
    name: String,
}

#[derive(Debug, Deserialize)]
struct JiraStatus {
    name: String,
    #[serde(rename = "statusCategory")]
    status_category: JiraStatusCategory,
}

#[derive(Debug, Deserialize)]
struct JiraStatusCategory {
    key: String,
}

#[derive(Debug, Deserialize)]
struct JiraAssignee {
    #[serde(rename = "displayName")]
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct JiraParent {
    key: String,
}

#[derive(Debug, Deserialize)]
struct JiraSearchResult {
    issues: Vec<JiraIssue>,
}

#[derive(Debug, Deserialize)]
struct JiraTransitions {
    transitions: Vec<JiraTransition>,
}

#[derive(Debug, Deserialize)]
struct JiraTransition {
    id: String,
    name: String,
}

impl JiraIssue {
    fn into_issue(self) -> Issue {
        let issue_type = match self.fields.issue_type.name.to_lowercase().as_str() {
            "epic" => IssueType::Epic,
            "task" => IssueType::Task,
            "sub-task" | "subtask" => IssueType::Subtask,
            "story" => IssueType::Story,
            "bug" => IssueType::Bug,
            other => IssueType::Other(other.to_string()),
        };

        let status_category = match self.fields.status.status_category.key.as_str() {
            "new" => StatusCategory::ToDo,
            "indeterminate" => StatusCategory::InProgress,
            "done" => StatusCategory::Done,
            other => StatusCategory::Unknown(other.to_string()),
        };

        // Extract plain text from ADF description if present
        let description = self.fields.description.and_then(|d| extract_adf_text(&d));

        Issue {
            key: self.key,
            summary: self.fields.summary,
            issue_type,
            status: IssueStatus {
                name: self.fields.status.name,
                category: status_category,
            },
            assignee: self.fields.assignee.map(|a| a.display_name),
            epic_key: self.fields.parent.map(|p| p.key),
            labels: self.fields.labels.unwrap_or_default(),
            description,
        }
    }
}

/// Extract plain text from Atlassian Document Format (ADF)
fn extract_adf_text(adf: &serde_json::Value) -> Option<String> {
    if let Some(content) = adf.get("content").and_then(|c| c.as_array()) {
        let mut text_parts = Vec::new();
        for block in content {
            if let Some(inner) = block.get("content").and_then(|c| c.as_array()) {
                for node in inner {
                    if let Some(text) = node.get("text").and_then(|t| t.as_str()) {
                        text_parts.push(text.to_string());
                    }
                }
            }
        }
        if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join("\n"))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to get test config from env vars. Returns None if not set.
    fn jira_test_config() -> Option<(String, String, String, String)> {
        Some((
            std::env::var("BLUE_JIRA_TEST_DOMAIN").ok()?,
            std::env::var("BLUE_JIRA_TEST_EMAIL").ok()?,
            std::env::var("BLUE_JIRA_TEST_TOKEN").ok()?,
            std::env::var("BLUE_JIRA_TEST_PROJECT").ok()?,
        ))
    }

    fn create_test_tracker() -> Option<(JiraCloudTracker, String)> {
        let (domain, email, token, project) = jira_test_config()?;
        Some((JiraCloudTracker::new(domain, email, token), project))
    }

    #[test]
    fn test_adf_extraction() {
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": [{
                "type": "paragraph",
                "content": [{
                    "type": "text",
                    "text": "Hello world"
                }]
            }]
        });
        assert_eq!(extract_adf_text(&adf), Some("Hello world".to_string()));
    }

    #[test]
    fn test_adf_extraction_empty() {
        let adf = serde_json::json!({
            "type": "doc",
            "version": 1,
            "content": []
        });
        assert_eq!(extract_adf_text(&adf), None);
    }

    // --- E2E Tests (skip when env vars not set) ---
    //
    // These tests create issues prefixed with `blue-e2e-{run_id}-` for identification.
    // The test account may not have delete permissions, so created issues are left in place.
    // They can be cleaned up manually via JQL: summary ~ "blue-e2e-*"

    #[test]
    fn e2e_auth_status() {
        let Some((tracker, _project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let status = tracker.auth_status().expect("auth_status failed");
        assert!(status.authenticated);
        assert!(status.user.is_some());
        eprintln!("Authenticated as: {:?}", status.user);
    }

    #[test]
    fn e2e_list_projects() {
        let Some((tracker, project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let projects = tracker.list_projects().expect("list_projects failed");
        assert!(
            projects.iter().any(|p| p.key == project),
            "Expected project {} in list: {:?}",
            project,
            projects
        );
    }

    #[test]
    fn e2e_create_and_get_issue() {
        let Some((tracker, project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let summary = format!("blue-e2e-{}-test-task", run_id);

        // Create
        let issue = tracker
            .create_issue(CreateIssueOpts {
                project: project.clone(),
                issue_type: super::super::IssueType::Task,
                summary: summary.clone(),
                description: Some("Created by Blue e2e test".to_string()),
                epic_key: None,
                labels: vec!["blue-e2e".to_string()],
                components: vec![],
            })
            .expect("create_issue failed");

        assert!(issue.key.starts_with(&format!("{}-", project)));
        assert_eq!(issue.summary, summary);
        eprintln!("Created issue: {}", issue.key);

        // Get
        let fetched = tracker.get_issue(&issue.key).expect("get_issue failed");
        assert_eq!(fetched.key, issue.key);
        assert_eq!(fetched.summary, summary);
    }

    #[test]
    fn e2e_list_issues() {
        let Some((tracker, project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let issues = tracker
            .list_issues(&project, None)
            .expect("list_issues failed");

        eprintln!("Found {} issues in project {}", issues.len(), project);
    }

    #[test]
    fn e2e_create_epic_with_child() {
        let Some((tracker, project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Create epic
        let epic = tracker
            .create_issue(CreateIssueOpts {
                project: project.clone(),
                issue_type: super::super::IssueType::Epic,
                summary: format!("blue-e2e-{}-epic", run_id),
                description: Some("E2e test epic".to_string()),
                epic_key: None,
                labels: vec!["blue-e2e".to_string()],
                components: vec![],
            })
            .expect("create epic failed");

        eprintln!("Created epic: {}", epic.key);

        // Create child task under epic
        let task = tracker
            .create_issue(CreateIssueOpts {
                project: project.clone(),
                issue_type: super::super::IssueType::Task,
                summary: format!("blue-e2e-{}-child-task", run_id),
                description: Some("Child of e2e epic".to_string()),
                epic_key: Some(epic.key.clone()),
                labels: vec!["blue-e2e".to_string()],
                components: vec![],
            })
            .expect("create child task failed");

        eprintln!("Created child task: {} (parent: {})", task.key, epic.key);

        // Verify parent linkage
        assert_eq!(task.epic_key.as_deref(), Some(epic.key.as_str()));

        // List children of epic
        let children = tracker
            .list_issues(&project, Some(&epic.key))
            .expect("list epic children failed");
        assert!(
            children.iter().any(|i| i.key == task.key),
            "Child task {} not found under epic {}. Found: {:?}",
            task.key,
            epic.key,
            children.iter().map(|i| &i.key).collect::<Vec<_>>()
        );
    }

    #[test]
    fn e2e_transition_issue() {
        let Some((tracker, project)) = create_test_tracker() else {
            eprintln!("Skipping: BLUE_JIRA_TEST_* env vars not set");
            return;
        };

        let run_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();

        // Create task
        let issue = tracker
            .create_issue(CreateIssueOpts {
                project: project.clone(),
                issue_type: super::super::IssueType::Task,
                summary: format!("blue-e2e-{}-transition", run_id),
                description: None,
                epic_key: None,
                labels: vec!["blue-e2e".to_string()],
                components: vec![],
            })
            .expect("create issue failed");

        eprintln!(
            "Created issue: {} (status: {})",
            issue.key, issue.status.name
        );

        // Transition to "In Progress"
        let transition_result = tracker.transition_issue(TransitionOpts {
            key: issue.key.clone(),
            target_status: "In Progress".to_string(),
        });

        match transition_result {
            Ok(()) => {
                let updated = tracker.get_issue(&issue.key).expect("get after transition");
                eprintln!("Transitioned to: {}", updated.status.name);
                assert_ne!(
                    updated.status.name, issue.status.name,
                    "Status should have changed"
                );
            }
            Err(e) => {
                // Custom workflows may not have "In Progress" — not a test failure
                eprintln!("Transition not available (custom workflow?): {}", e);
            }
        }
    }
}
