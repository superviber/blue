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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Create an in-memory database with the alignment schema
    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();

        // Create all alignment tables
        conn.execute_batch(
            r#"
            CREATE TABLE alignment_dialogues (
                dialogue_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                question TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL,
                converged_at TEXT,
                total_rounds INTEGER DEFAULT 0,
                total_alignment INTEGER DEFAULT 0,
                output_dir TEXT,
                calibrated INTEGER DEFAULT 0,
                domain_id TEXT,
                ethos_id TEXT,
                background TEXT
            );

            CREATE TABLE alignment_experts (
                dialogue_id TEXT NOT NULL,
                expert_slug TEXT NOT NULL,
                role TEXT NOT NULL,
                description TEXT,
                focus TEXT,
                tier TEXT NOT NULL,
                source TEXT NOT NULL,
                relevance REAL,
                creation_reason TEXT,
                color TEXT,
                scores TEXT DEFAULT '{}',
                raw_content TEXT,
                total_score INTEGER DEFAULT 0,
                first_round INTEGER,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, expert_slug)
            );

            CREATE TABLE alignment_rounds (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                title TEXT,
                score INTEGER NOT NULL,
                summary TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                PRIMARY KEY (dialogue_id, round)
            );

            CREATE TABLE alignment_perspectives (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                label TEXT NOT NULL,
                content TEXT NOT NULL,
                contributors TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                refs TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, seq)
            );

            CREATE TABLE alignment_perspective_events (
                dialogue_id TEXT NOT NULL,
                perspective_round INTEGER NOT NULL,
                perspective_seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_round INTEGER NOT NULL,
                actors TEXT NOT NULL,
                result_id TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, perspective_round, perspective_seq, created_at)
            );

            CREATE TABLE alignment_tensions (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                label TEXT NOT NULL,
                description TEXT NOT NULL,
                contributors TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'open',
                refs TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, seq)
            );

            CREATE TABLE alignment_tension_events (
                dialogue_id TEXT NOT NULL,
                tension_round INTEGER NOT NULL,
                tension_seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_round INTEGER NOT NULL,
                actors TEXT NOT NULL,
                reason TEXT,
                reference TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, tension_round, tension_seq, created_at)
            );

            CREATE TABLE alignment_recommendations (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                label TEXT NOT NULL,
                content TEXT NOT NULL,
                contributors TEXT NOT NULL,
                parameters TEXT,
                status TEXT NOT NULL DEFAULT 'proposed',
                refs TEXT,
                adopted_in_verdict TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, seq)
            );

            CREATE TABLE alignment_recommendation_events (
                dialogue_id TEXT NOT NULL,
                rec_round INTEGER NOT NULL,
                rec_seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_round INTEGER NOT NULL,
                actors TEXT NOT NULL,
                result_id TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, rec_round, rec_seq, created_at)
            );

            CREATE TABLE alignment_evidence (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                label TEXT NOT NULL,
                content TEXT NOT NULL,
                contributors TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'cited',
                refs TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, seq)
            );

            CREATE TABLE alignment_claims (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                label TEXT NOT NULL,
                content TEXT NOT NULL,
                contributors TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'asserted',
                refs TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, seq)
            );

            CREATE TABLE alignment_refs (
                dialogue_id TEXT NOT NULL,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                ref_type TEXT NOT NULL,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, source_id, ref_type, target_id)
            );

            CREATE TABLE alignment_verdicts (
                dialogue_id TEXT NOT NULL,
                verdict_id TEXT NOT NULL,
                verdict_type TEXT NOT NULL,
                round INTEGER NOT NULL,
                author_expert TEXT,
                recommendation TEXT NOT NULL,
                description TEXT NOT NULL,
                conditions TEXT,
                vote TEXT,
                confidence TEXT,
                tensions_resolved TEXT,
                tensions_accepted TEXT,
                recommendations_adopted TEXT,
                key_evidence TEXT,
                key_claims TEXT,
                supporting_experts TEXT,
                ethos_compliance TEXT,
                created_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, verdict_id)
            );
            "#,
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_generate_dialogue_id() {
        let conn = setup_test_db();

        let id1 = generate_dialogue_id(&conn, "Test Dialogue").unwrap();
        assert_eq!(id1, "test-dialogue");

        // Create the first dialogue
        create_dialogue(&conn, "Test Dialogue", None, None, None).unwrap();

        // Second dialogue with same title should get suffix
        let id2 = generate_dialogue_id(&conn, "Test Dialogue").unwrap();
        assert_eq!(id2, "test-dialogue-2");
    }

    #[test]
    fn test_display_id_format() {
        assert_eq!(display_id(EntityType::Perspective, 0, 1), "P0001");
        assert_eq!(display_id(EntityType::Tension, 1, 5), "T0105");
        assert_eq!(display_id(EntityType::Recommendation, 2, 15), "R0215");
        assert_eq!(display_id(EntityType::Evidence, 0, 99), "E0099");
        assert_eq!(display_id(EntityType::Claim, 3, 1), "C0301");
    }

    #[test]
    fn test_parse_display_id() {
        let (entity, round, seq) = parse_display_id("P0001").unwrap();
        assert_eq!(entity, EntityType::Perspective);
        assert_eq!(round, 0);
        assert_eq!(seq, 1);

        let (entity, round, seq) = parse_display_id("T0105").unwrap();
        assert_eq!(entity, EntityType::Tension);
        assert_eq!(round, 1);
        assert_eq!(seq, 5);

        let (entity, round, seq) = parse_display_id("R0215").unwrap();
        assert_eq!(entity, EntityType::Recommendation);
        assert_eq!(round, 2);
        assert_eq!(seq, 15);

        // Invalid IDs
        assert!(parse_display_id("X0001").is_none()); // Invalid type
        assert!(parse_display_id("P001").is_none()); // Too short
        assert!(parse_display_id("P00001").is_none()); // Too long
    }

    #[test]
    fn test_create_and_get_dialogue() {
        let conn = setup_test_db();

        let id = create_dialogue(
            &conn,
            "NVIDIA Investment Analysis",
            Some("Should Acme Trust swap NVAI for NVDA?"),
            Some("/tmp/blue-dialogue/nvidia"),
            None,
        )
        .unwrap();

        assert_eq!(id, "nvidia-investment-analysis");

        let dialogue = get_dialogue(&conn, &id).unwrap();
        assert_eq!(dialogue.title, "NVIDIA Investment Analysis");
        assert_eq!(
            dialogue.question,
            Some("Should Acme Trust swap NVAI for NVDA?".to_string())
        );
        assert_eq!(dialogue.status, DialogueStatus::Open);
        assert_eq!(dialogue.total_rounds, 0);
        assert_eq!(dialogue.total_alignment, 0);
    }

    #[test]
    fn test_register_expert() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        register_expert(
            &conn,
            "test",
            "muffin",
            "Value Analyst",
            ExpertTier::Core,
            ExpertSource::Pool,
            Some("Evaluates intrinsic value"),
            Some("Margin of safety"),
            Some(0.95),
            None,
            Some("#ff6b6b"),
            Some(0),
        )
        .unwrap();

        let experts = get_experts(&conn, "test").unwrap();
        assert_eq!(experts.len(), 1);
        assert_eq!(experts[0].expert_slug, "muffin");
        assert_eq!(experts[0].role, "Value Analyst");
        assert_eq!(experts[0].tier, ExpertTier::Core);
        assert_eq!(experts[0].source, ExpertSource::Pool);
    }

    #[test]
    fn test_register_perspective() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        let id = register_perspective(
            &conn,
            "test",
            0,
            "Income mandate mismatch",
            "NVIDIA's zero dividend conflicts with the trust's 4% income requirement.",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        assert_eq!(id, "P0001");

        // Register another perspective
        let id2 = register_perspective(
            &conn,
            "test",
            0,
            "Concentration risk",
            "Adding NVDA increases semiconductor exposure.",
            &["cupcake".to_string()],
            None,
        )
        .unwrap();

        assert_eq!(id2, "P0002");

        let perspectives = get_perspectives(&conn, "test").unwrap();
        assert_eq!(perspectives.len(), 2);
        assert_eq!(perspectives[0].label, "Income mandate mismatch");
        assert_eq!(perspectives[1].label, "Concentration risk");
    }

    #[test]
    fn test_register_tension() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        let id = register_tension(
            &conn,
            "test",
            0,
            "Growth vs income",
            "NVIDIA's zero dividend conflicts with 4% income mandate",
            &["muffin".to_string(), "cupcake".to_string()],
            None,
        )
        .unwrap();

        assert_eq!(id, "T0001");

        let tensions = get_tensions(&conn, "test").unwrap();
        assert_eq!(tensions.len(), 1);
        assert_eq!(tensions[0].status, TensionStatus::Open);
    }

    #[test]
    fn test_tension_lifecycle() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        register_tension(
            &conn,
            "test",
            0,
            "Growth vs income",
            "Income mandate conflict",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        // Address the tension
        update_tension_status(
            &conn,
            "test",
            "T0001",
            TensionStatus::Addressed,
            &["donut".to_string()],
            Some("R0001"),
            1,
        )
        .unwrap();

        let tensions = get_tensions(&conn, "test").unwrap();
        assert_eq!(tensions[0].status, TensionStatus::Addressed);

        // Resolve the tension
        update_tension_status(
            &conn,
            "test",
            "T0001",
            TensionStatus::Resolved,
            &["muffin".to_string()],
            Some("P0101"),
            2,
        )
        .unwrap();

        let tensions = get_tensions(&conn, "test").unwrap();
        assert_eq!(tensions[0].status, TensionStatus::Resolved);
    }

    #[test]
    fn test_expert_scores() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        register_expert(
            &conn,
            "test",
            "muffin",
            "Value Analyst",
            ExpertTier::Core,
            ExpertSource::Pool,
            None,
            None,
            None,
            None,
            None,
            Some(0),
        )
        .unwrap();

        // Update score for round 0
        update_expert_score(&conn, "test", "muffin", 0, 12).unwrap();

        let experts = get_experts(&conn, "test").unwrap();
        assert_eq!(experts[0].total_score, 12);
        assert_eq!(experts[0].scores["0"], 12);

        // Update score for round 1
        update_expert_score(&conn, "test", "muffin", 1, 8).unwrap();

        let experts = get_experts(&conn, "test").unwrap();
        assert_eq!(experts[0].total_score, 20);
        assert_eq!(experts[0].scores["1"], 8);
    }

    #[test]
    fn test_cross_references() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        register_ref(
            &conn,
            "test",
            EntityType::Perspective,
            "P0101",
            RefType::Refine,
            EntityType::Perspective,
            "P0001",
        )
        .unwrap();

        register_ref(
            &conn,
            "test",
            EntityType::Recommendation,
            "R0001",
            RefType::Address,
            EntityType::Tension,
            "T0001",
        )
        .unwrap();

        // Verify refs are stored (we'd need a get_refs function to fully test)
        // For now, just verify no errors
    }

    #[test]
    fn test_verdict_registration() {
        let conn = setup_test_db();
        create_dialogue(&conn, "Test", None, None, None).unwrap();

        let verdict = Verdict {
            dialogue_id: "test".to_string(),
            verdict_id: "final".to_string(),
            verdict_type: VerdictType::Final,
            round: 3,
            author_expert: None,
            recommendation: "APPROVE conditional partial trim".to_string(),
            description: "The panel approved the strategy with conditions.".to_string(),
            conditions: Some(vec![
                "Execute 60-90 days post-refinancing".to_string(),
                "Implement 30-delta covered calls".to_string(),
            ]),
            vote: Some("12-0".to_string()),
            confidence: Some("unanimous".to_string()),
            tensions_resolved: Some(vec!["T0001".to_string(), "T0002".to_string()]),
            tensions_accepted: None,
            recommendations_adopted: Some(vec!["R0001".to_string()]),
            key_evidence: None,
            key_claims: None,
            supporting_experts: None,
            ethos_compliance: None,
            created_at: Utc::now(),
        };

        register_verdict(&conn, &verdict).unwrap();

        // Verify dialogue status updated to converged
        let dialogue = get_dialogue(&conn, "test").unwrap();
        assert_eq!(dialogue.status, DialogueStatus::Converged);

        let verdicts = get_verdicts(&conn, "test").unwrap();
        assert_eq!(verdicts.len(), 1);
        assert_eq!(verdicts[0].verdict_type, VerdictType::Final);
        assert_eq!(verdicts[0].vote, Some("12-0".to_string()));
    }

    #[test]
    fn test_full_dialogue_workflow() {
        let conn = setup_test_db();

        // Create dialogue
        let id = create_dialogue(
            &conn,
            "NVIDIA Investment",
            Some("Should we swap NVAI for NVDA?"),
            None,
            None,
        )
        .unwrap();

        // Register experts
        register_expert(
            &conn,
            &id,
            "muffin",
            "Value Analyst",
            ExpertTier::Core,
            ExpertSource::Pool,
            None,
            None,
            Some(0.95),
            None,
            None,
            Some(0),
        )
        .unwrap();

        register_expert(
            &conn,
            &id,
            "donut",
            "Options Strategist",
            ExpertTier::Adjacent,
            ExpertSource::Pool,
            None,
            None,
            Some(0.70),
            None,
            None,
            Some(0),
        )
        .unwrap();

        // Create round 0
        create_round(&conn, &id, 0, Some("Opening Arguments"), 0).unwrap();

        // Register perspectives
        let p1 = register_perspective(
            &conn,
            &id,
            0,
            "Income mandate mismatch",
            "Zero dividend conflicts with 4% requirement",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p1, "P0001");

        let p2 = register_perspective(
            &conn,
            &id,
            0,
            "Options overlay opportunity",
            "Covered calls can generate income",
            &["donut".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p2, "P0002");

        // Register tension
        let t1 = register_tension(
            &conn,
            &id,
            0,
            "Growth vs income",
            "Fundamental conflict",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(t1, "T0001");

        // Register recommendation
        let r1 = register_recommendation(
            &conn,
            &id,
            0,
            "Income Collar Structure",
            "Use 30-delta covered calls",
            &["donut".to_string()],
            Some(&serde_json::json!({"delta": "0.30", "dte": "45"})),
            None,
        )
        .unwrap();
        assert_eq!(r1, "R0001");

        // Update scores
        update_expert_score(&conn, &id, "muffin", 0, 12).unwrap();
        update_expert_score(&conn, &id, "donut", 0, 15).unwrap();

        // Verify state
        let dialogue = get_dialogue(&conn, &id).unwrap();
        assert_eq!(dialogue.total_rounds, 1);

        let perspectives = get_perspectives(&conn, &id).unwrap();
        assert_eq!(perspectives.len(), 2);

        let tensions = get_tensions(&conn, &id).unwrap();
        assert_eq!(tensions.len(), 1);

        let recommendations = get_recommendations(&conn, &id).unwrap();
        assert_eq!(recommendations.len(), 1);
        assert!(recommendations[0].parameters.is_some());

        let experts = get_experts(&conn, &id).unwrap();
        let total_score: i32 = experts.iter().map(|e| e.total_score).sum();
        assert_eq!(total_score, 27);
    }
}
