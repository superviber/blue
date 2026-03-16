//! E2E tests for PM module (RFC 0068)
//!
//! Tests the full workflow: domain.yaml, PM repo structure, ID generation,
//! and Jira collision checking.
//!
//! Jira-dependent tests require BLUE_JIRA_TEST_* env vars and skip otherwise.

use blue_core::pm::domain::{PmDomain, RepoEntry};
use blue_core::pm::id::{
    format_epic_id, format_story_id, next_epic_id, next_story_id, parse_id,
};
use std::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sample_domain() -> PmDomain {
    PmDomain {
        org: "the-move-social".to_string(),
        key: "TMS".to_string(),
        domain: Some("themovesocial.atlassian.net".to_string()),
        project_key: Some("SCRUM".to_string()),
        drift_policy: "warn".to_string(),
        jira: None,
        components: vec![],
        areas: vec![],
        repos: vec![
            RepoEntry {
                name: "themove-backend".to_string(),
                key: Some("BKD".to_string()),
                url: Some("git@github.com:the-move-social/themove-backend.git".to_string()),
                description: Some(
                    "Backend API services — REST endpoints, auth, database".to_string(),
                ),
            },
            RepoEntry {
                name: "themove-frontend".to_string(),
                key: Some("FRD".to_string()),
                url: Some("git@github.com:the-move-social/themove-frontend.git".to_string()),
                description: Some(
                    "React web application — UI components, routing, state".to_string(),
                ),
            },
            RepoEntry {
                name: "themove-product".to_string(),
                key: Some("PRD".to_string()),
                url: None,
                description: Some("Product specs, feature docs, user research".to_string()),
            },
            RepoEntry {
                name: "project-management".to_string(),
                key: Some("PM".to_string()),
                url: None,
                description: Some(
                    "Project management — epics, stories, sprints, releases".to_string(),
                ),
            },
        ],
    }
}

/// Scaffold a realistic PM repo structure in a temp directory
fn scaffold_pm_repo(root: &std::path::Path, domain: &PmDomain) {
    // domain.yaml
    domain.save(&root.join("domain.yaml")).unwrap();

    // jira.toml
    fs::write(
        root.join("jira.toml"),
        r#"provider = "jira-cloud"

[link_types]
depends_on = "Blocks"
relates_to = "Relates"

[status_map]
backlog = "To Do"
ready = "To Do"
in-progress = "In Progress"
in-review = "In Progress"
done = "Done"
blocked = "In Progress"
"#,
    )
    .unwrap();

    // Epic directories with stories
    let epic1 = root.join("epics").join("TMS-01-party-system");
    fs::create_dir_all(&epic1).unwrap();
    fs::write(
        epic1.join("_epic.md"),
        r#"---
type: epic
id: TMS-01
title: "Party System"
status: backlog
priority: 1
labels: [phase-0, core-social]
release: phase-0-mvp
---
"#,
    )
    .unwrap();
    fs::write(
        epic1.join("BKD-001-create-party-api.md"),
        r#"---
type: story
id: BKD-001
title: "Create Party API endpoint"
epic: TMS-01
repo: themove-backend
status: backlog
points: 3
sprint: s01
---
"#,
    )
    .unwrap();
    fs::write(
        epic1.join("BKD-002-party-invites-api.md"),
        r#"---
type: story
id: BKD-002
title: "Party Invites API"
epic: TMS-01
repo: themove-backend
status: backlog
points: 2
sprint: s01
---
"#,
    )
    .unwrap();
    fs::write(
        epic1.join("FRD-001-create-party-ui.md"),
        r#"---
type: story
id: FRD-001
title: "Create Party UI component"
epic: TMS-01
repo: themove-frontend
status: backlog
points: 5
sprint: s02
depends_on:
  - BKD-001
---
"#,
    )
    .unwrap();

    let epic2 = root.join("epics").join("TMS-02-move-discovery");
    fs::create_dir_all(&epic2).unwrap();
    fs::write(
        epic2.join("_epic.md"),
        r#"---
type: epic
id: TMS-02
title: "Move Discovery"
status: backlog
priority: 2
labels: [phase-1, ai]
---
"#,
    )
    .unwrap();
    fs::write(
        epic2.join("BKD-003-ai-move-generation.md"),
        r#"---
type: story
id: BKD-003
title: "AI Move Generation"
epic: TMS-02
repo: themove-backend
status: backlog
points: 8
sprint: s03
---
"#,
    )
    .unwrap();
}

