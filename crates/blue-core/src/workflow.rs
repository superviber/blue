//! Workflow transitions for Blue documents
//!
//! Status management and validation for RFCs, Spikes, and other documents.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// RFC status values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RfcStatus {
    /// Initial proposal, still being refined
    Draft,
    /// Approved, ready to implement
    Accepted,
    /// Work has started
    InProgress,
    /// Work is complete
    Implemented,
    /// Replaced by a newer RFC
    Superseded,
}

impl RfcStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RfcStatus::Draft => "draft",
            RfcStatus::Accepted => "accepted",
            RfcStatus::InProgress => "in-progress",
            RfcStatus::Implemented => "implemented",
            RfcStatus::Superseded => "superseded",
        }
    }

    pub fn parse(s: &str) -> Result<Self, WorkflowError> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(RfcStatus::Draft),
            "accepted" => Ok(RfcStatus::Accepted),
            "in-progress" | "in_progress" | "inprogress" => Ok(RfcStatus::InProgress),
            "implemented" => Ok(RfcStatus::Implemented),
            "superseded" => Ok(RfcStatus::Superseded),
            _ => Err(WorkflowError::InvalidStatus(s.to_string())),
        }
    }

    /// Check if transition to the given status is valid
    pub fn can_transition_to(&self, target: RfcStatus) -> bool {
        matches!(
            (self, target),
            // Normal forward flow
            (RfcStatus::Draft, RfcStatus::Accepted)
                | (RfcStatus::Accepted, RfcStatus::InProgress)
                | (RfcStatus::InProgress, RfcStatus::Implemented)
                // Can supersede from any active state
                | (RfcStatus::Draft, RfcStatus::Superseded)
                | (RfcStatus::Accepted, RfcStatus::Superseded)
                | (RfcStatus::InProgress, RfcStatus::Superseded)
                // Can go back to draft if needed
                | (RfcStatus::Accepted, RfcStatus::Draft)
        )
    }

    /// Get allowed transitions from current status
    pub fn allowed_transitions(&self) -> Vec<RfcStatus> {
        match self {
            RfcStatus::Draft => vec![RfcStatus::Accepted, RfcStatus::Superseded],
            RfcStatus::Accepted => {
                vec![RfcStatus::InProgress, RfcStatus::Draft, RfcStatus::Superseded]
            }
            RfcStatus::InProgress => vec![RfcStatus::Implemented, RfcStatus::Superseded],
            RfcStatus::Implemented => vec![],
            RfcStatus::Superseded => vec![],
        }
    }
}

/// Spike outcome values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeOutcome {
    /// Investigation showed this isn't worth pursuing
    NoAction,
    /// Learned enough to make a decision
    DecisionMade,
    /// Should build something (requires RFC)
    RecommendsImplementation,
    /// Fix applied during investigation (RFC 0035)
    Resolved,
}

impl SpikeOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpikeOutcome::NoAction => "no-action",
            SpikeOutcome::DecisionMade => "decision-made",
            SpikeOutcome::RecommendsImplementation => "recommends-implementation",
            SpikeOutcome::Resolved => "resolved",
        }
    }

    pub fn parse(s: &str) -> Result<Self, WorkflowError> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "no-action" => Ok(SpikeOutcome::NoAction),
            "decision-made" => Ok(SpikeOutcome::DecisionMade),
            "recommends-implementation" => Ok(SpikeOutcome::RecommendsImplementation),
            "resolved" => Ok(SpikeOutcome::Resolved),
            _ => Err(WorkflowError::InvalidOutcome(s.to_string())),
        }
    }
}

/// Spike status values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeStatus {
    /// Investigation in progress
    InProgress,
    /// Investigation complete
    Completed,
    /// Investigation led directly to fix (RFC 0035)
    Resolved,
}

