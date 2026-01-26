//! GitHub forge implementation
//!
//! REST API client for GitHub.

use serde::{Deserialize, Serialize};

use super::{CreatePrOpts, Forge, ForgeError, ForgeType, MergeStrategy, PrState, PullRequest};

/// GitHub forge implementation
pub struct GitHubForge {
    token: String,
    client: reqwest::blocking::Client,
}

impl GitHubForge {
    /// Create a new GitHub forge client
    pub fn new(token: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("blue-mcp/0.1")
            .build()
            .expect("Failed to create HTTP client");

        Self { token, client }
    }

    fn api_url(path: &str) -> String {
        format!("https://api.github.com{}", path)
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }
}

impl Forge for GitHubForge {
    fn create_pr(&self, opts: CreatePrOpts) -> Result<PullRequest, ForgeError> {
        let url = Self::api_url(&format!("/repos/{}/{}/pulls", opts.owner, opts.repo));

        let body = GitHubCreatePr {
            title: opts.title,
            body: opts.body,
            head: opts.head,
            base: opts.base,
            draft: opts.draft,
        };

        let response = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .map_err(|e| ForgeError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(ForgeError::Api {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let pr: GitHubPr = response
            .json()
            .map_err(|e| ForgeError::Parse(e.to_string()))?;

        Ok(pr.into_pull_request())
    }

    fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest, ForgeError> {
        let url = Self::api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number));

        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .map_err(|e| ForgeError::Http(e.to_string()))?;

        let status = response.status();
        if status.as_u16() == 404 {
            return Err(ForgeError::NotFound {
                owner: owner.to_string(),
                repo: repo.to_string(),
                number,
            });
        }

        if !status.is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(ForgeError::Api {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let pr: GitHubPr = response
            .json()
            .map_err(|e| ForgeError::Parse(e.to_string()))?;

        Ok(pr.into_pull_request())
    }

    fn merge_pr(&self, owner: &str, repo: &str, number: u64, strategy: MergeStrategy) -> Result<(), ForgeError> {
        let url = Self::api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number));

        let merge_method = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };

        let body = GitHubMergePr {
            merge_method: merge_method.to_string(),
        };

        let response = self.client
            .put(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&body)
            .send()
            .map_err(|e| ForgeError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(ForgeError::Api {
                status: status.as_u16(),
                message: error_text,
            });
        }

        Ok(())
    }

    fn pr_is_merged(&self, owner: &str, repo: &str, number: u64) -> Result<bool, ForgeError> {
        let url = Self::api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number));

        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .map_err(|e| ForgeError::Http(e.to_string()))?;

        // 204 = merged, 404 = not merged
        Ok(response.status().as_u16() == 204)
    }

    fn forge_type(&self) -> ForgeType {
        ForgeType::GitHub
    }
}

// GitHub API request/response types

#[derive(Debug, Serialize)]
struct GitHubCreatePr {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    head: String,
    base: String,
    draft: bool,
}

#[derive(Debug, Serialize)]
struct GitHubMergePr {
    merge_method: String,
}

#[derive(Debug, Deserialize)]
struct GitHubPr {
    number: u64,
    title: String,
    state: String,
    merged: Option<bool>,
    html_url: String,
    head: GitHubBranch,
    base: GitHubBranch,
}

#[derive(Debug, Deserialize)]
struct GitHubBranch {
    #[serde(rename = "ref")]
    ref_name: String,
}

impl GitHubPr {
    fn into_pull_request(self) -> PullRequest {
        PullRequest {
            number: self.number,
            url: self.html_url,
            title: self.title,
            state: if self.state == "open" { PrState::Open } else { PrState::Closed },
            merged: self.merged.unwrap_or(false),
            head: self.head.ref_name,
            base: self.base.ref_name,
        }
    }
}