// ---------------------------------------------------------------------------
// E2E: domain.yaml lifecycle
// ---------------------------------------------------------------------------

#[test]
fn e2e_domain_yaml_full_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("domain.yaml");

    // Create
    let domain = sample_domain();
    domain.save(&path).unwrap();

    // Load and verify
    let loaded = PmDomain::load(&path).unwrap();
    assert_eq!(loaded.org, "the-move-social");
    assert_eq!(loaded.key, "TMS");
    assert_eq!(loaded.repos.len(), 4);

    // Lookup by name
    let bkd = loaded.find_repo("themove-backend").unwrap();
    assert_eq!(bkd.key.as_deref(), Some("BKD"));
    assert!(bkd.description.as_ref().unwrap().contains("Backend"));

    // Lookup by key
    let frd = loaded.find_repo_by_key("FRD").unwrap();
    assert_eq!(frd.name, "themove-frontend");

    // Add new repo
    let mut domain = loaded;
    let added = domain
        .upsert_repo(RepoEntry {
            name: "themove-infra".to_string(),
            key: Some("INF".to_string()),
            url: None,
            description: Some("Infrastructure and DevOps".to_string()),
        })
        .unwrap();
    assert!(added);
    assert_eq!(domain.repos.len(), 5);

    // Update existing repo
    let updated = domain
        .upsert_repo(RepoEntry {
            name: "themove-backend".to_string(),
            key: Some("BKD".to_string()),
            url: Some("git@github.com:the-move-social/themove-backend.git".to_string()),
            description: Some("Backend API — updated description".to_string()),
        })
        .unwrap();
    assert!(!updated);
    assert_eq!(domain.repos.len(), 5);

    // Save updated and verify roundtrip
    domain.save(&path).unwrap();
    let reloaded = PmDomain::load(&path).unwrap();
    assert_eq!(reloaded.repos.len(), 5);
    assert!(reloaded
        .find_repo("themove-backend")
        .unwrap()
        .description
        .as_ref()
        .unwrap()
        .contains("updated"));
}

#[test]
fn e2e_domain_yaml_validation_errors() {
    let dir = tempfile::tempdir().unwrap();

    // Duplicate key should fail on save
    let domain = PmDomain {
        org: "test".to_string(),
        key: "TST".to_string(),
        domain: None,
        project_key: None,
        drift_policy: "warn".to_string(),
        jira: None,
        components: vec![],
        areas: vec![],
        repos: vec![
            RepoEntry {
                name: "repo-a".to_string(),
                key: Some("DUP".to_string()),
                url: None,
                description: None,
            },
            RepoEntry {
                name: "repo-b".to_string(),
                key: Some("DUP".to_string()),
                url: None,
                description: None,
            },
        ],
    };
    assert!(domain.save(&dir.path().join("domain.yaml")).is_err());

    // Missing file should fail on load
    assert!(PmDomain::load(&dir.path().join("nonexistent.yaml")).is_err());
}

// ---------------------------------------------------------------------------
// E2E: PM repo structure and ID generation
// ---------------------------------------------------------------------------

#[test]
fn e2e_pm_repo_structure_and_ids() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = sample_domain();

    scaffold_pm_repo(pm_root, &domain);

    // Verify domain.yaml loads from scaffolded repo
    let loaded = PmDomain::load(&pm_root.join("domain.yaml")).unwrap();
    assert_eq!(loaded.key, "TMS");

    // Verify PmDomain::find_in_repo
    assert!(PmDomain::find_in_repo(pm_root).is_some());
    assert!(PmDomain::find_in_repo(&pm_root.join("nonexistent")).is_none());

    // --- Epic ID generation ---
    // TMS-01, TMS-02 exist → next should be TMS-03
    let next_epic = next_epic_id(pm_root, &domain, None).unwrap();
    assert_eq!(next_epic, "TMS-03");

    // --- Story ID generation per repo key ---
    // BKD: 001, 002, 003 exist → next BKD-004
    let next_bkd = next_story_id(pm_root, &domain, "BKD", None).unwrap();
    assert_eq!(next_bkd, "BKD-004");

    // FRD: 001 exists → next FRD-002
    let next_frd = next_story_id(pm_root, &domain, "FRD", None).unwrap();
    assert_eq!(next_frd, "FRD-002");

    // PRD: no stories → next PRD-001
    let next_prd = next_story_id(pm_root, &domain, "PRD", None).unwrap();
    assert_eq!(next_prd, "PRD-001");
}

