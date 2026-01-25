//! Forgejo/Gitea forge implementation
//!
//! REST API client for Forgejo and Gitea instances.

use serde::{Deserialize, Serialize};

use super::{CreatePrOpts, Forge, ForgeError, ForgeType, MergeStrategy, PrState, PullRequest};

/// Forgejo/Gitea forge implementation
pub struct ForgejoForge {
    host: String,
    token: String,
    client: reqwest::blocking::Client,
}

impl ForgejoForge {
    /// Create a new Forgejo forge client
    pub fn new(host: &str, token: String) -> Self {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            host: host.to_string(),
            token,
            client,
        }
    }

    fn api_url(&self, path: &str) -> String {
        format!("https://{}/api/v1{}", self.host, path)
    }

    fn auth_header(&self) -> String {
        format!("token {}", self.token)
    }
}

impl Forge for ForgejoForge {
    fn create_pr(&self, opts: CreatePrOpts) -> Result<PullRequest, ForgeError> {
        let url = self.api_url(&format!("/repos/{}/{}/pulls", opts.owner, opts.repo));

        let body = ForgejoCreatePr {
            title: opts.title,
            body: opts.body,
            head: opts.head,
            base: opts.base,
        };

        let response = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
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

        let pr: ForgejoPr = response
            .json()
            .map_err(|e| ForgeError::Parse(e.to_string()))?;

        Ok(pr.into_pull_request(&self.host, &opts.owner, &opts.repo))
    }

    fn get_pr(&self, owner: &str, repo: &str, number: u64) -> Result<PullRequest, ForgeError> {
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}", owner, repo, number));

        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
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

        let pr: ForgejoPr = response
            .json()
            .map_err(|e| ForgeError::Parse(e.to_string()))?;

        Ok(pr.into_pull_request(&self.host, owner, repo))
    }

    fn merge_pr(&self, owner: &str, repo: &str, number: u64, strategy: MergeStrategy) -> Result<(), ForgeError> {
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number));

        let do_type = match strategy {
            MergeStrategy::Merge => "merge",
            MergeStrategy::Squash => "squash",
            MergeStrategy::Rebase => "rebase",
        };

        let body = ForgejoMergePr {
            do_type: do_type.to_string(),
            merge_message_field: None,
        };

        let response = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
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
        let url = self.api_url(&format!("/repos/{}/{}/pulls/{}/merge", owner, repo, number));

        let response = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .map_err(|e| ForgeError::Http(e.to_string()))?;

        // 204 = merged, 404 = not merged
        Ok(response.status().as_u16() == 204)
    }

    fn forge_type(&self) -> ForgeType {
        ForgeType::Forgejo
    }
}

// Forgejo API request/response types

#[derive(Debug, Serialize)]
struct ForgejoCreatePr {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    head: String,
    base: String,
}

#[derive(Debug, Serialize)]
struct ForgejoMergePr {
    #[serde(rename = "Do")]
    do_type: String,
    #[serde(rename = "MergeMessageField", skip_serializing_if = "Option::is_none")]
    merge_message_field: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForgejoPr {
    number: u64,
    title: String,
    state: String,
    merged: bool,
    head: ForgejoBranch,
    base: ForgejoBranch,
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ForgejoBranch {
    #[serde(rename = "ref")]
    ref_name: String,
}

impl ForgejoPr {
    fn into_pull_request(self, host: &str, owner: &str, repo: &str) -> PullRequest {
        let url = self.html_url.unwrap_or_else(|| {
            format!("https://{}/{}/{}/pulls/{}", host, owner, repo, self.number)
        });

        PullRequest {
            number: self.number,
            url,
            title: self.title,
            state: if self.state == "open" { PrState::Open } else { PrState::Closed },
            merged: self.merged,
            head: self.head.ref_name,
            base: self.base.ref_name,
        }
    }
}
