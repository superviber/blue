//! Alignment dialogue database operations (RFC 0051)
//!
//! DB-first architecture for tracking perspectives, tensions, recommendations,
//! evidence, and claims with full lifecycle and cross-referencing.

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AlignmentDbError {
    #[error("Database error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Dialogue not found: {0}")]
    DialogueNotFound(String),

    #[error("Expert not found: {0}")]
    ExpertNotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Invalid state transition: {entity} from '{from}' to '{to}'")]
    InvalidTransition {
        entity: String,
        from: String,
        to: String,
    },

    #[error("Reference not found: {0}")]
    RefNotFound(String),
}

// ==================== Enums ====================

/// Dialogue status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DialogueStatus {
    Open,
    Converging,
    Converged,
    Abandoned,
}

impl DialogueStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Converging => "converging",
            Self::Converged => "converged",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "converging" => Self::Converging,
            "converged" => Self::Converged,
            "abandoned" => Self::Abandoned,
            _ => Self::Open,
        }
    }
}

/// Expert tier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExpertTier {
    Core,
    Adjacent,
    Wildcard,
}

impl ExpertTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Adjacent => "Adjacent",
            Self::Wildcard => "Wildcard",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Adjacent" | "adjacent" => Self::Adjacent,
            "Wildcard" | "wildcard" => Self::Wildcard,
            _ => Self::Core,
        }
    }
}

/// Expert source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExpertSource {
    Pool,
    Created,
    Retained,
}

impl ExpertSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pool => "pool",
            Self::Created => "created",
            Self::Retained => "retained",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "created" => Self::Created,
            "retained" => Self::Retained,
            _ => Self::Pool,
        }
    }
}

/// Perspective status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PerspectiveStatus {
    Open,
    Refined,
    Conceded,
    Merged,
}

impl PerspectiveStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Refined => "refined",
            Self::Conceded => "conceded",
            Self::Merged => "merged",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "refined" => Self::Refined,
            "conceded" => Self::Conceded,
            "merged" => Self::Merged,
            _ => Self::Open,
        }
    }
}

/// Tension status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TensionStatus {
    Open,
    Addressed,
    Resolved,
    Reopened,
}

impl TensionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Addressed => "addressed",
            Self::Resolved => "resolved",
            Self::Reopened => "reopened",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "addressed" => Self::Addressed,
            "resolved" => Self::Resolved,
            "reopened" => Self::Reopened,
            _ => Self::Open,
        }
    }
}

/// Recommendation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecommendationStatus {
    Proposed,
    Amended,
    Adopted,
    Rejected,
}

impl RecommendationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Amended => "amended",
            Self::Adopted => "adopted",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "amended" => Self::Amended,
            "adopted" => Self::Adopted,
            "rejected" => Self::Rejected,
            _ => Self::Proposed,
        }
    }
}

/// Evidence status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceStatus {
    Cited,
    Challenged,
    Confirmed,
    Refuted,
}

impl EvidenceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cited => "cited",
            Self::Challenged => "challenged",
            Self::Confirmed => "confirmed",
            Self::Refuted => "refuted",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "challenged" => Self::Challenged,
            "confirmed" => Self::Confirmed,
            "refuted" => Self::Refuted,
            _ => Self::Cited,
        }
    }
}

/// Claim status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClaimStatus {
    Asserted,
    Supported,
    Opposed,
    Adopted,
    Withdrawn,
}

impl ClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Asserted => "asserted",
            Self::Supported => "supported",
            Self::Opposed => "opposed",
            Self::Adopted => "adopted",
            Self::Withdrawn => "withdrawn",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "supported" => Self::Supported,
            "opposed" => Self::Opposed,
            "adopted" => Self::Adopted,
            "withdrawn" => Self::Withdrawn,
            _ => Self::Asserted,
        }
    }
}

/// Entity type for cross-references
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityType {
    #[serde(rename = "P")]
    Perspective,
    #[serde(rename = "R")]
    Recommendation,
    #[serde(rename = "T")]
    Tension,
    #[serde(rename = "E")]
    Evidence,
    #[serde(rename = "C")]
    Claim,
}

