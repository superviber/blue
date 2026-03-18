//! Credential storage for issue trackers (RFC 0063/0065)
//!
//! Three-tier hierarchy (checked in order):
//! 1. Environment variables: BLUE_JIRA_TOKEN_{DOMAIN_SLUG}, BLUE_JIRA_EMAIL_{DOMAIN_SLUG}
//! 2. OS keychain: service "blue-jira", user "{email}@{domain}"
//! 3. TOML file: ~/.config/blue/jira-credentials.toml (chmod 0600)

use super::{TrackerCredentials, TrackerError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const KEYRING_SERVICE: &str = "blue-jira";

/// Credential store for a specific Jira domain
pub struct CredentialStore {
    domain: String,
}

impl CredentialStore {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
        }
    }

    /// Resolve credentials from the three-tier hierarchy
    pub fn get_credentials(&self) -> Result<TrackerCredentials, TrackerError> {
        // Tier 1: Environment variables
        if let Some(creds) = self.resolve_env() {
            return Ok(creds);
        }

        // Tier 2: OS keychain
        if let Some(creds) = self.resolve_keychain() {
            return Ok(creds);
        }

        // Tier 3: TOML file
        if let Some(creds) = self.resolve_toml() {
            return Ok(creds);
        }

        Err(TrackerError::MissingCredentials(format!(
            "No credentials found for {}. Run: blue jira auth login --domain {}",
            self.domain, self.domain
        )))
    }

    /// Store credentials in keychain (primary interactive path)
    pub fn store_keychain(&self, creds: &TrackerCredentials) -> Result<(), TrackerError> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, &self.domain)
            .map_err(|e| TrackerError::MissingCredentials(format!("Keychain error: {}", e)))?;

        // Store as JSON so we can retrieve both email and token
        let value = serde_json::json!({
            "email": creds.email,
            "token": creds.token,
        });

        entry.set_password(&value.to_string()).map_err(|e| {
            TrackerError::MissingCredentials(format!("Keychain store failed: {}", e))
        })?;

        Ok(())
    }

    /// Store credentials in TOML file (fallback)
    pub fn store_toml(&self, creds: &TrackerCredentials) -> Result<(), TrackerError> {
        let path = toml_path();
        let mut file = load_toml_file(&path);

        // Remove existing entry for this domain
        file.credentials.retain(|c| c.domain != self.domain);

        file.credentials.push(TomlCredential {
            domain: self.domain.clone(),
            email: creds.email.clone(),
            token: creds.token.clone(),
        });

        save_toml_file(&path, &file)
    }

    /// Clear credentials from all stores
    pub fn clear(&self) -> Result<(), TrackerError> {
        // Clear keychain entries (best-effort)
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &self.domain) {
            let _ = entry.delete_credential();
        }

        // Clear TOML entry
        let path = toml_path();
        let mut file = load_toml_file(&path);
        file.credentials.retain(|c| c.domain != self.domain);
        let _ = save_toml_file(&path, &file);

        Ok(())
    }

    /// Which tier resolved credentials (for diagnostics)
    pub fn resolve_tier(&self) -> Option<&'static str> {
        if self.resolve_env().is_some() {
            Some("environment")
        } else if self.resolve_keychain().is_some() {
            Some("keychain")
        } else if self.resolve_toml().is_some() {
            Some("toml")
        } else {
            None
        }
    }

    // --- Tier implementations ---

    fn resolve_env(&self) -> Option<TrackerCredentials> {
        let slug = domain_slug(&self.domain);
        let token = std::env::var(format!("BLUE_JIRA_TOKEN_{}", slug)).ok()?;
        let email = std::env::var(format!("BLUE_JIRA_EMAIL_{}", slug)).ok()?;
        Some(TrackerCredentials { email, token })
    }

    fn resolve_keychain(&self) -> Option<TrackerCredentials> {
        // Try to find any entry for this domain
        let entry = keyring::Entry::new(KEYRING_SERVICE, &self.domain).ok()?;
        let password = entry.get_password().ok()?;

        // Parse JSON-stored credentials
        let value: serde_json::Value = serde_json::from_str(&password).ok()?;
        let email = value.get("email")?.as_str()?.to_string();
        let token = value.get("token")?.as_str()?.to_string();

        Some(TrackerCredentials { email, token })
    }

    fn resolve_toml(&self) -> Option<TrackerCredentials> {
        let path = toml_path();
        let file = load_toml_file(&path);
        let entry = file.credentials.iter().find(|c| c.domain == self.domain)?;

        Some(TrackerCredentials {
            email: entry.email.clone(),
            token: entry.token.clone(),
        })
    }
}

/// Convert domain to env var slug: superviber.atlassian.net → SUPERVIBER_ATLASSIAN_NET
fn domain_slug(domain: &str) -> String {
    domain.replace(['.', '-'], "_").to_uppercase()
}

fn toml_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("blue")
        .join("jira-credentials.toml")
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct TomlCredentialFile {
    #[serde(default)]
    credentials: Vec<TomlCredential>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TomlCredential {
    domain: String,
    email: String,
    token: String,
}

fn load_toml_file(path: &PathBuf) -> TomlCredentialFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_toml_file(path: &PathBuf, file: &TomlCredentialFile) -> Result<(), TrackerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            TrackerError::MissingCredentials(format!("Failed to create config dir: {}", e))
        })?;
    }

    let content = toml::to_string_pretty(file)
        .map_err(|e| TrackerError::MissingCredentials(format!("TOML serialize failed: {}", e)))?;

    std::fs::write(path, &content).map_err(|e| {
        TrackerError::MissingCredentials(format!("Failed to write credentials: {}", e))
    })?;

    // chmod 0600 on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_slug() {
        assert_eq!(
            domain_slug("superviber.atlassian.net"),
            "SUPERVIBER_ATLASSIAN_NET"
        );
        assert_eq!(domain_slug("my-org.atlassian.net"), "MY_ORG_ATLASSIAN_NET");
    }

    #[test]
    fn test_env_tier_resolution() {
        let store = CredentialStore::new("test-env.example.com");

        // No env vars set — should return None
        assert!(store.resolve_env().is_none());

        // Set env vars
        std::env::set_var("BLUE_JIRA_TOKEN_TEST_ENV_EXAMPLE_COM", "test-token");
        std::env::set_var("BLUE_JIRA_EMAIL_TEST_ENV_EXAMPLE_COM", "test@example.com");

        let creds = store.resolve_env().expect("should resolve from env");
        assert_eq!(creds.email, "test@example.com");
        assert_eq!(creds.token, "test-token");

        // Cleanup
        std::env::remove_var("BLUE_JIRA_TOKEN_TEST_ENV_EXAMPLE_COM");
        std::env::remove_var("BLUE_JIRA_EMAIL_TEST_ENV_EXAMPLE_COM");
    }

    #[test]
    fn test_toml_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("creds.toml");

        let file = TomlCredentialFile {
            credentials: vec![TomlCredential {
                domain: "test.atlassian.net".to_string(),
                email: "user@test.com".to_string(),
                token: "secret123".to_string(),
            }],
        };

        save_toml_file(&path, &file).unwrap();

        let loaded = load_toml_file(&path);
        assert_eq!(loaded.credentials.len(), 1);
        assert_eq!(loaded.credentials[0].domain, "test.atlassian.net");
        assert_eq!(loaded.credentials[0].token, "secret123");
    }
}
