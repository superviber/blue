//! Org-level operations handler (RFC 0074)
//!
//! Provides aggregate status and cross-repo RFC listing
//! for orgs that have an org.yaml manifest.

use serde_json::{json, Value};
use std::path::Path;

use crate::handler_error::HandlerError;
use crate::org::OrgManifest;
use crate::pm::domain::PmDomain;

/// Scan all repos in org and return aggregate status
pub fn handle_org_status(org_root: &Path, manifest: &OrgManifest) -> Result<Value, HandlerError> {
    let pm_path = manifest.pm_repo_path(org_root);
    let domain_yaml = pm_path.join("domain.yaml");

    if !domain_yaml.exists() {
        return Err(HandlerError::NotFound(format!(
            "domain.yaml not found in PM repo at {}",
            pm_path.display()
        )));
    }

    let domain = PmDomain::load(&domain_yaml)
        .map_err(|e| HandlerError::Workflow(format!("Failed to load domain.yaml: {}", e)))?;

    let mut repo_statuses = Vec::new();

    for repo_entry in &domain.repos {
        let repo_path = org_root.join(&repo_entry.name);
        let exists = repo_path.exists();
        let has_blue = repo_path.join(".blue").exists();

        // Count RFCs if .blue/docs/rfcs exists
        let mut rfc_counts = json!({});
        if has_blue {
            let rfcs_path = repo_path.join(".blue").join("docs").join("rfcs");
            if rfcs_path.exists() {
                let (draft, approved, implemented) = count_rfcs(&rfcs_path);
                rfc_counts = json!({
                    "draft": draft,
                    "approved": approved,
                    "implemented": implemented,
                    "total": draft + approved + implemented
                });
            }
        }

        // Get git branch if repo exists
        let branch = if exists {
            current_branch(&repo_path)
        } else {
            None
        };

        repo_statuses.push(json!({
            "name": repo_entry.name,
            "description": repo_entry.description,
            "exists": exists,
            "has_blue": has_blue,
            "branch": branch,
            "rfcs": rfc_counts
        }));
    }

    // PM repo info
    let pm_info = json!({
        "path": pm_path.display().to_string(),
        "has_domain_yaml": true,
        "areas": domain.areas.iter().map(|a| json!({
            "key": a.key,
            "name": a.name,
            "repos": a.repos
        })).collect::<Vec<_>>(),
        "components": domain.components.iter().map(|c| &c.name).collect::<Vec<_>>(),
        "repo_count": domain.repos.len()
    });

    // Jira info
    let jira_info = json!({
        "domain": domain.jira_domain(),
        "project_key": domain.jira_project_key()
    });

    // Count epics
    let epics_path = pm_path.join("epics");
    let epic_count = if epics_path.exists() {
        std::fs::read_dir(&epics_path)
            .map(|entries| entries.flatten().filter(|e| e.path().is_dir()).count())
            .unwrap_or(0)
    } else {
        0
    };

    Ok(json!({
        "status": "success",
        "org": manifest.org,
        "pm_repo": manifest.pm_repo,
        "repos": repo_statuses,
        "pm": pm_info,
        "jira": jira_info,
        "epic_count": epic_count,
        "message": crate::voice::success(
            &format!("Org '{}': {} repos, {} areas, {} epics",
                manifest.org, domain.repos.len(), domain.areas.len(), epic_count),
            None
        )
    }))
}

