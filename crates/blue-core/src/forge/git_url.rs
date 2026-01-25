//! Git URL parsing utilities
//!
//! Parses various git remote URL formats to extract host, owner, and repo.

/// Parsed git URL components
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitUrl {
    pub host: String,
    pub owner: String,
    pub repo: String,
    pub protocol: GitProtocol,
}

/// Git URL protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitProtocol {
    Ssh,
    Https,
    Git,
}

/// Parse a git remote URL into its components
///
/// Supports formats:
/// - `git@github.com:owner/repo.git` (SSH)
/// - `https://github.com/owner/repo.git` (HTTPS)
/// - `git://github.com/owner/repo.git` (Git protocol)
/// - `ssh://git@github.com/owner/repo.git` (SSH with explicit protocol)
pub fn parse_git_url(url: &str) -> GitUrl {
    let url = url.trim();

    // SSH format: git@host:owner/repo.git
    if url.starts_with("git@") || url.contains("@") && url.contains(":") && !url.contains("://") {
        return parse_ssh_url(url);
    }

    // HTTPS format: https://host/owner/repo.git
    if url.starts_with("https://") {
        return parse_https_url(url);
    }

    // SSH with protocol: ssh://git@host/owner/repo.git
    if url.starts_with("ssh://") {
        return parse_ssh_protocol_url(url);
    }

    // Git protocol: git://host/owner/repo.git
    if url.starts_with("git://") {
        return parse_git_protocol_url(url);
    }

    // Fallback: try to extract what we can
    GitUrl {
        host: String::new(),
        owner: String::new(),
        repo: url.to_string(),
        protocol: GitProtocol::Https,
    }
}

fn parse_ssh_url(url: &str) -> GitUrl {
    // Format: git@host:owner/repo.git or user@host:owner/repo.git
    let without_user = url.split('@').nth(1).unwrap_or(url);
    let parts: Vec<&str> = without_user.splitn(2, ':').collect();

    if parts.len() != 2 {
        return GitUrl {
            host: String::new(),
            owner: String::new(),
            repo: url.to_string(),
            protocol: GitProtocol::Ssh,
        };
    }

    let host = parts[0].to_string();
    let path = parts[1].trim_end_matches(".git");
    let path_parts: Vec<&str> = path.splitn(2, '/').collect();

    let (owner, repo) = if path_parts.len() == 2 {
        (path_parts[0].to_string(), path_parts[1].to_string())
    } else {
        (String::new(), path.to_string())
    };

    GitUrl {
        host,
        owner,
        repo,
        protocol: GitProtocol::Ssh,
    }
}

fn parse_https_url(url: &str) -> GitUrl {
    // Format: https://host/owner/repo.git
    let without_protocol = url.trim_start_matches("https://");
    let parts: Vec<&str> = without_protocol.splitn(4, '/').collect();

    let host = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let owner = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let repo = parts.get(2)
        .map(|s| s.trim_end_matches(".git").to_string())
        .unwrap_or_default();

    GitUrl {
        host,
        owner,
        repo,
        protocol: GitProtocol::Https,
    }
}

fn parse_ssh_protocol_url(url: &str) -> GitUrl {
    // Format: ssh://git@host/owner/repo.git
    let without_protocol = url.trim_start_matches("ssh://");
    let without_user = without_protocol.split('@').nth(1).unwrap_or(without_protocol);
    let parts: Vec<&str> = without_user.splitn(4, '/').collect();

    let host = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let owner = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let repo = parts.get(2)
        .map(|s| s.trim_end_matches(".git").to_string())
        .unwrap_or_default();

    GitUrl {
        host,
        owner,
        repo,
        protocol: GitProtocol::Ssh,
    }
}

fn parse_git_protocol_url(url: &str) -> GitUrl {
    // Format: git://host/owner/repo.git
    let without_protocol = url.trim_start_matches("git://");
    let parts: Vec<&str> = without_protocol.splitn(4, '/').collect();

    let host = parts.first().map(|s| s.to_string()).unwrap_or_default();
    let owner = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let repo = parts.get(2)
        .map(|s| s.trim_end_matches(".git").to_string())
        .unwrap_or_default();

    GitUrl {
        host,
        owner,
        repo,
        protocol: GitProtocol::Git,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ssh_url() {
        let url = parse_git_url("git@github.com:owner/repo.git");
        assert_eq!(url.host, "github.com");
        assert_eq!(url.owner, "owner");
        assert_eq!(url.repo, "repo");
        assert_eq!(url.protocol, GitProtocol::Ssh);
    }

    #[test]
    fn test_parse_https_url() {
        let url = parse_git_url("https://github.com/owner/repo.git");
        assert_eq!(url.host, "github.com");
        assert_eq!(url.owner, "owner");
        assert_eq!(url.repo, "repo");
        assert_eq!(url.protocol, GitProtocol::Https);
    }

    #[test]
    fn test_parse_https_url_no_git_suffix() {
        let url = parse_git_url("https://github.com/owner/repo");
        assert_eq!(url.host, "github.com");
        assert_eq!(url.owner, "owner");
        assert_eq!(url.repo, "repo");
    }

    #[test]
    fn test_parse_codeberg_ssh() {
        let url = parse_git_url("git@codeberg.org:user/project.git");
        assert_eq!(url.host, "codeberg.org");
        assert_eq!(url.owner, "user");
        assert_eq!(url.repo, "project");
    }

    #[test]
    fn test_parse_custom_host() {
        let url = parse_git_url("git@git.example.com:team/app.git");
        assert_eq!(url.host, "git.example.com");
        assert_eq!(url.owner, "team");
        assert_eq!(url.repo, "app");
    }

    #[test]
    fn test_parse_ssh_protocol() {
        let url = parse_git_url("ssh://git@github.com/owner/repo.git");
        assert_eq!(url.host, "github.com");
        assert_eq!(url.owner, "owner");
        assert_eq!(url.repo, "repo");
        assert_eq!(url.protocol, GitProtocol::Ssh);
    }
}
