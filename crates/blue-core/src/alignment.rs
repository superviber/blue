//! Alignment Dialogue Orchestration
//!
//! Implements RFC 0012: Alignment Dialogue Orchestration
//! Based on ADR 0006 from coherence-mcp.
//!
//! The ALIGNMENT measure: Wisdom + Consistency + Truth + Relationships
//! - All dimensions are UNBOUNDED
//! - Convergence is direction, not destination

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// An expert in the alignment dialogue panel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expert {
    /// Short identifier (e.g., "DS", "PM", "SEC")
    pub id: String,
    /// Full name (e.g., "Distributed Systems Architect")
    pub name: String,
    /// Primary perspective (e.g., "Consistency, partition tolerance")
    pub perspective: String,
    /// Home domain for relevance scoring
    pub domain: String,
    /// Tier: Core (0.8+), Adjacent (0.4-0.8), Wildcard (<0.4)
    pub tier: ExpertTier,
    /// Optional emoji for display
    pub emoji: Option<String>,
}

/// Expert tier based on relevance to topic
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ExpertTier {
    Core,
    Adjacent,
    Wildcard,
}

impl ExpertTier {
    pub fn from_relevance(score: f64) -> Self {
        if score >= 0.8 {
            ExpertTier::Core
        } else if score >= 0.4 {
            ExpertTier::Adjacent
        } else {
            ExpertTier::Wildcard
        }
    }
}

/// A single expert's response in a round
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertResponse {
    /// Expert ID
    pub expert_id: String,
    /// Full response content
    pub content: String,
    /// Position summary (1-2 sentences)
    pub position: String,
    /// Confidence in position (0.0 - 1.0)
    pub confidence: f64,
    /// New perspectives surfaced [PERSPECTIVE Pxx: ...]
    pub perspectives: Vec<Perspective>,
    /// Tensions raised [TENSION Tx: ...]
    pub tensions: Vec<Tension>,
    /// Refinements made [REFINEMENT: ...]
    pub refinements: Vec<String>,
    /// Concessions made [CONCESSION: ...]
    pub concessions: Vec<String>,
    /// Tensions resolved [RESOLVED Tx: ...]
    pub resolved_tensions: Vec<String>,
    /// ALIGNMENT score for this response
    pub score: AlignmentScore,
}

/// A perspective surfaced during dialogue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Perspective {
    /// Perspective ID (P01, P02, ...)
    pub id: String,
    /// Perspective description
    pub description: String,
    /// Who surfaced it
    pub surfaced_by: String,
    /// Which round
    pub round: u32,
    /// Current status
    pub status: PerspectiveStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PerspectiveStatus {
    Active,
    Converged,
    Deferred,
}

/// A tension between positions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tension {
    /// Tension ID (T1, T2, ...)
    pub id: String,
    /// Tension description
    pub description: String,
    /// Position A
    pub position_a: String,
    /// Position B
    pub position_b: String,
    /// Current status
    pub status: TensionStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TensionStatus {
    Open,
    Resolved,
}

/// ALIGNMENT score (unbounded)
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct AlignmentScore {
    /// How many perspectives integrated? How well synthesized?
    pub wisdom: u32,
    /// Does it follow patterns? Internally consistent?
    pub consistency: u32,
    /// Grounded in reality? Single source of truth?
    pub truth: u32,
    /// Connections to other artifacts?
    pub relationships: u32,
}

impl AlignmentScore {
    pub fn total(&self) -> u32 {
        self.wisdom + self.consistency + self.truth + self.relationships
    }
}

/// A single round of dialogue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round {
    /// Round number (0 = opening arguments)
    pub number: u32,
    /// All expert responses for this round
    pub responses: Vec<ExpertResponse>,
    /// Cumulative ALIGNMENT score after this round
    pub total_score: u32,
    /// ALIGNMENT velocity (delta from previous round)
    pub velocity: i32,
    /// Convergence percentage (0.0 - 1.0)
    pub convergence: f64,
}

/// Full alignment dialogue state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentDialogue {
    /// Topic being deliberated
    pub topic: String,
    /// Optional constraint
    pub constraint: Option<String>,
    /// Expert panel
    pub experts: Vec<Expert>,
    /// All rounds of dialogue
    pub rounds: Vec<Round>,
    /// All perspectives surfaced
    pub perspectives: Vec<Perspective>,
    /// All tensions tracked
    pub tensions: Vec<Tension>,
    /// Convergence threshold (default 0.95)
    pub convergence_threshold: f64,
    /// Maximum rounds (safety valve)
    pub max_rounds: u32,
    /// Current status
    pub status: DialogueStatus,
    /// Link to RFC if applicable
    pub rfc_title: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DialogueStatus {
    InProgress,
    Converged,
    MaxRoundsReached,
    Interrupted,
}