#[test]
fn e2e_incremental_story_creation() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = sample_domain();

    scaffold_pm_repo(pm_root, &domain);

    // Next BKD should be 004
    let id1 = next_story_id(pm_root, &domain, "BKD", None).unwrap();
    assert_eq!(id1, "BKD-004");

    // Simulate creating that story file
    let epic_dir = pm_root.join("epics").join("TMS-01-party-system");
    fs::write(
        epic_dir.join("BKD-004-new-feature.md"),
        "---\ntype: story\nid: BKD-004\n---\n",
    )
    .unwrap();

    // Next should be 005
    let id2 = next_story_id(pm_root, &domain, "BKD", None).unwrap();
    assert_eq!(id2, "BKD-005");
}

#[test]
fn e2e_incremental_epic_creation() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = sample_domain();

    scaffold_pm_repo(pm_root, &domain);

    // Next epic should be TMS-03
    let id1 = next_epic_id(pm_root, &domain, None).unwrap();
    assert_eq!(id1, "TMS-03");

    // Simulate creating that epic directory
    let epic_dir = pm_root.join("epics").join("TMS-03-notifications");
    fs::create_dir_all(&epic_dir).unwrap();
    fs::write(epic_dir.join("_epic.md"), "---\ntype: epic\nid: TMS-03\n---\n").unwrap();

    // Next should be TMS-04
    let id2 = next_epic_id(pm_root, &domain, None).unwrap();
    assert_eq!(id2, "TMS-04");
}

#[test]
fn e2e_cross_epic_story_ids() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = sample_domain();

    scaffold_pm_repo(pm_root, &domain);

    // BKD-003 is under TMS-02. Add BKD-005 under TMS-01 (gap at 004).
    let epic1 = pm_root.join("epics").join("TMS-01-party-system");
    fs::write(
        epic1.join("BKD-005-skip-ahead.md"),
        "---\ntype: story\nid: BKD-005\n---\n",
    )
    .unwrap();

    // Next BKD should be 006, respecting max across all epics
    let next = next_story_id(pm_root, &domain, "BKD", None).unwrap();
    assert_eq!(next, "BKD-006");
}

#[test]
fn e2e_empty_pm_repo() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();

    let domain = PmDomain {
        org: "neworg".to_string(),
        key: "NEW".to_string(),
        domain: None,
        project_key: None,
        drift_policy: "warn".to_string(),
        jira: None,
        components: vec![],
        areas: vec![],
        repos: vec![RepoEntry {
            name: "first-repo".to_string(),
            key: Some("FST".to_string()),
            url: None,
            description: Some("The first repo".to_string()),
        }],
    };

    // No epics/ directory at all
    let epic_id = next_epic_id(pm_root, &domain, None).unwrap();
    assert_eq!(epic_id, "NEW-01");

    let story_id = next_story_id(pm_root, &domain, "FST", None).unwrap();
    assert_eq!(story_id, "FST-001");
}

// ---------------------------------------------------------------------------
// E2E: ID parsing and formatting
// ---------------------------------------------------------------------------

#[test]
fn e2e_id_parsing_roundtrip() {
    // Epic IDs
    for n in 1..=99 {
        let id = format_epic_id("TMS", n);
        let (prefix, num) = parse_id(&id).unwrap();
        assert_eq!(prefix, "TMS");
        assert_eq!(num, n);
    }

    // Story IDs
    for n in 1..=999 {
        let id = format_story_id("BKD", n);
        let (prefix, num) = parse_id(&id).unwrap();
        assert_eq!(prefix, "BKD");
        assert_eq!(num, n);
    }
}

// ---------------------------------------------------------------------------
// E2E: Jira collision check (requires BLUE_JIRA_TEST_* env vars)
// ---------------------------------------------------------------------------

