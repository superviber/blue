//! Jira lint checks — detect credential leaks in committed files (RFC 0063, Phase 3)
//!
//! Scans file content for patterns that look like Atlassian API tokens,
//! raw credential values, or credential file contents that should never
//! be committed to version control.

use regex::Regex;

/// A lint warning about potentially leaked credentials
#[derive(Debug, Clone)]
pub struct LintWarning {
    pub file: String,
    pub line: usize,
    pub message: String,
    pub severity: LintSeverity,
}

/// Severity of a lint warning
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LintSeverity {
    Error,
    Warning,
}

impl std::fmt::Display for LintSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LintSeverity::Error => write!(f, "error"),
            LintSeverity::Warning => write!(f, "warning"),
        }
    }
}

/// Check file content for Atlassian/Jira API credential patterns.
///
/// Returns a list of warnings for any suspicious patterns found.
/// The `file_name` parameter is used for reporting only.
pub fn check_for_jira_credentials(content: &str, file_name: &str) -> Vec<LintWarning> {
    let mut warnings = Vec::new();

    // Pattern 1: Atlassian API token prefix (ATATT followed by base64-ish)
    let atatt_re = Regex::new(r"ATATT[A-Za-z0-9+/=_\-]{20,}").unwrap();

    // Pattern 2: Raw token assignment in config files
    // Matches: token = "...", token: "...", "token": "..."
    let token_assign_re =
        Regex::new(r#"(?i)(?:api[_-]?)?token\s*[=:]\s*["'][A-Za-z0-9+/=_\-]{16,}["']"#).unwrap();

    // Pattern 3: Jira credentials TOML structure
    let toml_cred_re =
        Regex::new(r"(?i)\[\[credentials\]\]").unwrap();

    // Pattern 4: Basic auth header with base64 payload
    let basic_auth_re =
        Regex::new(r"(?i)(?:basic\s+|authorization:\s*basic\s+)[A-Za-z0-9+/=]{20,}").unwrap();

    // Pattern 5: Atlassian Cloud API token env var with value
    let env_token_re =
        Regex::new(r#"(?i)BLUE_JIRA_TOKEN[A-Z_]*\s*=\s*["']?[A-Za-z0-9+/=_\-]{16,}"#).unwrap();

    for (line_idx, line) in content.lines().enumerate() {
        let line_num = line_idx + 1;

        if atatt_re.is_match(line) {
            warnings.push(LintWarning {
                file: file_name.to_string(),
                line: line_num,
                message: "Atlassian API token detected (ATATT prefix)".to_string(),
                severity: LintSeverity::Error,
            });
        }

        if token_assign_re.is_match(line) {
            warnings.push(LintWarning {
                file: file_name.to_string(),
                line: line_num,
                message: "Possible API token assignment detected".to_string(),
                severity: LintSeverity::Error,
            });
        }

        if toml_cred_re.is_match(line) {
            warnings.push(LintWarning {
                file: file_name.to_string(),
                line: line_num,
                message: "Credential TOML structure detected — this file should not be committed"
                    .to_string(),
                severity: LintSeverity::Error,
            });
        }

        if basic_auth_re.is_match(line) {
            warnings.push(LintWarning {
                file: file_name.to_string(),
                line: line_num,
                message: "Basic auth header with encoded credentials detected".to_string(),
                severity: LintSeverity::Warning,
            });
        }

        if env_token_re.is_match(line) {
            warnings.push(LintWarning {
                file: file_name.to_string(),
                line: line_num,
                message: "Jira token environment variable with value detected".to_string(),
                severity: LintSeverity::Error,
            });
        }
    }

    warnings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_content() {
        let content = r#"
# My RFC

| | |
|---|---|
| **Status** | Draft |
| **Jira** | PROJ-123 |

## Summary

This is a normal RFC with no credentials.
"#;
        let warnings = check_for_jira_credentials(content, "test.md");
        assert!(warnings.is_empty(), "Expected no warnings, got: {:?}", warnings);
    }

    #[test]
    fn test_detect_atatt_token() {
        let content = "token = ATATT3xFfGF0Z3k3TURRNExUVmpaVGt0TkRSaE1DMWhORGM1TFRFNU";
        let warnings = check_for_jira_credentials(content, "config.toml");
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].severity, LintSeverity::Error);
        assert!(warnings[0].message.contains("ATATT"));
    }

    #[test]
    fn test_detect_token_assignment() {
        let content = r#"token = "abcdefghijklmnopqrstuvwx""#;
        let warnings = check_for_jira_credentials(content, ".env");
        assert!(
            warnings.iter().any(|w| w.message.contains("token assignment")),
            "Expected token assignment warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_detect_api_token_assignment() {
        let content = r#"api_token = "abcdefghijklmnopqrstuvwx""#;
        let warnings = check_for_jira_credentials(content, "config.yml");
        assert!(
            warnings.iter().any(|w| w.message.contains("token assignment")),
            "Expected token assignment warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_detect_toml_credentials_block() {
        let content = r#"
[[credentials]]
domain = "myorg.atlassian.net"
email = "user@example.com"
token = "secret123456789012345"
"#;
        let warnings = check_for_jira_credentials(content, "jira-credentials.toml");
        assert!(
            warnings.iter().any(|w| w.message.contains("Credential TOML")),
            "Expected TOML credential warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_detect_basic_auth_header() {
        let content = "Authorization: Basic dXNlckBleGFtcGxlLmNvbTpzZWNyZXQxMjM=";
        let warnings = check_for_jira_credentials(content, "script.sh");
        assert!(
            warnings.iter().any(|w| w.message.contains("Basic auth")),
            "Expected basic auth warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_detect_env_token_with_value() {
        let content = "BLUE_JIRA_TOKEN_MYORG_ATLASSIAN_NET=abcdefghijklmnopqrstuvwx";
        let warnings = check_for_jira_credentials(content, ".env");
        assert!(
            warnings.iter().any(|w| w.message.contains("Jira token environment")),
            "Expected env token warning, got: {:?}",
            warnings
        );
    }

    #[test]
    fn test_line_numbers_correct() {
        let content = "line 1\nline 2\nATATT3xFfGF0Z3k3TURRNExUVmpaVGt0TkRSaE1DMWhORGM1TFRFOU\nline 4";
        let warnings = check_for_jira_credentials(content, "test.rs");
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].line, 3);
    }

    #[test]
    fn test_multiple_warnings_same_file() {
        let content = r#"
ATATT3xFfGF0Z3k3TURRNExUVmpaVGt0TkRSaE1DMWhORGM1TFRFOU
token = "abcdefghijklmnopqrstuvwx"
"#;
        let warnings = check_for_jira_credentials(content, "bad.toml");
        assert!(warnings.len() >= 2, "Expected at least 2 warnings, got {}", warnings.len());
    }

    #[test]
    fn test_short_tokens_not_flagged() {
        // Tokens shorter than the minimum length should not trigger
        let content = r#"token = "short""#;
        let warnings = check_for_jira_credentials(content, "test.toml");
        assert!(
            !warnings.iter().any(|w| w.message.contains("token assignment")),
            "Short token should not be flagged"
        );
    }

    #[test]
    fn test_severity_display() {
        assert_eq!(LintSeverity::Error.to_string(), "error");
        assert_eq!(LintSeverity::Warning.to_string(), "warning");
    }
}