impl EntityType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Perspective => "P",
            Self::Recommendation => "R",
            Self::Tension => "T",
            Self::Evidence => "E",
            Self::Claim => "C",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "P" => Some(Self::Perspective),
            "R" => Some(Self::Recommendation),
            "T" => Some(Self::Tension),
            "E" => Some(Self::Evidence),
            "C" => Some(Self::Claim),
            _ => None,
        }
    }
}

/// Reference type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefType {
    Support,
    Oppose,
    Refine,
    Address,
    Resolve,
    Reopen,
    Question,
    Depend,
}

impl RefType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Support => "support",
            Self::Oppose => "oppose",
            Self::Refine => "refine",
            Self::Address => "address",
            Self::Resolve => "resolve",
            Self::Reopen => "reopen",
            Self::Question => "question",
            Self::Depend => "depend",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "support" => Some(Self::Support),
            "oppose" => Some(Self::Oppose),
            "refine" => Some(Self::Refine),
            "address" => Some(Self::Address),
            "resolve" => Some(Self::Resolve),
            "reopen" => Some(Self::Reopen),
            "question" => Some(Self::Question),
            "depend" => Some(Self::Depend),
            _ => None,
        }
    }
}

/// Verdict type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerdictType {
    Interim,
    Final,
    Minority,
    Dissent,
}

impl VerdictType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Interim => "interim",
            Self::Final => "final",
            Self::Minority => "minority",
            Self::Dissent => "dissent",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "final" => Self::Final,
            "minority" => Self::Minority,
            "dissent" => Self::Dissent,
            _ => Self::Interim,
        }
    }
}

/// Move type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MoveType {
    Defend,
    Challenge,
    Bridge,
    Request,
    Concede,
    Converge,
}

impl MoveType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Defend => "defend",
            Self::Challenge => "challenge",
            Self::Bridge => "bridge",
            Self::Request => "request",
            Self::Concede => "concede",
            Self::Converge => "converge",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "defend" => Some(Self::Defend),
            "challenge" => Some(Self::Challenge),
            "bridge" => Some(Self::Bridge),
            "request" => Some(Self::Request),
            "concede" => Some(Self::Concede),
            "converge" => Some(Self::Converge),
            _ => None,
        }
    }
}

// ==================== Data Types ====================

/// Dialogue background context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueBackground {
    pub subject: String,
    pub description: Option<String>,
    pub constraints: Option<serde_json::Value>,
    pub situation: Option<String>,
}

/// Alignment dialogue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dialogue {
    pub dialogue_id: String,
    pub title: String,
    pub question: Option<String>,
    pub status: DialogueStatus,
    pub created_at: DateTime<Utc>,
    pub converged_at: Option<DateTime<Utc>>,
    pub total_rounds: i32,
    pub total_alignment: i32,
    pub output_dir: Option<String>,
    pub calibrated: bool,
    pub domain_id: Option<String>,
    pub ethos_id: Option<String>,
    pub background: Option<DialogueBackground>,
}

/// Expert in a dialogue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueExpert {
    pub dialogue_id: String,
    pub expert_slug: String,
    pub role: String,
    pub description: Option<String>,
    pub focus: Option<String>,
    pub tier: ExpertTier,
    pub source: ExpertSource,
    pub relevance: Option<f64>,
    pub creation_reason: Option<String>,
    pub color: Option<String>,
    pub scores: serde_json::Value,
    pub raw_content: Option<serde_json::Value>,
    pub total_score: i32,
    pub first_round: Option<i32>,
    pub created_at: DateTime<Utc>,
}

/// Perspective
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbPerspective {
    pub dialogue_id: String,
    pub round: i32,
    pub seq: i32,
    pub label: String,
    pub content: String,
    pub contributors: Vec<String>,
    pub status: PerspectiveStatus,
    pub refs: Option<Vec<Reference>>,
    pub created_at: DateTime<Utc>,
}

/// Tension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbTension {
    pub dialogue_id: String,
    pub round: i32,
    pub seq: i32,
    pub label: String,
    pub description: String,
    pub contributors: Vec<String>,
    pub status: TensionStatus,
    pub refs: Option<Vec<Reference>>,
    pub created_at: DateTime<Utc>,
}

/// Recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbRecommendation {
    pub dialogue_id: String,
    pub round: i32,
    pub seq: i32,
    pub label: String,
    pub content: String,
    pub contributors: Vec<String>,
    pub parameters: Option<serde_json::Value>,
    pub status: RecommendationStatus,
    pub refs: Option<Vec<Reference>>,
    pub adopted_in_verdict: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbEvidence {
    pub dialogue_id: String,
    pub round: i32,
    pub seq: i32,
    pub label: String,
    pub content: String,
    pub contributors: Vec<String>,
    pub status: EvidenceStatus,
    pub refs: Option<Vec<Reference>>,
    pub created_at: DateTime<Utc>,
}

/// Claim
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbClaim {
    pub dialogue_id: String,
    pub round: i32,
    pub seq: i32,
    pub label: String,
    pub content: String,
    pub contributors: Vec<String>,
    pub status: ClaimStatus,
    pub refs: Option<Vec<Reference>>,
    pub created_at: DateTime<Utc>,
}

/// Cross-reference between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reference {
    #[serde(rename = "type")]
    pub ref_type: RefType,
    pub target: String,
}

/// Verdict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verdict {
    pub dialogue_id: String,
    pub verdict_id: String,
    pub verdict_type: VerdictType,
    pub round: i32,
    pub author_expert: Option<String>,
    pub recommendation: String,
    pub description: String,
    pub conditions: Option<Vec<String>>,
    pub vote: Option<String>,
    pub confidence: Option<String>,
    pub tensions_resolved: Option<Vec<String>>,
    pub tensions_accepted: Option<Vec<String>>,
    pub recommendations_adopted: Option<Vec<String>>,
    pub key_evidence: Option<Vec<String>>,
    pub key_claims: Option<Vec<String>>,
    pub supporting_experts: Option<Vec<String>>,
    pub ethos_compliance: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

// ==================== ID Generation ====================

/// Generate a collision-safe dialogue ID from title
pub fn generate_dialogue_id(conn: &Connection, title: &str) -> Result<String, AlignmentDbError> {
    let slug = slugify(title);

    // Check if base slug exists
    let exists: bool = conn
        .query_row(
            "SELECT 1 FROM alignment_dialogues WHERE dialogue_id = ?1",
            params![&slug],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);

    if !exists {
        return Ok(slug);
    }

    // Try numbered suffixes
    for i in 2..=99 {
        let candidate = format!("{}-{}", slug, i);
        let exists: bool = conn
            .query_row(
                "SELECT 1 FROM alignment_dialogues WHERE dialogue_id = ?1",
                params![&candidate],
                |_| Ok(true),
            )
            .optional()?
            .unwrap_or(false);

        if !exists {
            return Ok(candidate);
        }
    }

    Err(AlignmentDbError::Validation(
        "Too many dialogues with similar titles".to_string(),
    ))
}

/// Generate display ID from round and seq: P{round:02d}{seq:02d}
pub fn display_id(entity_type: EntityType, round: i32, seq: i32) -> String {
    format!("{}{:02}{:02}", entity_type.as_str(), round, seq)
}

/// Parse display ID to extract round and seq
pub fn parse_display_id(id: &str) -> Option<(EntityType, i32, i32)> {
    if id.len() != 5 {
        return None;
    }

    let entity_type = EntityType::from_str(&id[0..1])?;
    let round = id[1..3].parse().ok()?;
    let seq = id[3..5].parse().ok()?;

    Some((entity_type, round, seq))
}

/// Simple slugification
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ==================== Database Operations ====================

/// Create a new dialogue
pub fn create_dialogue(
    conn: &Connection,
    title: &str,
    question: Option<&str>,
    output_dir: Option<&str>,
    background: Option<&DialogueBackground>,
) -> Result<String, AlignmentDbError> {
    let dialogue_id = generate_dialogue_id(conn, title)?;
    let now = Utc::now().to_rfc3339();
    let background_json = background.map(|b| serde_json::to_string(b).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_dialogues
         (dialogue_id, title, question, status, created_at, output_dir, background)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![dialogue_id, title, question, "open", now, output_dir, background_json,],
    )?;

    Ok(dialogue_id)
}