fn jira_test_config() -> Option<(String, String, String, String)> {
    Some((
        std::env::var("BLUE_JIRA_TEST_DOMAIN").ok()?,
        std::env::var("BLUE_JIRA_TEST_EMAIL").ok()?,
        std::env::var("BLUE_JIRA_TEST_TOKEN").ok()?,
        std::env::var("BLUE_JIRA_TEST_PROJECT").ok()?,
    ))
}

#[test]
fn e2e_jira_collision_check() {
    let Some((jira_domain, email, token, project)) = jira_test_config() else {
        eprintln!("Skipping Jira e2e: BLUE_JIRA_TEST_* env vars not set");
        return;
    };

    let tracker = blue_core::JiraCloudTracker::new(jira_domain, email, token);

    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = PmDomain {
        org: "test-org".to_string(),
        key: "TST".to_string(),
        domain: None,
        project_key: Some(project),
        drift_policy: "warn".to_string(),
        jira: None,
        components: vec![],
        areas: vec![],
        repos: vec![RepoEntry {
            name: "test-repo".to_string(),
            key: Some("TR".to_string()),
            url: None,
            description: None,
        }],
    };

    // Should succeed even with Jira check (just finds max from both sources)
    let epic_id = next_epic_id(pm_root, &domain, Some(&tracker)).unwrap();
    eprintln!("Next epic ID with Jira check: {}", epic_id);
    assert!(epic_id.starts_with("TST-"));

    let story_id = next_story_id(pm_root, &domain, "TR", Some(&tracker)).unwrap();
    eprintln!("Next story ID with Jira check: {}", story_id);
    assert!(story_id.starts_with("TR-"));
}

// ---------------------------------------------------------------------------
// E2E: domain.yaml content matches RFC 0068 spec
// ---------------------------------------------------------------------------

#[test]
fn e2e_domain_yaml_matches_rfc_spec() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("domain.yaml");

    let domain = sample_domain();
    domain.save(&path).unwrap();

    // Read raw YAML and verify structure
    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains("org:"));
    assert!(content.contains("key:"));
    assert!(content.contains("repos:"));

    // Verify all repos have required fields
    let loaded = PmDomain::load(&path).unwrap();
    for repo in &loaded.repos {
        assert!(!repo.name.is_empty(), "repo name must not be empty");
        let key = repo.key.as_deref().expect("repo key must be present");
        assert!(!key.is_empty(), "repo key must not be empty");
        assert!(
            key.chars().all(|c| c.is_ascii_uppercase()),
            "repo key {} should be uppercase",
            key
        );
    }

    // Verify org key is uppercase
    assert!(
        loaded.key.chars().all(|c| c.is_ascii_uppercase()),
        "org key should be uppercase"
    );
}

#[test]
fn e2e_full_pm_repo_scaffold_and_validate() {
    let dir = tempfile::tempdir().unwrap();
    let pm_root = dir.path();
    let domain = sample_domain();

    scaffold_pm_repo(pm_root, &domain);

    // Verify file structure exists
    assert!(pm_root.join("domain.yaml").exists());
    assert!(pm_root.join("jira.toml").exists());
    assert!(pm_root.join("epics/TMS-01-party-system/_epic.md").exists());
    assert!(pm_root
        .join("epics/TMS-01-party-system/BKD-001-create-party-api.md")
        .exists());
    assert!(pm_root
        .join("epics/TMS-01-party-system/BKD-002-party-invites-api.md")
        .exists());
    assert!(pm_root
        .join("epics/TMS-01-party-system/FRD-001-create-party-ui.md")
        .exists());
    assert!(pm_root.join("epics/TMS-02-move-discovery/_epic.md").exists());
    assert!(pm_root
        .join("epics/TMS-02-move-discovery/BKD-003-ai-move-generation.md")
        .exists());

    // Verify story YAML front matter parses
    let story_content =
        fs::read_to_string(pm_root.join("epics/TMS-01-party-system/BKD-001-create-party-api.md"))
            .unwrap();
    assert!(story_content.contains("type: story"));
    assert!(story_content.contains("id: BKD-001"));
    assert!(story_content.contains("epic: TMS-01"));
    assert!(story_content.contains("repo: themove-backend"));

    // Verify epic YAML front matter parses
    let epic_content =
        fs::read_to_string(pm_root.join("epics/TMS-01-party-system/_epic.md")).unwrap();
    assert!(epic_content.contains("type: epic"));
    assert!(epic_content.contains("id: TMS-01"));
}