impl AlignmentDialogue {
    /// Create a new dialogue
    pub fn new(topic: String, constraint: Option<String>, experts: Vec<Expert>) -> Self {
        Self {
            topic,
            constraint,
            experts,
            rounds: Vec::new(),
            perspectives: Vec::new(),
            tensions: Vec::new(),
            convergence_threshold: 0.95,
            max_rounds: 12,
            status: DialogueStatus::InProgress,
            rfc_title: None,
        }
    }

    /// Get current round number
    pub fn current_round(&self) -> u32 {
        self.rounds.len() as u32
    }

    /// Get total ALIGNMENT score
    pub fn total_score(&self) -> u32 {
        self.rounds.last().map(|r| r.total_score).unwrap_or(0)
    }

    /// Get current ALIGNMENT velocity
    pub fn velocity(&self) -> i32 {
        self.rounds.last().map(|r| r.velocity).unwrap_or(0)
    }

    /// Get current convergence
    pub fn convergence(&self) -> f64 {
        self.rounds.last().map(|r| r.convergence).unwrap_or(0.0)
    }

    /// Check if dialogue should continue
    pub fn should_continue(&self) -> bool {
        match self.status {
            DialogueStatus::InProgress => {
                // Continue if: not converged AND under max rounds AND velocity not plateaued
                let not_converged = self.convergence() < self.convergence_threshold;
                let under_max = self.current_round() < self.max_rounds;
                let not_plateaued = self.rounds.len() < 2 || {
                    let last_two: Vec<_> = self.rounds.iter().rev().take(2).collect();
                    last_two.iter().any(|r| r.velocity.abs() > 2)
                };
                not_converged && under_max && not_plateaued
            }
            _ => false,
        }
    }

    /// Add a completed round
    pub fn add_round(&mut self, responses: Vec<ExpertResponse>) {
        let round_num = self.current_round();
        let prev_total = self.total_score();

        // Calculate round totals
        let round_score: u32 = responses.iter().map(|r| r.score.total()).sum();
        let new_total = prev_total + round_score;
        let velocity = round_score as i32;

        // Calculate convergence (proportion of experts with aligned positions)
        let convergence = calculate_convergence(&responses);

        // Extract new perspectives and tensions
        for response in &responses {
            self.perspectives.extend(response.perspectives.clone());
            self.tensions.extend(response.tensions.clone());

            // Mark resolved tensions
            for resolved_id in &response.resolved_tensions {
                if let Some(t) = self.tensions.iter_mut().find(|t| t.id == *resolved_id) {
                    t.status = TensionStatus::Resolved;
                }
            }
        }

        let round = Round {
            number: round_num,
            responses,
            total_score: new_total,
            velocity,
            convergence,
        };

        self.rounds.push(round);

        // Update status
        if convergence >= self.convergence_threshold {
            self.status = DialogueStatus::Converged;
        } else if self.current_round() >= self.max_rounds {
            self.status = DialogueStatus::MaxRoundsReached;
        }
    }
}

/// Calculate convergence from expert responses
/// Uses position clustering - convergence = size of largest aligned group / total
fn calculate_convergence(responses: &[ExpertResponse]) -> f64 {
    if responses.is_empty() {
        return 0.0;
    }

    // Simple approach: count how many experts have high confidence (>0.7)
    // and similar positions (based on first few words matching)
    let high_confidence: Vec<_> = responses
        .iter()
        .filter(|r| r.confidence >= 0.7)
        .collect();

    if high_confidence.is_empty() {
        return 0.0;
    }

    // Group by position similarity (simple word overlap)
    let mut position_groups: HashMap<String, usize> = HashMap::new();
    for response in &high_confidence {
        // Normalize position to first 20 chars
        let key = response.position.chars().take(20).collect::<String>().to_lowercase();
        *position_groups.entry(key).or_insert(0) += 1;
    }

    let largest_group = position_groups.values().max().copied().unwrap_or(0);
    largest_group as f64 / responses.len() as f64
}