/// List RFCs across all repos in the org
pub fn handle_org_rfc_list(
    org_root: &Path,
    manifest: &OrgManifest,
) -> Result<Value, HandlerError> {
    let pm_path = manifest.pm_repo_path(org_root);
    let domain_yaml = pm_path.join("domain.yaml");

    if !domain_yaml.exists() {
        return Err(HandlerError::NotFound("domain.yaml not found".into()));
    }

    let domain = PmDomain::load(&domain_yaml)
        .map_err(|e| HandlerError::Workflow(format!("Failed to load domain.yaml: {}", e)))?;

    let mut all_rfcs = Vec::new();

    for repo_entry in &domain.repos {
        let repo_path = org_root.join(&repo_entry.name);
        let rfcs_path = repo_path.join(".blue").join("docs").join("rfcs");

        if !rfcs_path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&rfcs_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.ends_with(".md") || name_str.ends_with(".plan.md") {
                    continue;
                }

                // Parse status from filename: NNNN-{D|A|I|S}-title.md
                let status = if name_str.contains("-D-") {
                    "draft"
                } else if name_str.contains("-A-") {
                    "approved"
                } else if name_str.contains("-I-") {
                    "implemented"
                } else if name_str.contains("-S-") {
                    "superseded"
                } else {
                    "unknown"
                };

                // Extract number and title from filename
                let parts: Vec<&str> = name_str.splitn(3, '-').collect();
                let number = parts
                    .first()
                    .and_then(|p| p.parse::<i64>().ok())
                    .unwrap_or(0);
                let title = if parts.len() >= 3 {
                    parts[2].trim_end_matches(".md").to_string()
                } else {
                    name_str.trim_end_matches(".md").to_string()
                };

                all_rfcs.push(json!({
                    "repo": repo_entry.name,
                    "number": number,
                    "status": status,
                    "title": title,
                    "file": name_str.to_string()
                }));
            }
        }
    }

    // Sort by number
    all_rfcs.sort_by(|a, b| {
        a.get("number")
            .and_then(|n| n.as_i64())
            .unwrap_or(0)
            .cmp(
                &b.get("number")
                    .and_then(|n| n.as_i64())
                    .unwrap_or(0),
            )
    });

    let total = all_rfcs.len();

    Ok(json!({
        "status": "success",
        "rfcs": all_rfcs,
        "total": total,
        "message": crate::voice::success(
            &format!("{} RFCs across org '{}'", total, manifest.org),
            None
        )
    }))
}

/// Search PM repo for epics/stories that might relate to an RFC
pub fn handle_org_link(
    org_root: &Path,
    manifest: &OrgManifest,
    args: &Value,
) -> Result<Value, HandlerError> {
    let repo_name = args
        .get("repo")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let rfc_file = args
        .get("rfc")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    // Read the RFC file
    let rfc_path = org_root
        .join(repo_name)
        .join(".blue")
        .join("docs")
        .join("rfcs")
        .join(rfc_file);
    if !rfc_path.exists() {
        return Err(HandlerError::NotFound(format!(
            "RFC not found: {}",
            rfc_path.display()
        )));
    }
    let rfc_content =
        std::fs::read_to_string(&rfc_path).map_err(|e| HandlerError::Workflow(e.to_string()))?;

    // Extract keywords from RFC title (first line) and problem section
    let rfc_title = rfc_content
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches("# ")
        .to_string();
    let keywords = extract_keywords(&rfc_title, &rfc_content);

    // Check if RFC already has Jira binding
    let existing_binding = crate::tracker::sync::parse_jira_binding(&rfc_content);

    // Scan epics/stories in PM repo
    let pm_path = manifest.pm_repo_path(org_root);
    let epics_path = pm_path.join("epics");
    let mut candidates = Vec::new();

    if epics_path.exists() {
        if let Ok(epic_dirs) = std::fs::read_dir(&epics_path) {
            for epic_entry in epic_dirs.flatten() {
                let epic_path = epic_entry.path();
                if !epic_path.is_dir() {
                    continue;
                }

                let epic_name = epic_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                // Read _epic.md for epic title
                let epic_md = epic_path.join("_epic.md");
                let epic_title = if epic_md.exists() {
                    std::fs::read_to_string(&epic_md)
                        .ok()
                        .and_then(|c| {
                            c.lines()
                                .next()
                                .map(|l| l.trim_start_matches("# ").to_string())
                        })
                        .unwrap_or_else(|| epic_name.clone())
                } else {
                    epic_name.clone()
                };

                // Scan stories in this epic
                if let Ok(stories) = std::fs::read_dir(&epic_path) {
                    for story_entry in stories.flatten() {
                        let story_path = story_entry.path();
                        let story_file = story_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("");
                        if !story_file.ends_with(".md") || story_file == "_epic.md" {
                            continue;
                        }

                        let story_content =
                            std::fs::read_to_string(&story_path).unwrap_or_default();
                        let story_title = story_content
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim_start_matches("# ")
                            .to_string();

                        // Score keyword match
                        let score = score_match(&keywords, &story_title, &story_content);
                        if score > 0 {
                            candidates.push(json!({
                                "epic": epic_name,
                                "epic_title": epic_title,
                                "story": story_file.trim_end_matches(".md"),
                                "story_title": story_title,
                                "score": score,
                                "path": story_path.display().to_string()
                            }));
                        }
                    }
                }

                // Also check epic-level match
                let epic_content = std::fs::read_to_string(&epic_md).unwrap_or_default();
                let epic_score = score_match(&keywords, &epic_title, &epic_content);
                if epic_score > 0 {
                    candidates.push(json!({
                        "epic": epic_name,
                        "epic_title": epic_title,
                        "story": null,
                        "story_title": null,
                        "score": epic_score,
                        "path": epic_md.display().to_string()
                    }));
                }
            }
        }
    }

    // Sort by score descending
    candidates.sort_by(|a, b| {
        b.get("score")
            .and_then(|s| s.as_i64())
            .unwrap_or(0)
            .cmp(&a.get("score").and_then(|s| s.as_i64()).unwrap_or(0))
    });

    // Limit to top 10
    candidates.truncate(10);

    Ok(json!({
        "status": "success",
        "rfc_title": rfc_title,
        "rfc_file": rfc_file,
        "repo": repo_name,
        "existing_binding": {
            "jira_key": existing_binding.task_key,
            "epic_id": existing_binding.epic_id
        },
        "candidates": candidates,
        "message": crate::voice::success(
            &format!("Found {} candidate matches for '{}'", candidates.len(), rfc_title),
            None
        )
    }))
}