/// Get a dialogue by ID
pub fn get_dialogue(conn: &Connection, dialogue_id: &str) -> Result<Dialogue, AlignmentDbError> {
    conn.query_row(
        "SELECT dialogue_id, title, question, status, created_at, converged_at,
                total_rounds, total_alignment, output_dir, calibrated, domain_id,
                ethos_id, background
         FROM alignment_dialogues WHERE dialogue_id = ?1",
        params![dialogue_id],
        |row| {
            let background: Option<String> = row.get(12)?;
            Ok(Dialogue {
                dialogue_id: row.get(0)?,
                title: row.get(1)?,
                question: row.get(2)?,
                status: DialogueStatus::from_str(&row.get::<_, String>(3)?),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
                converged_at: row
                    .get::<_, Option<String>>(5)?
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                total_rounds: row.get(6)?,
                total_alignment: row.get(7)?,
                output_dir: row.get(8)?,
                calibrated: row.get::<_, i32>(9)? != 0,
                domain_id: row.get(10)?,
                ethos_id: row.get(11)?,
                background: background.and_then(|s| serde_json::from_str(&s).ok()),
            })
        },
    )
    .map_err(|_| AlignmentDbError::DialogueNotFound(dialogue_id.to_string()))
}

/// Register an expert in a dialogue
pub fn register_expert(
    conn: &Connection,
    dialogue_id: &str,
    expert_slug: &str,
    role: &str,
    tier: ExpertTier,
    source: ExpertSource,
    description: Option<&str>,
    focus: Option<&str>,
    relevance: Option<f64>,
    creation_reason: Option<&str>,
    color: Option<&str>,
    first_round: Option<i32>,
) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO alignment_experts
         (dialogue_id, expert_slug, role, description, focus, tier, source,
          relevance, creation_reason, color, scores, first_round, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            dialogue_id,
            expert_slug,
            role,
            description,
            focus,
            tier.as_str(),
            source.as_str(),
            relevance,
            creation_reason,
            color,
            "{}", // Empty scores JSON
            first_round,
            now,
        ],
    )?;

    Ok(())
}