/// Expert panel templates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelTemplate {
    Infrastructure,
    Product,
    MachineLearning,
    Governance,
    General,
}

impl PanelTemplate {
    /// Generate experts for this template
    pub fn generate_experts(&self, count: usize) -> Vec<Expert> {
        match self {
            PanelTemplate::Infrastructure => infrastructure_experts(count),
            PanelTemplate::Product => product_experts(count),
            PanelTemplate::MachineLearning => ml_experts(count),
            PanelTemplate::Governance => governance_experts(count),
            PanelTemplate::General => general_experts(count),
        }
    }
}

fn infrastructure_experts(count: usize) -> Vec<Expert> {
    let all = vec![
        Expert {
            id: "DS".to_string(),
            name: "Distributed Systems Architect".to_string(),
            perspective: "Consistency, availability, partition tolerance".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SEC".to_string(),
            name: "Security Engineer".to_string(),
            perspective: "Threat modeling, defense in depth".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DBA".to_string(),
            name: "Database Architect".to_string(),
            perspective: "Data integrity, query optimization".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SRE".to_string(),
            name: "Site Reliability Engineer".to_string(),
            perspective: "Uptime, observability, incident response".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "API".to_string(),
            name: "API Designer".to_string(),
            perspective: "Interface contracts, versioning, ergonomics".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DX".to_string(),
            name: "Developer Experience Lead".to_string(),
            perspective: "Tooling, documentation, onboarding".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "NET".to_string(),
            name: "Network Engineer".to_string(),
            perspective: "Latency, throughput, topology".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "IAC".to_string(),
            name: "Infrastructure as Code Specialist".to_string(),
            perspective: "Reproducibility, drift detection".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "COST".to_string(),
            name: "Cloud Cost Analyst".to_string(),
            perspective: "Resource optimization, TCO".to_string(),
            domain: "Finance".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PRIV".to_string(),
            name: "Privacy Engineer".to_string(),
            perspective: "Data minimization, compliance".to_string(),
            domain: "Legal".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ARCH".to_string(),
            name: "Solutions Architect".to_string(),
            perspective: "Integration patterns, trade-offs".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PERF".to_string(),
            name: "Performance Engineer".to_string(),
            perspective: "Profiling, optimization, benchmarking".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
    ];
    all.into_iter().take(count).collect()
}

fn product_experts(count: usize) -> Vec<Expert> {
    let all = vec![
        Expert {
            id: "PM".to_string(),
            name: "Product Manager".to_string(),
            perspective: "User value, market fit, prioritization".to_string(),
            domain: "Product".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "UX".to_string(),
            name: "UX Designer".to_string(),
            perspective: "User flows, accessibility, delight".to_string(),
            domain: "Design".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ENG".to_string(),
            name: "Engineering Lead".to_string(),
            perspective: "Feasibility, technical debt, velocity".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "QA".to_string(),
            name: "Quality Assurance Lead".to_string(),
            perspective: "Edge cases, regression, test coverage".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DATA".to_string(),
            name: "Data Analyst".to_string(),
            perspective: "Metrics, A/B testing, insights".to_string(),
            domain: "Analytics".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SUP".to_string(),
            name: "Customer Support Lead".to_string(),
            perspective: "Pain points, friction, feedback".to_string(),
            domain: "Support".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "MKT".to_string(),
            name: "Marketing Lead".to_string(),
            perspective: "Positioning, messaging, GTM".to_string(),
            domain: "Marketing".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "LEGAL".to_string(),
            name: "Legal Counsel".to_string(),
            perspective: "Compliance, risk, terms".to_string(),
            domain: "Legal".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "BIZ".to_string(),
            name: "Business Development".to_string(),
            perspective: "Partnerships, ecosystem, growth".to_string(),
            domain: "Business".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "FIN".to_string(),
            name: "Finance Analyst".to_string(),
            perspective: "Unit economics, burn, runway".to_string(),
            domain: "Finance".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "OPS".to_string(),
            name: "Operations Lead".to_string(),
            perspective: "Process, scalability, efficiency".to_string(),
            domain: "Operations".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "COMM".to_string(),
            name: "Community Manager".to_string(),
            perspective: "Engagement, advocacy, feedback loops".to_string(),
            domain: "Community".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
    ];
    all.into_iter().take(count).collect()
}

fn ml_experts(count: usize) -> Vec<Expert> {
    let all = vec![
        Expert {
            id: "MLE".to_string(),
            name: "ML Engineer".to_string(),
            perspective: "Model architecture, training, inference".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DS".to_string(),
            name: "Data Scientist".to_string(),
            perspective: "Feature engineering, experimentation".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "MLOPS".to_string(),
            name: "MLOps Engineer".to_string(),
            perspective: "Model serving, monitoring, retraining".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ETHICS".to_string(),
            name: "AI Ethics Researcher".to_string(),
            perspective: "Bias, fairness, transparency".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "NLP".to_string(),
            name: "NLP Specialist".to_string(),
            perspective: "Language understanding, generation".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "CV".to_string(),
            name: "Computer Vision Expert".to_string(),
            perspective: "Image understanding, spatial reasoning".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DE".to_string(),
            name: "Data Engineer".to_string(),
            perspective: "Pipelines, quality, scale".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ALIGN".to_string(),
            name: "AI Alignment Researcher".to_string(),
            perspective: "Safety, alignment, interpretability".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PROD".to_string(),
            name: "ML Product Manager".to_string(),
            perspective: "Use cases, evaluation, productization".to_string(),
            domain: "Product".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "HW".to_string(),
            name: "ML Hardware Specialist".to_string(),
            perspective: "GPU/TPU optimization, inference efficiency".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "RES".to_string(),
            name: "Research Scientist".to_string(),
            perspective: "State of art, novel approaches".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "EVAL".to_string(),
            name: "Evaluation Specialist".to_string(),
            perspective: "Benchmarks, metrics, ground truth".to_string(),
            domain: "AI".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
    ];
    all.into_iter().take(count).collect()
}

fn governance_experts(count: usize) -> Vec<Expert> {
    let all = vec![
        Expert {
            id: "GOV".to_string(),
            name: "Governance Specialist".to_string(),
            perspective: "Decision processes, accountability".to_string(),
            domain: "Governance".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "LAW".to_string(),
            name: "Legal Scholar".to_string(),
            perspective: "Regulatory compliance, precedent".to_string(),
            domain: "Legal".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ECON".to_string(),
            name: "Economist".to_string(),
            perspective: "Incentives, market dynamics".to_string(),
            domain: "Economics".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PHIL".to_string(),
            name: "Philosopher".to_string(),
            perspective: "Ethics, values, first principles".to_string(),
            domain: "Philosophy".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "GAME".to_string(),
            name: "Game Theorist".to_string(),
            perspective: "Strategic interaction, equilibria".to_string(),
            domain: "Game Theory".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SOC".to_string(),
            name: "Sociologist".to_string(),
            perspective: "Social dynamics, institutions".to_string(),
            domain: "Social".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DAO".to_string(),
            name: "DAO Researcher".to_string(),
            perspective: "Decentralized coordination, tokenomics".to_string(),
            domain: "Governance".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "TRUST".to_string(),
            name: "Trust Researcher".to_string(),
            perspective: "Reputation, verification, cooperation".to_string(),
            domain: "Social".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "HIST".to_string(),
            name: "Historian".to_string(),
            perspective: "Precedent, patterns, context".to_string(),
            domain: "Humanities".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PSY".to_string(),
            name: "Psychologist".to_string(),
            perspective: "Behavior, motivation, bias".to_string(),
            domain: "Psychology".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SYS".to_string(),
            name: "Systems Thinker".to_string(),
            perspective: "Feedback loops, emergence, complexity".to_string(),
            domain: "Systems".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "COMM".to_string(),
            name: "Community Organizer".to_string(),
            perspective: "Participation, voice, inclusion".to_string(),
            domain: "Community".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
    ];
    all.into_iter().take(count).collect()
}

fn general_experts(count: usize) -> Vec<Expert> {
    // Mix from all domains
    let all = vec![
        Expert {
            id: "ARCH".to_string(),
            name: "Solutions Architect".to_string(),
            perspective: "Integration, trade-offs, patterns".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "PM".to_string(),
            name: "Product Manager".to_string(),
            perspective: "User value, prioritization".to_string(),
            domain: "Product".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SEC".to_string(),
            name: "Security Engineer".to_string(),
            perspective: "Threat modeling, defense".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "UX".to_string(),
            name: "UX Designer".to_string(),
            perspective: "User experience, accessibility".to_string(),
            domain: "Design".to_string(),
            tier: ExpertTier::Core,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "ENG".to_string(),
            name: "Senior Engineer".to_string(),
            perspective: "Implementation, maintenance".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "QA".to_string(),
            name: "QA Lead".to_string(),
            perspective: "Quality, edge cases, testing".to_string(),
            domain: "Tech".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "OPS".to_string(),
            name: "Operations Lead".to_string(),
            perspective: "Reliability, process, scale".to_string(),
            domain: "Operations".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DATA".to_string(),
            name: "Data Analyst".to_string(),
            perspective: "Metrics, insights, evidence".to_string(),
            domain: "Analytics".to_string(),
            tier: ExpertTier::Adjacent,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "LEGAL".to_string(),
            name: "Legal Counsel".to_string(),
            perspective: "Compliance, risk, terms".to_string(),
            domain: "Legal".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "FIN".to_string(),
            name: "Finance Analyst".to_string(),
            perspective: "Costs, ROI, budgets".to_string(),
            domain: "Finance".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "SUP".to_string(),
            name: "Support Lead".to_string(),
            perspective: "User pain points, feedback".to_string(),
            domain: "Support".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
        Expert {
            id: "DOC".to_string(),
            name: "Technical Writer".to_string(),
            perspective: "Clarity, documentation, onboarding".to_string(),
            domain: "Documentation".to_string(),
            tier: ExpertTier::Wildcard,
            emoji: Some("🧁".to_string()),
        },
    ];
    all.into_iter().take(count).collect()
}

/// Build an expert prompt for a dialogue round
pub fn build_expert_prompt(
    expert: &Expert,
    topic: &str,
    constraint: Option<&str>,
    round: u32,
    previous_rounds: &str,
) -> String {
    let constraint_text = constraint
        .map(|c| format!("\n**Constraint**: {}", c))
        .unwrap_or_default();

    let round_instruction = if round == 0 {
        "This is the OPENING ARGUMENTS round. Provide your independent perspective on the topic. Do not assume others' positions."
    } else {
        "Review previous rounds and respond. Build on good ideas, challenge weak ones, surface new perspectives."
    };

    format!(
        r#"You are {name} 🧁 in an ALIGNMENT-seeking dialogue.

**Topic**: {topic}{constraint}

**Your Expertise**: {perspective}
**Your Domain**: {domain}
**Your Tier**: {tier:?}

## Your Role

- SURFACE perspectives others may have missed
- DEFEND valuable ideas with love, not ego
- CHALLENGE assumptions with curiosity, not destruction
- INTEGRATE perspectives that resonate
- CONCEDE gracefully when others see something you missed
- CELEBRATE when others make the solution stronger

## Round {round}: {round_instruction}

{previous}

## Response Format

### {id} 🧁

[Your response - be specific, cite evidence, explain reasoning]

Use these markers:
- [PERSPECTIVE Pxx: ...] - new viewpoint you're surfacing
- [TENSION Tx: ...] - unresolved issue needing attention
- [REFINEMENT: ...] - when you're improving the proposal
- [CONCESSION: ...] - when another 🧁 was right
- [RESOLVED Tx: ...] - when addressing a tension

End with a 1-2 sentence position statement and confidence level (0.0-1.0).

**Position**: [Your stance in 1-2 sentences]
**Confidence**: [0.0-1.0]"#,
        name = expert.name,
        topic = topic,
        constraint = constraint_text,
        perspective = expert.perspective,
        domain = expert.domain,
        tier = expert.tier,
        round = round,
        round_instruction = round_instruction,
        previous = if previous_rounds.is_empty() {
            String::new()
        } else {
            format!("## Previous Rounds\n\n{}", previous_rounds)
        },
        id = expert.id,
    )
}

/// Parse an expert response from LLM output
pub fn parse_expert_response(expert_id: &str, content: &str) -> ExpertResponse {
    let mut perspectives = Vec::new();
    let mut tensions = Vec::new();
    let mut refinements = Vec::new();
    let mut concessions = Vec::new();
    let mut resolved = Vec::new();
    let mut position = String::new();
    let mut confidence = 0.5;

    // Extract markers
    for line in content.lines() {
        if line.contains("[PERSPECTIVE") {
            if let Some(p) = extract_marker(line, "PERSPECTIVE") {
                perspectives.push(Perspective {
                    id: format!("P{:02}", perspectives.len() + 1),
                    description: p,
                    surfaced_by: expert_id.to_string(),
                    round: 0, // Filled in by caller
                    status: PerspectiveStatus::Active,
                });
            }
        } else if line.contains("[TENSION") {
            if let Some(t) = extract_marker(line, "TENSION") {
                tensions.push(Tension {
                    id: format!("T{}", tensions.len() + 1),
                    description: t,
                    position_a: String::new(),
                    position_b: String::new(),
                    status: TensionStatus::Open,
                });
            }
        } else if line.contains("[REFINEMENT") {
            if let Some(r) = extract_marker(line, "REFINEMENT") {
                refinements.push(r);
            }
        } else if line.contains("[CONCESSION") {
            if let Some(c) = extract_marker(line, "CONCESSION") {
                concessions.push(c);
            }
        } else if line.contains("[RESOLVED") {
            if let Some(r) = extract_marker(line, "RESOLVED") {
                resolved.push(r);
            }
        } else if line.starts_with("**Position**:") {
            position = line.trim_start_matches("**Position**:").trim().to_string();
        } else if line.starts_with("**Confidence**:") {
            if let Ok(c) = line
                .trim_start_matches("**Confidence**:")
                .trim()
                .parse::<f64>()
            {
                confidence = c.clamp(0.0, 1.0);
            }
        }
    }

    // Calculate score based on contributions
    let score = AlignmentScore {
        wisdom: perspectives.len() as u32 * 3 + refinements.len() as u32 * 2,
        consistency: if confidence >= 0.7 { 2 } else { 1 },
        truth: if !position.is_empty() { 2 } else { 0 },
        relationships: concessions.len() as u32 + resolved.len() as u32,
    };

    ExpertResponse {
        expert_id: expert_id.to_string(),
        content: content.to_string(),
        position,
        confidence,
        perspectives,
        tensions,
        refinements,
        concessions,
        resolved_tensions: resolved,
        score,
    }
}

fn extract_marker(line: &str, marker: &str) -> Option<String> {
    let start = line.find(&format!("[{}", marker))?;
    let end = line[start..].find(']')?;
    let content = &line[start + marker.len() + 1..start + end];
    // Remove the marker prefix (like "Pxx:" or "Tx:")
    let clean = content
        .trim()
        .trim_start_matches(|c: char| c.is_alphanumeric() || c == ':')
        .trim();
    if clean.is_empty() {
        None
    } else {
        Some(clean.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expert_tier_from_relevance() {
        assert_eq!(ExpertTier::from_relevance(0.9), ExpertTier::Core);
        assert_eq!(ExpertTier::from_relevance(0.8), ExpertTier::Core);
        assert_eq!(ExpertTier::from_relevance(0.6), ExpertTier::Adjacent);
        assert_eq!(ExpertTier::from_relevance(0.4), ExpertTier::Adjacent);
        assert_eq!(ExpertTier::from_relevance(0.3), ExpertTier::Wildcard);
    }

    #[test]
    fn test_alignment_score_total() {
        let score = AlignmentScore {
            wisdom: 10,
            consistency: 5,
            truth: 3,
            relationships: 2,
        };
        assert_eq!(score.total(), 20);
    }

    #[test]
    fn test_panel_template_generates_experts() {
        let experts = PanelTemplate::Infrastructure.generate_experts(5);
        assert_eq!(experts.len(), 5);
        assert!(experts.iter().all(|e| !e.id.is_empty()));
    }

    #[test]
    fn test_dialogue_should_continue() {
        let experts = PanelTemplate::General.generate_experts(3);
        let mut dialogue = AlignmentDialogue::new("Test topic".to_string(), None, experts);

        // Should continue when fresh
        assert!(dialogue.should_continue());

        // Simulate convergence
        dialogue.status = DialogueStatus::Converged;
        assert!(!dialogue.should_continue());
    }

    #[test]
    fn test_parse_expert_response() {
        let content = r#"### DS 🧁

This is my analysis of the situation.

[PERSPECTIVE P01: We need to consider CAP theorem implications]
[TENSION T1: Consistency vs availability trade-off]

I think we should prioritize consistency.

**Position**: Prioritize strong consistency with eventual availability fallback.
**Confidence**: 0.85"#;

        let response = parse_expert_response("DS", content);
        assert_eq!(response.expert_id, "DS");
        assert_eq!(response.perspectives.len(), 1);
        assert_eq!(response.tensions.len(), 1);
        assert!((response.confidence - 0.85).abs() < 0.01);
        assert!(response.position.contains("consistency"));
    }
}