/// Sync RFCs across all repos with PM/Jira, reporting drift
pub fn handle_org_sync(org_root: &Path, manifest: &OrgManifest) -> Result<Value, HandlerError> {
    let pm_path = manifest.pm_repo_path(org_root);
    let domain_yaml = pm_path.join("domain.yaml");

    if !domain_yaml.exists() {
        return Err(HandlerError::NotFound("domain.yaml not found".into()));
    }

    let domain = PmDomain::load(&domain_yaml)
        .map_err(|e| HandlerError::Workflow(format!("Failed to load domain.yaml: {}", e)))?;

    let mut all_rfcs = Vec::new();
    let mut linked: i64 = 0;
    let mut unlinked: i64 = 0;

    for repo_entry in &domain.repos {
        let repo_path = org_root.join(&repo_entry.name);
        let rfcs_path = repo_path.join(".blue").join("docs").join("rfcs");

        if !rfcs_path.exists() {
            continue;
        }

        if let Ok(entries) = std::fs::read_dir(&rfcs_path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if !name_str.ends_with(".md") || name_str.ends_with(".plan.md") {
                    continue;
                }

                // Parse status from filename
                let status = if name_str.contains("-D-") {
                    "draft"
                } else if name_str.contains("-A-") {
                    "approved"
                } else if name_str.contains("-I-") {
                    "implemented"
                } else if name_str.contains("-S-") {
                    "superseded"
                } else {
                    "unknown"
                };

                // Read file and check for Jira binding
                let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
                let binding = crate::tracker::sync::parse_jira_binding(&content);
                let title = content
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim_start_matches("# ")
                    .to_string();

                let has_jira = binding.task_key.is_some();
                if has_jira {
                    linked += 1;
                } else {
                    unlinked += 1;
                }

                all_rfcs.push(json!({
                    "repo": repo_entry.name,
                    "file": name_str.to_string(),
                    "title": title,
                    "status": status,
                    "jira_key": binding.task_key,
                    "epic_id": binding.epic_id,
                    "linked": has_jira
                }));
            }
        }
    }

    // Sort: unlinked first, then by repo
    all_rfcs.sort_by(|a, b| {
        let a_linked = a.get("linked").and_then(|l| l.as_bool()).unwrap_or(false);
        let b_linked = b.get("linked").and_then(|l| l.as_bool()).unwrap_or(false);
        a_linked
            .cmp(&b_linked)
            .then_with(|| {
                a.get("repo")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .cmp(b.get("repo").and_then(|r| r.as_str()).unwrap_or(""))
            })
    });

    let hint = if unlinked > 0 {
        Some(format!(
            "{} RFCs have no Jira ticket. Use 'blue org link' to connect them.",
            unlinked
        ))
    } else {
        None
    };

    Ok(json!({
        "status": "success",
        "rfcs": all_rfcs,
        "total": linked + unlinked,
        "linked": linked,
        "unlinked": unlinked,
        "jira": {
            "domain": domain.jira_domain(),
            "project_key": domain.jira_project_key()
        },
        "message": crate::voice::success(
            &format!("{} RFCs ({} linked, {} unlinked)", linked + unlinked, linked, unlinked),
            hint.as_deref()
        )
    }))
}