/// Get experts for a dialogue
pub fn get_experts(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<DialogueExpert>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, expert_slug, role, description, focus, tier, source,
                relevance, creation_reason, color, scores, raw_content, total_score,
                first_round, created_at
         FROM alignment_experts WHERE dialogue_id = ?1",
    )?;

    let experts = stmt
        .query_map(params![dialogue_id], |row| {
            let scores: String = row.get(10)?;
            let raw_content: Option<String> = row.get(11)?;
            Ok(DialogueExpert {
                dialogue_id: row.get(0)?,
                expert_slug: row.get(1)?,
                role: row.get(2)?,
                description: row.get(3)?,
                focus: row.get(4)?,
                tier: ExpertTier::from_str(&row.get::<_, String>(5)?),
                source: ExpertSource::from_str(&row.get::<_, String>(6)?),
                relevance: row.get(7)?,
                creation_reason: row.get(8)?,
                color: row.get(9)?,
                scores: serde_json::from_str(&scores).unwrap_or(serde_json::json!({})),
                raw_content: raw_content.and_then(|s| serde_json::from_str(&s).ok()),
                total_score: row.get(12)?,
                first_round: row.get(13)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(14)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(experts)
}

/// Create a new round
pub fn create_round(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    title: Option<&str>,
    score: i32,
) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO alignment_rounds
         (dialogue_id, round, title, score, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![dialogue_id, round, title, score, "open", now],
    )?;

    // Update dialogue total rounds
    conn.execute(
        "UPDATE alignment_dialogues SET total_rounds = ?1, total_alignment = total_alignment + ?2
         WHERE dialogue_id = ?3",
        params![round + 1, score, dialogue_id],
    )?;

    Ok(())
}

/// Get next sequence number for an entity type in a round
pub fn next_seq(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    entity_type: EntityType,
) -> Result<i32, AlignmentDbError> {
    let table = match entity_type {
        EntityType::Perspective => "alignment_perspectives",
        EntityType::Recommendation => "alignment_recommendations",
        EntityType::Tension => "alignment_tensions",
        EntityType::Evidence => "alignment_evidence",
        EntityType::Claim => "alignment_claims",
    };

    let max_seq: Option<i32> = conn
        .query_row(
            &format!(
                "SELECT MAX(seq) FROM {} WHERE dialogue_id = ?1 AND round = ?2",
                table
            ),
            params![dialogue_id, round],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    Ok(max_seq.unwrap_or(0) + 1)
}

/// Register a perspective
pub fn register_perspective(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    label: &str,
    content: &str,
    contributors: &[String],
    refs: Option<&[Reference]>,
) -> Result<String, AlignmentDbError> {
    let seq = next_seq(conn, dialogue_id, round, EntityType::Perspective)?;
    let now = Utc::now().to_rfc3339();
    let contributors_json = serde_json::to_string(contributors).unwrap_or_default();
    let refs_json = refs.map(|r| serde_json::to_string(r).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_perspectives
         (dialogue_id, round, seq, label, content, contributors, status, refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            dialogue_id,
            round,
            seq,
            label,
            content,
            contributors_json,
            "open",
            refs_json,
            now,
        ],
    )?;

    let display = display_id(EntityType::Perspective, round, seq);

    // Create perspective event
    conn.execute(
        "INSERT INTO alignment_perspective_events
         (dialogue_id, perspective_round, perspective_seq, event_type, event_round, actors, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![dialogue_id, round, seq, "created", round, contributors_json, now],
    )?;

    Ok(display)
}

/// Register a tension
pub fn register_tension(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    label: &str,
    description: &str,
    contributors: &[String],
    refs: Option<&[Reference]>,
) -> Result<String, AlignmentDbError> {
    let seq = next_seq(conn, dialogue_id, round, EntityType::Tension)?;
    let now = Utc::now().to_rfc3339();
    let contributors_json = serde_json::to_string(contributors).unwrap_or_default();
    let refs_json = refs.map(|r| serde_json::to_string(r).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_tensions
         (dialogue_id, round, seq, label, description, contributors, status, refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            dialogue_id,
            round,
            seq,
            label,
            description,
            contributors_json,
            "open",
            refs_json,
            now,
        ],
    )?;

    let display = display_id(EntityType::Tension, round, seq);

    // Create tension event
    conn.execute(
        "INSERT INTO alignment_tension_events
         (dialogue_id, tension_round, tension_seq, event_type, event_round, actors, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![dialogue_id, round, seq, "created", round, contributors_json, now],
    )?;

    Ok(display)
}

/// Register a recommendation
pub fn register_recommendation(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    label: &str,
    content: &str,
    contributors: &[String],
    parameters: Option<&serde_json::Value>,
    refs: Option<&[Reference]>,
) -> Result<String, AlignmentDbError> {
    let seq = next_seq(conn, dialogue_id, round, EntityType::Recommendation)?;
    let now = Utc::now().to_rfc3339();
    let contributors_json = serde_json::to_string(contributors).unwrap_or_default();
    let params_json = parameters.map(|p| serde_json::to_string(p).unwrap_or_default());
    let refs_json = refs.map(|r| serde_json::to_string(r).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_recommendations
         (dialogue_id, round, seq, label, content, contributors, parameters, status, refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            dialogue_id,
            round,
            seq,
            label,
            content,
            contributors_json,
            params_json,
            "proposed",
            refs_json,
            now,
        ],
    )?;

    let display = display_id(EntityType::Recommendation, round, seq);

    // Create recommendation event
    conn.execute(
        "INSERT INTO alignment_recommendation_events
         (dialogue_id, rec_round, rec_seq, event_type, event_round, actors, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![dialogue_id, round, seq, "created", round, contributors_json, now],
    )?;

    Ok(display)
}

/// Register evidence
pub fn register_evidence(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    label: &str,
    content: &str,
    contributors: &[String],
    refs: Option<&[Reference]>,
) -> Result<String, AlignmentDbError> {
    let seq = next_seq(conn, dialogue_id, round, EntityType::Evidence)?;
    let now = Utc::now().to_rfc3339();
    let contributors_json = serde_json::to_string(contributors).unwrap_or_default();
    let refs_json = refs.map(|r| serde_json::to_string(r).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_evidence
         (dialogue_id, round, seq, label, content, contributors, status, refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            dialogue_id,
            round,
            seq,
            label,
            content,
            contributors_json,
            "cited",
            refs_json,
            now,
        ],
    )?;

    Ok(display_id(EntityType::Evidence, round, seq))
}

/// Register a claim
pub fn register_claim(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    label: &str,
    content: &str,
    contributors: &[String],
    refs: Option<&[Reference]>,
) -> Result<String, AlignmentDbError> {
    let seq = next_seq(conn, dialogue_id, round, EntityType::Claim)?;
    let now = Utc::now().to_rfc3339();
    let contributors_json = serde_json::to_string(contributors).unwrap_or_default();
    let refs_json = refs.map(|r| serde_json::to_string(r).unwrap_or_default());

    conn.execute(
        "INSERT INTO alignment_claims
         (dialogue_id, round, seq, label, content, contributors, status, refs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            dialogue_id,
            round,
            seq,
            label,
            content,
            contributors_json,
            "asserted",
            refs_json,
            now,
        ],
    )?;

    Ok(display_id(EntityType::Claim, round, seq))
}

/// Register a cross-reference
pub fn register_ref(
    conn: &Connection,
    dialogue_id: &str,
    source_type: EntityType,
    source_id: &str,
    ref_type: RefType,
    target_type: EntityType,
    target_id: &str,
) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR IGNORE INTO alignment_refs
         (dialogue_id, source_type, source_id, ref_type, target_type, target_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            dialogue_id,
            source_type.as_str(),
            source_id,
            ref_type.as_str(),
            target_type.as_str(),
            target_id,
            now,
        ],
    )?;

    Ok(())
}

/// Register a verdict
pub fn register_verdict(conn: &Connection, verdict: &Verdict) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO alignment_verdicts
         (dialogue_id, verdict_id, verdict_type, round, author_expert, recommendation,
          description, conditions, vote, confidence, tensions_resolved, tensions_accepted,
          recommendations_adopted, key_evidence, key_claims, supporting_experts,
          ethos_compliance, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            verdict.dialogue_id,
            verdict.verdict_id,
            verdict.verdict_type.as_str(),
            verdict.round,
            verdict.author_expert,
            verdict.recommendation,
            verdict.description,
            verdict
                .conditions
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default()),
            verdict.vote,
            verdict.confidence,
            verdict
                .tensions_resolved
                .as_ref()
                .map(|t| serde_json::to_string(t).unwrap_or_default()),
            verdict
                .tensions_accepted
                .as_ref()
                .map(|t| serde_json::to_string(t).unwrap_or_default()),
            verdict
                .recommendations_adopted
                .as_ref()
                .map(|r| serde_json::to_string(r).unwrap_or_default()),
            verdict
                .key_evidence
                .as_ref()
                .map(|e| serde_json::to_string(e).unwrap_or_default()),
            verdict
                .key_claims
                .as_ref()
                .map(|c| serde_json::to_string(c).unwrap_or_default()),
            verdict
                .supporting_experts
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_default()),
            verdict
                .ethos_compliance
                .as_ref()
                .map(|e| serde_json::to_string(e).unwrap_or_default()),
            now,
        ],
    )?;

    // Update dialogue status to converged if final verdict
    if verdict.verdict_type == VerdictType::Final {
        conn.execute(
            "UPDATE alignment_dialogues SET status = 'converged', converged_at = ?1
             WHERE dialogue_id = ?2",
            params![now, verdict.dialogue_id],
        )?;
    }

    Ok(())
}