impl SpikeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SpikeStatus::InProgress => "in-progress",
            SpikeStatus::Completed => "completed",
            SpikeStatus::Resolved => "resolved",
        }
    }

    pub fn parse(s: &str) -> Result<Self, WorkflowError> {
        match s.to_lowercase().replace('_', "-").as_str() {
            "in-progress" => Ok(SpikeStatus::InProgress),
            "completed" => Ok(SpikeStatus::Completed),
            "resolved" => Ok(SpikeStatus::Resolved),
            _ => Err(WorkflowError::InvalidStatus(s.to_string())),
        }
    }
}

/// PRD status values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PrdStatus {
    /// Initial requirements, still being refined
    Draft,
    /// Stakeholders have signed off
    Approved,
    /// All requirements implemented
    Implemented,
}

impl PrdStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrdStatus::Draft => "draft",
            PrdStatus::Approved => "approved",
            PrdStatus::Implemented => "implemented",
        }
    }

    pub fn parse(s: &str) -> Result<Self, WorkflowError> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(PrdStatus::Draft),
            "approved" => Ok(PrdStatus::Approved),
            "implemented" => Ok(PrdStatus::Implemented),
            _ => Err(WorkflowError::InvalidStatus(s.to_string())),
        }
    }

    pub fn can_transition_to(&self, target: PrdStatus) -> bool {
        matches!(
            (self, target),
            (PrdStatus::Draft, PrdStatus::Approved) | (PrdStatus::Approved, PrdStatus::Implemented)
        )
    }
}

/// Workflow errors
#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("'{0}' isn't a valid status. Try: draft, accepted, in-progress, implemented")]
    InvalidStatus(String),

    #[error("'{0}' isn't a valid outcome. Try: no-action, decision-made, recommends-implementation")]
    InvalidOutcome(String),

    #[error("Can't go from {from} to {to}. {hint}")]
    InvalidTransition {
        from: String,
        to: String,
        hint: String,
    },
}

/// Validate an RFC status transition
pub fn validate_rfc_transition(from: RfcStatus, to: RfcStatus) -> Result<(), WorkflowError> {
    if from.can_transition_to(to) {
        Ok(())
    } else {
        let hint = match (from, to) {
            (RfcStatus::Draft, RfcStatus::InProgress) => {
                "Accept it first, then start work".to_string()
            }
            (RfcStatus::Implemented, _) => {
                "Already implemented. Create a new RFC for changes".to_string()
            }
            (RfcStatus::Superseded, _) => "This RFC has been superseded".to_string(),
            _ => format!("From {} you can go to: {:?}", from.as_str(), from.allowed_transitions()),
        };

        Err(WorkflowError::InvalidTransition {
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
            hint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc_transitions() {
        assert!(RfcStatus::Draft.can_transition_to(RfcStatus::Accepted));
        assert!(RfcStatus::Accepted.can_transition_to(RfcStatus::InProgress));
        assert!(RfcStatus::InProgress.can_transition_to(RfcStatus::Implemented));

        assert!(!RfcStatus::Draft.can_transition_to(RfcStatus::InProgress));
        assert!(!RfcStatus::Implemented.can_transition_to(RfcStatus::Draft));
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(RfcStatus::parse("draft").unwrap(), RfcStatus::Draft);
        assert_eq!(RfcStatus::parse("in-progress").unwrap(), RfcStatus::InProgress);
        assert_eq!(RfcStatus::parse("IN_PROGRESS").unwrap(), RfcStatus::InProgress);
    }

    #[test]
    fn test_spike_outcome_parse() {
        assert_eq!(
            SpikeOutcome::parse("no-action").unwrap(),
            SpikeOutcome::NoAction
        );
        assert_eq!(
            SpikeOutcome::parse("recommends-implementation").unwrap(),
            SpikeOutcome::RecommendsImplementation
        );
        assert_eq!(
            SpikeOutcome::parse("resolved").unwrap(),
            SpikeOutcome::Resolved
        );
    }

    #[test]
    fn test_spike_status_parse_resolved() {
        assert_eq!(
            SpikeStatus::parse("resolved").unwrap(),
            SpikeStatus::Resolved
        );
        assert_eq!(SpikeStatus::Resolved.as_str(), "resolved");
    }
}