/// Extract keywords from RFC title and content
fn extract_keywords(title: &str, content: &str) -> Vec<String> {
    let stop_words: std::collections::HashSet<&str> = [
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do",
        "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall", "can",
        "need", "that", "this", "these", "those", "it", "its", "not", "no", "nor", "rfc", "status",
        "draft", "approved", "implemented",
    ]
    .iter()
    .copied()
    .collect();

    let mut keywords = Vec::new();

    // Extract from title
    for word in title.split_whitespace() {
        let clean = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if clean.len() > 2 && !stop_words.contains(clean.as_str()) {
            keywords.push(clean);
        }
    }

    // Extract from Problem section if present
    let mut in_problem = false;
    for line in content.lines() {
        if line.starts_with("## Problem") {
            in_problem = true;
            continue;
        }
        if in_problem && line.starts_with("## ") {
            break;
        }
        if in_problem {
            for word in line.split_whitespace() {
                let clean = word
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .to_lowercase();
                if clean.len() > 3
                    && !stop_words.contains(clean.as_str())
                    && !keywords.contains(&clean)
                {
                    keywords.push(clean);
                }
            }
        }
    }

    // Limit to top 20 keywords
    keywords.truncate(20);
    keywords
}

/// Score how well a story/epic matches the RFC keywords
fn score_match(keywords: &[String], title: &str, content: &str) -> i64 {
    let title_lower = title.to_lowercase();
    let content_lower = content.to_lowercase();
    let mut score: i64 = 0;

    for keyword in keywords {
        // Title match is worth more
        if title_lower.contains(keyword.as_str()) {
            score += 3;
        }
        // Content match
        if content_lower.contains(keyword.as_str()) {
            score += 1;
        }
    }

    score
}