/// Update tension status
pub fn update_tension_status(
    conn: &Connection,
    dialogue_id: &str,
    tension_id: &str,
    new_status: TensionStatus,
    actors: &[String],
    reference: Option<&str>,
    event_round: i32,
) -> Result<(), AlignmentDbError> {
    let (_, round, seq) = parse_display_id(tension_id).ok_or_else(|| {
        AlignmentDbError::Validation(format!("Invalid tension ID: {}", tension_id))
    })?;

    let now = Utc::now().to_rfc3339();
    let actors_json = serde_json::to_string(actors).unwrap_or_default();

    // Update status
    conn.execute(
        "UPDATE alignment_tensions SET status = ?1 WHERE dialogue_id = ?2 AND round = ?3 AND seq = ?4",
        params![new_status.as_str(), dialogue_id, round, seq],
    )?;

    // Create event
    let event_type = match new_status {
        TensionStatus::Open => "created",
        TensionStatus::Addressed => "addressed",
        TensionStatus::Resolved => "resolved",
        TensionStatus::Reopened => "reopened",
    };

    conn.execute(
        "INSERT INTO alignment_tension_events
         (dialogue_id, tension_round, tension_seq, event_type, event_round, actors, reference, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![dialogue_id, round, seq, event_type, event_round, actors_json, reference, now],
    )?;

    Ok(())
}

/// Update expert score for a round
pub fn update_expert_score(
    conn: &Connection,
    dialogue_id: &str,
    expert_slug: &str,
    round: i32,
    score: i32,
) -> Result<(), AlignmentDbError> {
    // Get current scores
    let current: String = conn
        .query_row(
            "SELECT scores FROM alignment_experts WHERE dialogue_id = ?1 AND expert_slug = ?2",
            params![dialogue_id, expert_slug],
            |row| row.get(0),
        )
        .map_err(|_| AlignmentDbError::ExpertNotFound(expert_slug.to_string()))?;

    let mut scores: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(&current).unwrap_or_default();

    scores.insert(round.to_string(), serde_json::json!(score));

    let total: i32 = scores
        .values()
        .filter_map(|v| v.as_i64())
        .map(|v| v as i32)
        .sum();

    let scores_json = serde_json::to_string(&scores).unwrap_or_default();

    conn.execute(
        "UPDATE alignment_experts SET scores = ?1, total_score = ?2
         WHERE dialogue_id = ?3 AND expert_slug = ?4",
        params![scores_json, total, dialogue_id, expert_slug],
    )?;

    Ok(())
}

/// Get all perspectives for a dialogue
pub fn get_perspectives(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<DbPerspective>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, round, seq, label, content, contributors, status, refs, created_at
         FROM alignment_perspectives WHERE dialogue_id = ?1 ORDER BY round, seq",
    )?;

    let perspectives = stmt
        .query_map(params![dialogue_id], |row| {
            let contributors: String = row.get(5)?;
            let refs: Option<String> = row.get(7)?;
            Ok(DbPerspective {
                dialogue_id: row.get(0)?,
                round: row.get(1)?,
                seq: row.get(2)?,
                label: row.get(3)?,
                content: row.get(4)?,
                contributors: serde_json::from_str(&contributors).unwrap_or_default(),
                status: PerspectiveStatus::from_str(&row.get::<_, String>(6)?),
                refs: refs.and_then(|r| serde_json::from_str(&r).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(perspectives)
}

/// Get all tensions for a dialogue
pub fn get_tensions(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<DbTension>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, round, seq, label, description, contributors, status, refs, created_at
         FROM alignment_tensions WHERE dialogue_id = ?1 ORDER BY round, seq",
    )?;

    let tensions = stmt
        .query_map(params![dialogue_id], |row| {
            let contributors: String = row.get(5)?;
            let refs: Option<String> = row.get(7)?;
            Ok(DbTension {
                dialogue_id: row.get(0)?,
                round: row.get(1)?,
                seq: row.get(2)?,
                label: row.get(3)?,
                description: row.get(4)?,
                contributors: serde_json::from_str(&contributors).unwrap_or_default(),
                status: TensionStatus::from_str(&row.get::<_, String>(6)?),
                refs: refs.and_then(|r| serde_json::from_str(&r).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tensions)
}

/// Get all recommendations for a dialogue
pub fn get_recommendations(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<DbRecommendation>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, round, seq, label, content, contributors, parameters, status,
                refs, adopted_in_verdict, created_at
         FROM alignment_recommendations WHERE dialogue_id = ?1 ORDER BY round, seq",
    )?;

    let recommendations = stmt
        .query_map(params![dialogue_id], |row| {
            let contributors: String = row.get(5)?;
            let parameters: Option<String> = row.get(6)?;
            let refs: Option<String> = row.get(8)?;
            Ok(DbRecommendation {
                dialogue_id: row.get(0)?,
                round: row.get(1)?,
                seq: row.get(2)?,
                label: row.get(3)?,
                content: row.get(4)?,
                contributors: serde_json::from_str(&contributors).unwrap_or_default(),
                parameters: parameters.and_then(|p| serde_json::from_str(&p).ok()),
                status: RecommendationStatus::from_str(&row.get::<_, String>(7)?),
                refs: refs.and_then(|r| serde_json::from_str(&r).ok()),
                adopted_in_verdict: row.get(9)?,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(recommendations)
}

/// Get all evidence for a dialogue
pub fn get_evidence(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<DbEvidence>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, round, seq, label, content, contributors, status, refs, created_at
         FROM alignment_evidence WHERE dialogue_id = ?1 ORDER BY round, seq",
    )?;

    let evidence = stmt
        .query_map(params![dialogue_id], |row| {
            let contributors: String = row.get(5)?;
            let refs: Option<String> = row.get(7)?;
            Ok(DbEvidence {
                dialogue_id: row.get(0)?,
                round: row.get(1)?,
                seq: row.get(2)?,
                label: row.get(3)?,
                content: row.get(4)?,
                contributors: serde_json::from_str(&contributors).unwrap_or_default(),
                status: EvidenceStatus::from_str(&row.get::<_, String>(6)?),
                refs: refs.and_then(|r| serde_json::from_str(&r).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(evidence)
}

/// Get all claims for a dialogue
pub fn get_claims(conn: &Connection, dialogue_id: &str) -> Result<Vec<DbClaim>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, round, seq, label, content, contributors, status, refs, created_at
         FROM alignment_claims WHERE dialogue_id = ?1 ORDER BY round, seq",
    )?;

    let claims = stmt
        .query_map(params![dialogue_id], |row| {
            let contributors: String = row.get(5)?;
            let refs: Option<String> = row.get(7)?;
            Ok(DbClaim {
                dialogue_id: row.get(0)?,
                round: row.get(1)?,
                seq: row.get(2)?,
                label: row.get(3)?,
                content: row.get(4)?,
                contributors: serde_json::from_str(&contributors).unwrap_or_default(),
                status: ClaimStatus::from_str(&row.get::<_, String>(6)?),
                refs: refs.and_then(|r| serde_json::from_str(&r).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(8)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(claims)
}

/// Get verdicts for a dialogue
pub fn get_verdicts(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<Verdict>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, verdict_id, verdict_type, round, author_expert, recommendation,
                description, conditions, vote, confidence, tensions_resolved, tensions_accepted,
                recommendations_adopted, key_evidence, key_claims, supporting_experts,
                ethos_compliance, created_at
         FROM alignment_verdicts WHERE dialogue_id = ?1 ORDER BY round, created_at",
    )?;

    let verdicts = stmt
        .query_map(params![dialogue_id], |row| {
            let conditions: Option<String> = row.get(7)?;
            let tensions_resolved: Option<String> = row.get(10)?;
            let tensions_accepted: Option<String> = row.get(11)?;
            let recommendations_adopted: Option<String> = row.get(12)?;
            let key_evidence: Option<String> = row.get(13)?;
            let key_claims: Option<String> = row.get(14)?;
            let supporting_experts: Option<String> = row.get(15)?;
            let ethos_compliance: Option<String> = row.get(16)?;

            Ok(Verdict {
                dialogue_id: row.get(0)?,
                verdict_id: row.get(1)?,
                verdict_type: VerdictType::from_str(&row.get::<_, String>(2)?),
                round: row.get(3)?,
                author_expert: row.get(4)?,
                recommendation: row.get(5)?,
                description: row.get(6)?,
                conditions: conditions.and_then(|c| serde_json::from_str(&c).ok()),
                vote: row.get(8)?,
                confidence: row.get(9)?,
                tensions_resolved: tensions_resolved.and_then(|t| serde_json::from_str(&t).ok()),
                tensions_accepted: tensions_accepted.and_then(|t| serde_json::from_str(&t).ok()),
                recommendations_adopted: recommendations_adopted
                    .and_then(|r| serde_json::from_str(&r).ok()),
                key_evidence: key_evidence.and_then(|e| serde_json::from_str(&e).ok()),
                key_claims: key_claims.and_then(|c| serde_json::from_str(&c).ok()),
                supporting_experts: supporting_experts.and_then(|s| serde_json::from_str(&s).ok()),
                ethos_compliance: ethos_compliance.and_then(|e| serde_json::from_str(&e).ok()),
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(17)?)
                    .unwrap_or_else(|_| Utc::now().into())
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(verdicts)
}
