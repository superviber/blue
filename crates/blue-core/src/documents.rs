//! Document types for Blue
//!
//! RFCs, ADRs, Spikes, and other document structures.

use serde::{Deserialize, Serialize};

/// Document status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Accepted,
    InProgress,
    Implemented,
    Superseded,
}

/// An RFC (Request for Comments) - a design document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rfc {
    pub title: String,
    pub status: Status,
    pub problem: Option<String>,
    pub proposal: Option<String>,
    pub goals: Vec<String>,
    pub plan: Vec<Task>,
}

/// A task within an RFC plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub description: String,
    pub completed: bool,
}

/// A Spike - a time-boxed investigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spike {
    pub title: String,
    pub question: String,
    pub time_box: Option<String>,
    pub outcome: Option<SpikeOutcome>,
    pub summary: Option<String>,
}

/// Outcome of a spike investigation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeOutcome {
    NoAction,
    DecisionMade,
    RecommendsImplementation,
}

/// A Decision Note - lightweight choice documentation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub title: String,
    pub decision: String,
    pub rationale: Option<String>,
    pub alternatives: Vec<String>,
}

/// An ADR (Architecture Decision Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Adr {
    pub title: String,
    pub context: String,
    pub decision: String,
    pub consequences: Vec<String>,
}

impl Rfc {
    /// Create a new RFC in draft status
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            status: Status::Draft,
            problem: None,
            proposal: None,
            goals: Vec::new(),
            plan: Vec::new(),
        }
    }

    /// Calculate completion percentage of the plan
    pub fn progress(&self) -> f64 {
        if self.plan.is_empty() {
            return 0.0;
        }
        let completed = self.plan.iter().filter(|t| t.completed).count();
        (completed as f64 / self.plan.len() as f64) * 100.0
    }
}

impl Spike {
    /// Create a new spike
    pub fn new(title: impl Into<String>, question: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            question: question.into(),
            time_box: None,
            outcome: None,
            summary: None,
        }
    }
}