/// Check cross-repo RFC dependencies for a given RFC.
/// Returns unmet dependencies (RFCs that are still Draft in their repos).
pub fn handle_check_cross_deps(
    org_root: &Path,
    manifest: &OrgManifest,
    args: &Value,
) -> Result<Value, HandlerError> {
    let repo_name = args
        .get("repo")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;
    let rfc_file = args
        .get("rfc")
        .and_then(|v| v.as_str())
        .ok_or(HandlerError::InvalidParams)?;

    let rfc_path = org_root
        .join(repo_name)
        .join(".blue")
        .join("docs")
        .join("rfcs")
        .join(rfc_file);
    if !rfc_path.exists() {
        return Err(HandlerError::NotFound(format!(
            "RFC not found: {}",
            rfc_path.display()
        )));
    }

    let content =
        std::fs::read_to_string(&rfc_path).map_err(|e| HandlerError::Workflow(e.to_string()))?;

    // Parse "Depends On" from front matter table
    let deps = parse_depends_on(&content);

    if deps.is_empty() {
        return Ok(json!({
            "status": "success",
            "rfc": rfc_file,
            "dependencies": [],
            "unmet": [],
            "all_met": true,
            "message": "No dependencies declared"
        }));
    }

    // Load domain.yaml to know all repos
    let pm_path = manifest.pm_repo_path(org_root);
    let domain_yaml = pm_path.join("domain.yaml");
    let domain = PmDomain::load(&domain_yaml)
        .map_err(|e| HandlerError::Workflow(format!("Failed to load domain.yaml: {}", e)))?;

    let mut dep_results = Vec::new();
    let mut unmet = Vec::new();

    for dep_number in &deps {
        let mut found = false;
        for repo_entry in &domain.repos {
            let rfcs_dir = org_root
                .join(&repo_entry.name)
                .join(".blue")
                .join("docs")
                .join("rfcs");
            if !rfcs_dir.exists() {
                continue;
            }

            if let Ok(entries) = std::fs::read_dir(&rfcs_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    let prefix = format!("{:04}-", dep_number);
                    if name_str.starts_with(&prefix)
                        && name_str.ends_with(".md")
                        && !name_str.ends_with(".plan.md")
                    {
                        let is_approved = name_str.contains("-A-");
                        let is_implemented = name_str.contains("-I-");
                        let met = is_approved || is_implemented;

                        let status = if name_str.contains("-D-") {
                            "draft"
                        } else if is_approved {
                            "approved"
                        } else if is_implemented {
                            "implemented"
                        } else if name_str.contains("-S-") {
                            "superseded"
                        } else {
                            "unknown"
                        };

                        dep_results.push(json!({
                            "number": dep_number,
                            "repo": repo_entry.name,
                            "file": name_str.to_string(),
                            "status": status,
                            "met": met
                        }));

                        if !met {
                            unmet.push(json!({
                                "number": dep_number,
                                "repo": repo_entry.name,
                                "file": name_str.to_string(),
                                "status": status
                            }));
                        }

                        found = true;
                        break;
                    }
                }
            }
            if found {
                break;
            }
        }

        if !found {
            dep_results.push(json!({
                "number": dep_number,
                "repo": null,
                "file": null,
                "status": "not_found",
                "met": false
            }));
            unmet.push(json!({
                "number": dep_number,
                "repo": null,
                "status": "not_found"
            }));
        }
    }

    let all_met = unmet.is_empty();

    Ok(json!({
        "status": "success",
        "rfc": rfc_file,
        "dependencies": dep_results,
        "unmet": unmet,
        "all_met": all_met,
        "message": if all_met {
            format!("All {} dependencies met", dep_results.len())
        } else {
            format!("{} of {} dependencies unmet", unmet.len(), dep_results.len())
        }
    }))
}

/// Parse RFC numbers from "Depends On" front matter.
/// Handles formats like:
///   | Depends On | RFC 0067, RFC 0073 |
///   | Depends On | RFC 0063/0068/0070 (PM + Jira) |
fn parse_depends_on(content: &str) -> Vec<i32> {
    let re_simple = regex::Regex::new(r"RFC\s+(\d+)").unwrap();
    let re_slash = regex::Regex::new(r"(\d{4})/(\d{4})").unwrap();
    let mut deps = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        // Match: | Depends On | RFC 0067, RFC 0073 |
        // or: | **Depends On** | ... |
        if trimmed.starts_with('|')
            && (trimmed.contains("Depends On") || trimmed.contains("depends on"))
        {
            let parts: Vec<&str> = trimmed.split('|').collect();
            if parts.len() >= 3 {
                let value = parts[2].trim();
                // Extract RFC numbers: "RFC 0067", "RFC 0063"
                for cap in re_simple.captures_iter(value) {
                    if let Ok(n) = cap[1].parse::<i32>() {
                        deps.push(n);
                    }
                }
                // Handle slash-separated: "0063/0068/0070"
                for cap in re_slash.captures_iter(value) {
                    if let Ok(n) = cap[2].parse::<i32>() {
                        if !deps.contains(&n) {
                            deps.push(n);
                        }
                    }
                }
            }
            break;
        }
    }
    deps
}

/// Count RFCs by status in a directory
fn count_rfcs(rfcs_path: &Path) -> (usize, usize, usize) {
    let mut draft = 0;
    let mut approved = 0;
    let mut implemented = 0;

    if let Ok(entries) = std::fs::read_dir(rfcs_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.ends_with(".md") || name_str.ends_with(".plan.md") {
                continue;
            }
            if name_str.contains("-D-") {
                draft += 1;
            } else if name_str.contains("-A-") {
                approved += 1;
            } else if name_str.contains("-I-") {
                implemented += 1;
            }
        }
    }

    (draft, approved, implemented)
}

/// Get current git branch for a repo
fn current_branch(repo_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_path).ok()?;
    let head = repo.head().ok()?;
    head.shorthand().map(|s| s.to_string())
}
