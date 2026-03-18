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

    #[error("Batch validation failed with {} error(s)", .0.len())]
    BatchValidation(Vec<ValidationError>),
}

// ==================== Validation Types (RFC 0051 Phase 2c) ====================

/// Error codes for validation failures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationErrorCode {
    /// Missing required field
    MissingField,
    /// Invalid entity type (not P, R, T, E, or C)
    InvalidEntityType,
    /// Invalid reference type (not support, oppose, etc.)
    InvalidRefType,
    /// Entity type doesn't match ID prefix (e.g., source_type='P' but id='T0001')
    TypeIdMismatch,
    /// Invalid reference target (e.g., resolve targeting a Perspective instead of Tension)
    InvalidRefTarget,
    /// Invalid display ID format (should be like P0101, T0203)
    InvalidDisplayId,
    /// Invalid tension status transition
    InvalidStatusTransition,
    /// Duplicate entity (already exists)
    DuplicateEntity,
    /// Referenced entity not found
    EntityNotFound,
    /// Invalid round number
    InvalidRound,
    /// Expert not registered in dialogue
    ExpertNotInDialogue,
}

impl ValidationErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MissingField => "missing_field",
            Self::InvalidEntityType => "invalid_entity_type",
            Self::InvalidRefType => "invalid_ref_type",
            Self::TypeIdMismatch => "type_id_mismatch",
            Self::InvalidRefTarget => "invalid_ref_target",
            Self::InvalidDisplayId => "invalid_display_id",
            Self::InvalidStatusTransition => "invalid_status_transition",
            Self::DuplicateEntity => "duplicate_entity",
            Self::EntityNotFound => "entity_not_found",
            Self::InvalidRound => "invalid_round",
            Self::ExpertNotInDialogue => "expert_not_in_dialogue",
        }
    }
}

/// A single validation error with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error code for programmatic handling
    pub code: ValidationErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Field or path where error occurred (e.g., "perspectives[0].label")
    pub field: Option<String>,
    /// Suggestion for fixing the error
    pub suggestion: Option<String>,
    /// Additional context (e.g., valid options)
    pub context: Option<serde_json::Value>,
}

impl ValidationError {
    pub fn new(code: ValidationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            field: None,
            suggestion: None,
            context: None,
        }
    }

    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn with_context(mut self, context: serde_json::Value) -> Self {
        self.context = Some(context);
        self
    }

    /// Create a missing field error
    pub fn missing_field(field: &str) -> Self {
        Self::new(
            ValidationErrorCode::MissingField,
            format!("Required field '{}' is missing", field),
        )
        .with_field(field)
    }

    /// Create an invalid entity type error
    pub fn invalid_entity_type(value: &str) -> Self {
        Self::new(
            ValidationErrorCode::InvalidEntityType,
            format!(
                "Invalid entity type '{}'. Must be one of: P, R, T, E, C",
                value
            ),
        )
        .with_suggestion(
            "Use P (Perspective), R (Recommendation), T (Tension), E (Evidence), or C (Claim)",
        )
        .with_context(serde_json::json!({"valid_types": ["P", "R", "T", "E", "C"]}))
    }

    /// Create an invalid ref type error
    pub fn invalid_ref_type(value: &str) -> Self {
        Self::new(
            ValidationErrorCode::InvalidRefType,
            format!("Invalid reference type '{}'. Must be one of: support, oppose, refine, address, resolve, reopen, question, depend", value),
        )
        .with_suggestion("Use a valid reference type")
        .with_context(serde_json::json!({"valid_types": ["support", "oppose", "refine", "address", "resolve", "reopen", "question", "depend"]}))
    }

    /// Create a type/ID mismatch error
    pub fn type_id_mismatch(expected_type: &str, actual_id: &str) -> Self {
        Self::new(
            ValidationErrorCode::TypeIdMismatch,
            format!(
                "Entity type '{}' doesn't match ID '{}'. ID should start with '{}'",
                expected_type, actual_id, expected_type
            ),
        )
        .with_suggestion(format!(
            "Use an ID starting with '{}' (e.g., {}0101)",
            expected_type, expected_type
        ))
    }

    /// Create an invalid ref target error
    pub fn invalid_ref_target(ref_type: &str, target_type: &str, expected_type: &str) -> Self {
        Self::new(
            ValidationErrorCode::InvalidRefTarget,
            format!(
                "Reference type '{}' cannot target entity type '{}'. Expected: {}",
                ref_type, target_type, expected_type
            ),
        )
        .with_suggestion(format!("Use a {} entity as the target", expected_type))
        .with_context(serde_json::json!({"ref_type": ref_type, "expected_target": expected_type}))
    }

    /// Create an invalid display ID error
    pub fn invalid_display_id(id: &str) -> Self {
        Self::new(
            ValidationErrorCode::InvalidDisplayId,
            format!("Invalid display ID format '{}'. Expected format: [P|R|T|E|C]RRSS (e.g., P0101, T0203)", id),
        )
        .with_suggestion("Use format: type prefix + 2-digit round + 2-digit sequence (e.g., P0101)")
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code.as_str(), self.message)?;
        if let Some(field) = &self.field {
            write!(f, " (field: {})", field)?;
        }
        Ok(())
    }
}

/// Collector for batch validation errors
#[derive(Debug, Default)]
pub struct ValidationCollector {
    errors: Vec<ValidationError>,
}

impl ValidationCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn add(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    pub fn add_if<F>(&mut self, condition: bool, error_fn: F)
    where
        F: FnOnce() -> ValidationError,
    {
        if condition {
            self.errors.push(error_fn());
        }
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn into_result<T>(self, success: T) -> Result<T, AlignmentDbError> {
        if self.errors.is_empty() {
            Ok(success)
        } else {
            Err(AlignmentDbError::BatchValidation(self.errors))
        }
    }

    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    pub fn into_errors(self) -> Vec<ValidationError> {
        self.errors
    }
}

// ==================== Validation Functions ====================

/// Validate a reference's semantic constraints
pub fn validate_ref_semantics(
    ref_type: RefType,
    source_type: EntityType,
    target_type: EntityType,
) -> Option<ValidationError> {
    // resolve, reopen, address must target Tension (T)
    match ref_type {
        RefType::Resolve | RefType::Reopen | RefType::Address => {
            if target_type != EntityType::Tension {
                return Some(ValidationError::invalid_ref_target(
                    ref_type.as_str(),
                    target_type.as_str(),
                    "T (Tension)",
                ));
            }
        }
        // refine must be same-type
        RefType::Refine => {
            if source_type != target_type {
                return Some(ValidationError::new(
                    ValidationErrorCode::InvalidRefTarget,
                    format!(
                        "Reference type 'refine' requires same entity types. Source: {}, Target: {}",
                        source_type.as_str(),
                        target_type.as_str()
                    ),
                )
                .with_suggestion("Use matching entity types for refine references (P→P, R→R, etc.)"));
            }
        }
        // support, oppose, question, depend can target any type
        _ => {}
    }
    None
}

/// Validate a display ID format and extract components
pub fn validate_display_id(id: &str) -> Result<(EntityType, i32, i32), ValidationError> {
    parse_display_id(id).ok_or_else(|| ValidationError::invalid_display_id(id))
}

/// Validate that a display ID matches an expected entity type
pub fn validate_id_type_match(id: &str, expected_type: EntityType) -> Option<ValidationError> {
    if let Some((actual_type, _, _)) = parse_display_id(id) {
        if actual_type != expected_type {
            return Some(ValidationError::type_id_mismatch(
                expected_type.as_str(),
                id,
            ));
        }
    }
    None
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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
        params![
            dialogue_id,
            title,
            question,
            "open",
            now,
            output_dir,
            background_json,
        ],
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
#[allow(clippy::too_many_arguments)]
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

/// RFC 0057: Score components for ALIGNMENT breakdown
#[derive(Debug, Clone, Default)]
pub struct ScoreComponents {
    pub wisdom: i32,
    pub consistency: i32,
    pub truth: i32,
    pub relationships: i32,
}

impl ScoreComponents {
    pub fn total(&self) -> i32 {
        self.wisdom + self.consistency + self.truth + self.relationships
    }
}

/// RFC 0057: Convergence metrics for a round
#[derive(Debug, Clone, Default)]
pub struct ConvergenceMetrics {
    pub open_tensions: i32,
    pub new_perspectives: i32,
    pub converge_signals: i32,
    pub panel_size: i32,
}

impl ConvergenceMetrics {
    pub fn velocity(&self) -> i32 {
        self.open_tensions + self.new_perspectives
    }

    pub fn converge_percent(&self) -> f64 {
        if self.panel_size > 0 {
            (self.converge_signals as f64 * 100.0) / self.panel_size as f64
        } else {
            0.0
        }
    }

    pub fn can_converge(&self) -> bool {
        self.velocity() == 0 && self.converge_percent() >= 100.0
    }
}

/// Create a new round (legacy signature for backward compatibility)
pub fn create_round(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    title: Option<&str>,
    score: i32,
) -> Result<(), AlignmentDbError> {
    create_round_with_metrics(conn, dialogue_id, round, title, score, None, None)
}

/// RFC 0057: Create a new round with full metrics
pub fn create_round_with_metrics(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    title: Option<&str>,
    score: i32,
    score_components: Option<&ScoreComponents>,
    convergence: Option<&ConvergenceMetrics>,
) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    let sc = score_components.cloned().unwrap_or_default();
    let cm = convergence.cloned().unwrap_or_default();

    conn.execute(
        "INSERT INTO alignment_rounds
         (dialogue_id, round, title, score, score_wisdom, score_consistency, score_truth, score_relationships,
          open_tensions, new_perspectives, converge_signals, panel_size, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            dialogue_id, round, title, score,
            sc.wisdom, sc.consistency, sc.truth, sc.relationships,
            cm.open_tensions, cm.new_perspectives, cm.converge_signals, cm.panel_size,
            "open", now
        ],
    )?;

    // Update dialogue total rounds
    conn.execute(
        "UPDATE alignment_dialogues SET total_rounds = ?1, total_alignment = total_alignment + ?2
         WHERE dialogue_id = ?3",
        params![round + 1, score, dialogue_id],
    )?;

    Ok(())
}

/// RFC 0057: Record a convergence signal from an expert
pub fn record_convergence_signal(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
    expert_name: &str,
) -> Result<(), AlignmentDbError> {
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT OR REPLACE INTO alignment_convergence_signals
         (dialogue_id, round, expert_name, signaled_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![dialogue_id, round, expert_name, now],
    )?;

    Ok(())
}

/// RFC 0057: Get convergence signals for a round
pub fn get_convergence_signals(
    conn: &Connection,
    dialogue_id: &str,
    round: i32,
) -> Result<Vec<String>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT expert_name FROM alignment_convergence_signals
         WHERE dialogue_id = ?1 AND round = ?2",
    )?;

    let signals = stmt
        .query_map(params![dialogue_id, round], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    Ok(signals)
}

/// RFC 0057: Get scoreboard data for a dialogue
pub fn get_scoreboard(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<Vec<ScoreboardRow>, AlignmentDbError> {
    let mut stmt = conn.prepare(
        "SELECT round, W, C, T, R, total, open_tensions, new_perspectives, velocity,
                converge_signals, panel_size, converge_percent,
                cumulative_score, cumulative_W, cumulative_C, cumulative_T, cumulative_R
         FROM alignment_scoreboard WHERE dialogue_id = ?1 ORDER BY round",
    )?;

    let rows = stmt
        .query_map(params![dialogue_id], |row| {
            Ok(ScoreboardRow {
                round: row.get(0)?,
                w: row.get(1)?,
                c: row.get(2)?,
                t: row.get(3)?,
                r: row.get(4)?,
                total: row.get(5)?,
                open_tensions: row.get(6)?,
                new_perspectives: row.get(7)?,
                velocity: row.get(8)?,
                converge_signals: row.get(9)?,
                panel_size: row.get(10)?,
                converge_percent: row.get(11)?,
                cumulative_score: row.get(12)?,
                cumulative_w: row.get(13)?,
                cumulative_c: row.get(14)?,
                cumulative_t: row.get(15)?,
                cumulative_r: row.get(16)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

/// RFC 0057: Scoreboard row data
#[derive(Debug, Clone)]
pub struct ScoreboardRow {
    pub round: i32,
    pub w: i32,
    pub c: i32,
    pub t: i32,
    pub r: i32,
    pub total: i32,
    pub open_tensions: i32,
    pub new_perspectives: i32,
    pub velocity: i32,
    pub converge_signals: i32,
    pub panel_size: i32,
    pub converge_percent: f64,
    pub cumulative_score: i32,
    pub cumulative_w: i32,
    pub cumulative_c: i32,
    pub cumulative_t: i32,
    pub cumulative_r: i32,
}

/// RFC 0057: Check if dialogue can converge
pub fn can_dialogue_converge(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<(bool, Vec<String>), AlignmentDbError> {
    let scoreboard = get_scoreboard(conn, dialogue_id)?;

    if let Some(last_round) = scoreboard.last() {
        let mut blockers = Vec::new();

        if last_round.velocity > 0 {
            blockers.push(format!(
                "velocity={} (open_tensions={}, new_perspectives={})",
                last_round.velocity, last_round.open_tensions, last_round.new_perspectives
            ));
        }

        if last_round.converge_percent < 100.0 {
            blockers.push(format!(
                "converge_percent={:.0}% ({}/{})",
                last_round.converge_percent, last_round.converge_signals, last_round.panel_size
            ));
        }

        Ok((blockers.is_empty(), blockers))
    } else {
        Ok((false, vec!["no rounds registered".to_string()]))
    }
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
        params![
            dialogue_id,
            round,
            seq,
            "created",
            round,
            contributors_json,
            now
        ],
    )?;

    Ok(display)
}

/// Register a recommendation
#[allow(clippy::too_many_arguments)]
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
        params![
            dialogue_id,
            round,
            seq,
            "created",
            round,
            contributors_json,
            now
        ],
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

// ==================== Phase 6: Tooling & Analysis ====================

/// Expanded citation with full entity context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedCitation {
    /// Display ID (e.g., "P0001", "T0102")
    pub display_id: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Round where entity was created
    pub round: i32,
    /// Sequence within round
    pub seq: i32,
    /// Label/title of the entity
    pub label: String,
    /// Content snippet (first 200 chars)
    pub content_snippet: String,
    /// Full content
    pub content: String,
    /// Contributors who created/contributed
    pub contributors: Vec<String>,
    /// Status (for tensions)
    pub status: Option<String>,
    /// Parameters (for recommendations)
    pub parameters: Option<serde_json::Value>,
}

/// Expand a display ID into full entity context
pub fn expand_citation(
    conn: &Connection,
    dialogue_id: &str,
    display_id: &str,
) -> Result<ExpandedCitation, rusqlite::Error> {
    // Parse the display ID to get type, round, seq
    let (entity_type, round, seq) =
        parse_display_id(display_id).ok_or(rusqlite::Error::InvalidQuery)?;

    match entity_type {
        EntityType::Perspective => {
            let row: (String, String, String) = conn.query_row(
                "SELECT label, content, contributors FROM alignment_perspectives WHERE dialogue_id = ?1 AND round = ?2 AND seq = ?3",
                params![dialogue_id, round, seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            )?;
            let contributors: Vec<String> = serde_json::from_str(&row.2).unwrap_or_default();
            let content_snippet = if row.1.len() > 200 {
                format!("{}...", &row.1[..200])
            } else {
                row.1.clone()
            };
            Ok(ExpandedCitation {
                display_id: display_id.to_string(),
                entity_type,
                round,
                seq,
                label: row.0,
                content_snippet,
                content: row.1,
                contributors,
                status: None,
                parameters: None,
            })
        }
        EntityType::Recommendation => {
            let row: (String, String, String, Option<String>) = conn.query_row(
                "SELECT label, content, contributors, parameters FROM alignment_recommendations WHERE dialogue_id = ?1 AND round = ?2 AND seq = ?3",
                params![dialogue_id, round, seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            )?;
            let contributors: Vec<String> = serde_json::from_str(&row.2).unwrap_or_default();
            let parameters: Option<serde_json::Value> =
                row.3.and_then(|p| serde_json::from_str(&p).ok());
            let content_snippet = if row.1.len() > 200 {
                format!("{}...", &row.1[..200])
            } else {
                row.1.clone()
            };
            Ok(ExpandedCitation {
                display_id: display_id.to_string(),
                entity_type,
                round,
                seq,
                label: row.0,
                content_snippet,
                content: row.1,
                contributors,
                status: None,
                parameters,
            })
        }
        EntityType::Tension => {
            let row: (String, String, String, String) = conn.query_row(
                "SELECT label, description, contributors, status FROM alignment_tensions WHERE dialogue_id = ?1 AND round = ?2 AND seq = ?3",
                params![dialogue_id, round, seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            )?;
            let contributors: Vec<String> = serde_json::from_str(&row.2).unwrap_or_default();
            let content_snippet = if row.1.len() > 200 {
                format!("{}...", &row.1[..200])
            } else {
                row.1.clone()
            };
            Ok(ExpandedCitation {
                display_id: display_id.to_string(),
                entity_type,
                round,
                seq,
                label: row.0,
                content_snippet,
                content: row.1,
                contributors,
                status: Some(row.3),
                parameters: None,
            })
        }
        EntityType::Evidence => {
            let row: (String, String, String) = conn.query_row(
                "SELECT label, content, contributors FROM alignment_evidence WHERE dialogue_id = ?1 AND round = ?2 AND seq = ?3",
                params![dialogue_id, round, seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            )?;
            let contributors: Vec<String> = serde_json::from_str(&row.2).unwrap_or_default();
            let content_snippet = if row.1.len() > 200 {
                format!("{}...", &row.1[..200])
            } else {
                row.1.clone()
            };
            Ok(ExpandedCitation {
                display_id: display_id.to_string(),
                entity_type,
                round,
                seq,
                label: row.0,
                content_snippet,
                content: row.1,
                contributors,
                status: None,
                parameters: None,
            })
        }
        EntityType::Claim => {
            let row: (String, String, String) = conn.query_row(
                "SELECT label, content, contributors FROM alignment_claims WHERE dialogue_id = ?1 AND round = ?2 AND seq = ?3",
                params![dialogue_id, round, seq],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            )?;
            let contributors: Vec<String> = serde_json::from_str(&row.2).unwrap_or_default();
            let content_snippet = if row.1.len() > 200 {
                format!("{}...", &row.1[..200])
            } else {
                row.1.clone()
            };
            Ok(ExpandedCitation {
                display_id: display_id.to_string(),
                entity_type,
                round,
                seq,
                label: row.0,
                content_snippet,
                content: row.1,
                contributors,
                status: None,
                parameters: None,
            })
        }
    }
}

/// Expand multiple citations at once
pub fn expand_citations(
    conn: &Connection,
    dialogue_id: &str,
    display_ids: &[String],
) -> Vec<Result<ExpandedCitation, String>> {
    display_ids
        .iter()
        .map(|id| expand_citation(conn, dialogue_id, id).map_err(|e| format!("{}: {}", id, e)))
        .collect()
}

/// Cross-dialogue statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDialogueStats {
    /// Total number of dialogues
    pub total_dialogues: i32,
    /// Dialogues by status
    pub by_status: std::collections::HashMap<String, i32>,
    /// Total perspectives across all dialogues
    pub total_perspectives: i32,
    /// Total tensions across all dialogues
    pub total_tensions: i32,
    /// Open tensions count
    pub open_tensions: i32,
    /// Resolved tensions count
    pub resolved_tensions: i32,
    /// Total recommendations
    pub total_recommendations: i32,
    /// Total evidence items
    pub total_evidence: i32,
    /// Total claims
    pub total_claims: i32,
    /// Average alignment score per dialogue
    pub avg_alignment: f64,
    /// Total unique experts
    pub total_experts: i32,
    /// Most active experts (by total score)
    pub top_experts: Vec<(String, i32)>,
}

/// Get aggregated statistics across all dialogues
pub fn get_cross_dialogue_stats(conn: &Connection) -> Result<CrossDialogueStats, rusqlite::Error> {
    let total_dialogues: i32 =
        conn.query_row("SELECT COUNT(*) FROM alignment_dialogues", [], |row| {
            row.get(0)
        })?;

    // By status
    let mut by_status = std::collections::HashMap::new();
    let mut stmt =
        conn.prepare("SELECT status, COUNT(*) FROM alignment_dialogues GROUP BY status")?;
    let rows = stmt.query_map([], |row| {
        let status: String = row.get(0)?;
        let count: i32 = row.get(1)?;
        Ok((status, count))
    })?;
    for row in rows {
        let (status, count) = row?;
        by_status.insert(status, count);
    }

    // Entity counts
    let total_perspectives: i32 = conn
        .query_row("SELECT COUNT(*) FROM alignment_perspectives", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let total_tensions: i32 = conn
        .query_row("SELECT COUNT(*) FROM alignment_tensions", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let open_tensions: i32 = conn.query_row(
        "SELECT COUNT(*) FROM alignment_tensions WHERE status IN ('open', 'addressed', 'reopened')",
        [], |row| row.get(0)
    ).unwrap_or(0);

    let resolved_tensions: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM alignment_tensions WHERE status = 'resolved'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_recommendations: i32 = conn
        .query_row(
            "SELECT COUNT(*) FROM alignment_recommendations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_evidence: i32 = conn
        .query_row("SELECT COUNT(*) FROM alignment_evidence", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    let total_claims: i32 = conn
        .query_row("SELECT COUNT(*) FROM alignment_claims", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);

    // Average alignment
    let avg_alignment: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(total_alignment), 0.0) FROM alignment_dialogues",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    // Expert stats
    let total_experts: i32 = conn
        .query_row(
            "SELECT COUNT(DISTINCT expert_slug) FROM alignment_experts",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    // Top experts by total score
    let mut top_experts = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT expert_slug, SUM(total_score) as total
         FROM alignment_experts
         GROUP BY expert_slug
         ORDER BY total DESC
         LIMIT 10",
    )?;
    let rows = stmt.query_map([], |row| {
        let slug: String = row.get(0)?;
        let score: i32 = row.get(1)?;
        Ok((slug, score))
    })?;
    for row in rows {
        top_experts.push(row?);
    }

    Ok(CrossDialogueStats {
        total_dialogues,
        by_status,
        total_perspectives,
        total_tensions,
        open_tensions,
        resolved_tensions,
        total_recommendations,
        total_evidence,
        total_claims,
        avg_alignment,
        total_experts,
        top_experts,
    })
}

/// Real-time dialogue progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueProgress {
    /// Dialogue ID
    pub dialogue_id: String,
    /// Current status
    pub status: String,
    /// Current round number
    pub current_round: i32,
    /// Total rounds completed
    pub total_rounds: i32,
    /// Running ALIGNMENT score
    pub total_alignment: i32,
    /// Per-round scores
    pub round_scores: Vec<i32>,
    /// ALIGNMENT velocity (change from last round)
    pub velocity: i32,
    /// Open tensions count
    pub open_tensions: i32,
    /// Resolved tensions count
    pub resolved_tensions: i32,
    /// Active experts in current round
    pub active_experts: Vec<String>,
    /// Expert leaderboard (slug, total_score)
    pub leaderboard: Vec<(String, i32)>,
    /// Convergence indicator (true if velocity near zero for 2+ rounds)
    pub converging: bool,
    /// Estimated completion (based on velocity trend)
    pub est_rounds_remaining: Option<i32>,
}

/// Get real-time progress for a dialogue
pub fn get_dialogue_progress(
    conn: &Connection,
    dialogue_id: &str,
) -> Result<DialogueProgress, AlignmentDbError> {
    // Get dialogue info
    let dialogue = get_dialogue(conn, dialogue_id)?;

    // Get round scores
    let mut round_scores = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT round, score FROM alignment_rounds WHERE dialogue_id = ?1 ORDER BY round",
    )?;
    let rows = stmt.query_map(params![dialogue_id], |row| {
        let _round: i32 = row.get(0)?;
        let score: i32 = row.get(1)?;
        Ok(score)
    })?;
    for row in rows {
        round_scores.push(row?);
    }

    // Calculate velocity
    let velocity = if round_scores.len() >= 2 {
        round_scores[round_scores.len() - 1] - round_scores[round_scores.len() - 2]
    } else {
        0
    };

    // Check convergence (velocity near zero for 2+ rounds)
    let converging = if round_scores.len() >= 3 {
        let v1 = round_scores[round_scores.len() - 1] - round_scores[round_scores.len() - 2];
        let v2 = round_scores[round_scores.len() - 2] - round_scores[round_scores.len() - 3];
        v1.abs() < 5 && v2.abs() < 5
    } else {
        false
    };

    // Tension counts
    let open_tensions: i32 = conn.query_row(
        "SELECT COUNT(*) FROM alignment_tensions WHERE dialogue_id = ?1 AND status IN ('open', 'addressed', 'reopened')",
        params![dialogue_id], |row| row.get(0)
    ).unwrap_or(0);

    let resolved_tensions: i32 = conn.query_row(
        "SELECT COUNT(*) FROM alignment_tensions WHERE dialogue_id = ?1 AND status = 'resolved'",
        params![dialogue_id], |row| row.get(0)
    ).unwrap_or(0);

    // Get current round experts
    let current_round = dialogue.total_rounds.saturating_sub(1).max(0);
    let mut active_experts = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT expert_slug FROM alignment_experts
         WHERE dialogue_id = ?1 AND first_round <= ?2
         ORDER BY total_score DESC",
    )?;
    let rows = stmt.query_map(params![dialogue_id, current_round], |row| row.get(0))?;
    for row in rows {
        active_experts.push(row?);
    }

    // Leaderboard
    let mut leaderboard = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT expert_slug, total_score FROM alignment_experts
         WHERE dialogue_id = ?1
         ORDER BY total_score DESC",
    )?;
    let rows = stmt.query_map(params![dialogue_id], |row| {
        let slug: String = row.get(0)?;
        let score: i32 = row.get(1)?;
        Ok((slug, score))
    })?;
    for row in rows {
        leaderboard.push(row?);
    }

    // Estimate rounds remaining based on open tensions and velocity
    let est_rounds_remaining = if converging || open_tensions == 0 {
        Some(1)
    } else if velocity > 0 {
        Some((open_tensions / 2).max(1)) // Rough estimate: ~2 tensions resolved per round
    } else {
        None // Can't estimate if not making progress
    };

    Ok(DialogueProgress {
        dialogue_id: dialogue_id.to_string(),
        status: match dialogue.status {
            DialogueStatus::Open => "open".to_string(),
            DialogueStatus::Converging => "converging".to_string(),
            DialogueStatus::Converged => "converged".to_string(),
            DialogueStatus::Abandoned => "abandoned".to_string(),
        },
        current_round,
        total_rounds: dialogue.total_rounds,
        total_alignment: dialogue.total_alignment,
        round_scores,
        velocity,
        open_tensions,
        resolved_tensions,
        active_experts,
        leaderboard,
        converging,
        est_rounds_remaining,
    })
}

/// Find dialogues with similar tensions or perspectives (basic text similarity)
pub fn find_similar_dialogues(
    conn: &Connection,
    query: &str,
    limit: i32,
) -> Result<Vec<(String, String, i32)>, rusqlite::Error> {
    // Simple LIKE-based search across dialogue titles and tension labels
    let pattern = format!("%{}%", query.to_lowercase());

    let mut results = Vec::new();

    // Search dialogue titles and questions
    let mut stmt = conn.prepare(
        "SELECT dialogue_id, title, total_alignment FROM alignment_dialogues
         WHERE LOWER(title) LIKE ?1 OR LOWER(question) LIKE ?1
         ORDER BY total_alignment DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pattern, limit], |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        Ok((id, title, score))
    })?;
    for row in rows {
        results.push(row?);
    }

    // Also search tension labels and descriptions
    let mut stmt = conn.prepare(
        "SELECT DISTINCT d.dialogue_id, d.title, d.total_alignment
         FROM alignment_dialogues d
         JOIN alignment_tensions t ON d.dialogue_id = t.dialogue_id
         WHERE LOWER(t.label) LIKE ?1 OR LOWER(t.description) LIKE ?1
         ORDER BY d.total_alignment DESC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![pattern, limit], |row| {
        let id: String = row.get(0)?;
        let title: String = row.get(1)?;
        let score: i32 = row.get(2)?;
        Ok((id, title, score))
    })?;
    for row in rows {
        let result = row?;
        if !results.iter().any(|(id, _, _)| id == &result.0) {
            results.push(result);
        }
    }

    results.truncate(limit as usize);
    Ok(results)
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
                -- RFC 0057: ALIGNMENT score components
                score_wisdom INTEGER NOT NULL DEFAULT 0,
                score_consistency INTEGER NOT NULL DEFAULT 0,
                score_truth INTEGER NOT NULL DEFAULT 0,
                score_relationships INTEGER NOT NULL DEFAULT 0,
                -- RFC 0057: Velocity & convergence tracking
                open_tensions INTEGER NOT NULL DEFAULT 0,
                new_perspectives INTEGER NOT NULL DEFAULT 0,
                converge_signals INTEGER NOT NULL DEFAULT 0,
                panel_size INTEGER NOT NULL DEFAULT 0,
                summary TEXT,
                status TEXT NOT NULL DEFAULT 'open',
                created_at TEXT NOT NULL,
                completed_at TEXT,
                PRIMARY KEY (dialogue_id, round)
            );

            -- RFC 0057: Track per-expert convergence signals
            CREATE TABLE alignment_convergence_signals (
                dialogue_id TEXT NOT NULL,
                round INTEGER NOT NULL,
                expert_name TEXT NOT NULL,
                signaled_at TEXT NOT NULL,
                PRIMARY KEY (dialogue_id, round, expert_name)
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

            -- Performance indices for common lookups
            CREATE INDEX idx_experts_dialogue ON alignment_experts(dialogue_id);
            CREATE INDEX idx_rounds_dialogue ON alignment_rounds(dialogue_id);
            CREATE INDEX idx_perspectives_dialogue ON alignment_perspectives(dialogue_id);
            CREATE INDEX idx_tensions_dialogue ON alignment_tensions(dialogue_id);
            CREATE INDEX idx_tensions_status ON alignment_tensions(dialogue_id, status);
            CREATE INDEX idx_recommendations_dialogue ON alignment_recommendations(dialogue_id);
            CREATE INDEX idx_evidence_dialogue ON alignment_evidence(dialogue_id);
            CREATE INDEX idx_claims_dialogue ON alignment_claims(dialogue_id);
            CREATE INDEX idx_refs_dialogue ON alignment_refs(dialogue_id);
            CREATE INDEX idx_refs_target ON alignment_refs(dialogue_id, target_id);
            CREATE INDEX idx_verdicts_dialogue ON alignment_verdicts(dialogue_id);
            CREATE INDEX idx_convergence_signals_round ON alignment_convergence_signals(dialogue_id, round);

            -- RFC 0057: Scoreboard view for efficient convergence queries
            CREATE VIEW alignment_scoreboard AS
            SELECT
                r.dialogue_id,
                r.round,
                r.score_wisdom AS W,
                r.score_consistency AS C,
                r.score_truth AS T,
                r.score_relationships AS R,
                r.score AS total,
                r.open_tensions,
                r.new_perspectives,
                (r.open_tensions + r.new_perspectives) AS velocity,
                r.converge_signals,
                r.panel_size,
                CASE WHEN r.panel_size > 0 THEN (r.converge_signals * 100.0 / r.panel_size) ELSE 0 END AS converge_percent,
                (SELECT SUM(score) FROM alignment_rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_score,
                (SELECT SUM(score_wisdom) FROM alignment_rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_W,
                (SELECT SUM(score_consistency) FROM alignment_rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_C,
                (SELECT SUM(score_truth) FROM alignment_rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_T,
                (SELECT SUM(score_relationships) FROM alignment_rounds r2 WHERE r2.dialogue_id = r.dialogue_id AND r2.round <= r.round) AS cumulative_R
            FROM alignment_rounds r
            ORDER BY r.dialogue_id, r.round;
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

    // ==================== Validation Tests (RFC 0051 Phase 2c) ====================

    #[test]
    fn test_validation_error_creation() {
        let err = ValidationError::missing_field("label");
        assert_eq!(err.code, ValidationErrorCode::MissingField);
        assert!(err.message.contains("label"));
        assert_eq!(err.field, Some("label".to_string()));

        let err = ValidationError::invalid_entity_type("X");
        assert_eq!(err.code, ValidationErrorCode::InvalidEntityType);
        assert!(err.context.is_some());

        let err = ValidationError::invalid_ref_type("bogus");
        assert_eq!(err.code, ValidationErrorCode::InvalidRefType);
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_validation_collector() {
        let mut collector = ValidationCollector::new();
        assert!(collector.is_empty());

        collector.add(ValidationError::missing_field("label"));
        collector.add(ValidationError::missing_field("content"));

        assert!(!collector.is_empty());
        assert_eq!(collector.len(), 2);

        let errors = collector.into_errors();
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn test_validate_ref_semantics_resolve_must_target_tension() {
        // resolve must target Tension
        let err = validate_ref_semantics(
            RefType::Resolve,
            EntityType::Perspective,
            EntityType::Perspective,
        );
        assert!(err.is_some());
        assert_eq!(
            err.as_ref().unwrap().code,
            ValidationErrorCode::InvalidRefTarget
        );

        // resolve targeting Tension is valid
        let err = validate_ref_semantics(
            RefType::Resolve,
            EntityType::Perspective,
            EntityType::Tension,
        );
        assert!(err.is_none());

        // address must target Tension
        let err = validate_ref_semantics(
            RefType::Address,
            EntityType::Recommendation,
            EntityType::Claim,
        );
        assert!(err.is_some());

        // reopen must target Tension
        let err =
            validate_ref_semantics(RefType::Reopen, EntityType::Evidence, EntityType::Evidence);
        assert!(err.is_some());
    }

    #[test]
    fn test_validate_ref_semantics_refine_must_be_same_type() {
        // refine P→P is valid
        let err = validate_ref_semantics(
            RefType::Refine,
            EntityType::Perspective,
            EntityType::Perspective,
        );
        assert!(err.is_none());

        // refine R→R is valid
        let err = validate_ref_semantics(
            RefType::Refine,
            EntityType::Recommendation,
            EntityType::Recommendation,
        );
        assert!(err.is_none());

        // refine P→R is invalid (different types)
        let err = validate_ref_semantics(
            RefType::Refine,
            EntityType::Perspective,
            EntityType::Recommendation,
        );
        assert!(err.is_some());
        assert_eq!(
            err.as_ref().unwrap().code,
            ValidationErrorCode::InvalidRefTarget
        );
    }

    #[test]
    fn test_validate_ref_semantics_support_can_target_any() {
        // support can target any entity type
        assert!(validate_ref_semantics(
            RefType::Support,
            EntityType::Perspective,
            EntityType::Tension
        )
        .is_none());
        assert!(
            validate_ref_semantics(RefType::Support, EntityType::Evidence, EntityType::Claim)
                .is_none()
        );
        assert!(validate_ref_semantics(
            RefType::Support,
            EntityType::Claim,
            EntityType::Recommendation
        )
        .is_none());

        // oppose can target any
        assert!(validate_ref_semantics(
            RefType::Oppose,
            EntityType::Perspective,
            EntityType::Perspective
        )
        .is_none());

        // question can target any
        assert!(
            validate_ref_semantics(RefType::Question, EntityType::Evidence, EntityType::Claim)
                .is_none()
        );
    }

    #[test]
    fn test_validate_display_id() {
        // Valid IDs
        assert!(validate_display_id("P0101").is_ok());
        assert!(validate_display_id("T0001").is_ok());
        assert!(validate_display_id("R0203").is_ok());
        assert!(validate_display_id("E0105").is_ok());
        assert!(validate_display_id("C0001").is_ok());

        // Invalid IDs
        assert!(validate_display_id("X0101").is_err()); // Invalid prefix
        assert!(validate_display_id("P01").is_err()); // Too short
        assert!(validate_display_id("P010101").is_err()); // Too long
        assert!(validate_display_id("").is_err()); // Empty
        assert!(validate_display_id("PERSPECTIVE").is_err()); // Not a display ID
    }

    #[test]
    fn test_type_id_mismatch_error() {
        let err = ValidationError::type_id_mismatch("P", "T0101");
        assert_eq!(err.code, ValidationErrorCode::TypeIdMismatch);
        assert!(err.message.contains("P"));
        assert!(err.message.contains("T0101"));
    }

    #[test]
    fn test_invalid_ref_target_error() {
        let err = ValidationError::invalid_ref_target("resolve", "P", "T (Tension)");
        assert_eq!(err.code, ValidationErrorCode::InvalidRefTarget);
        assert!(err.message.contains("resolve"));
        assert!(err.message.contains("Tension"));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_integration_multi_round_dialogue() {
        let conn = setup_test_db();

        // === Round 0: Create dialogue and initial experts ===
        let dialogue_id = create_dialogue(
            &conn,
            "Portfolio Rebalancing Decision",
            Some("Should we rebalance the trust portfolio given current market conditions?"),
            Some("/tmp/blue-dialogue/portfolio-rebalancing"),
            Some(&DialogueBackground {
                subject: "Trust portfolio management".to_string(),
                description: Some("Evaluating position changes for a family trust".to_string()),
                constraints: Some(serde_json::json!({
                    "income_requirement": "4% annual",
                    "risk_tolerance": "moderate"
                })),
                situation: Some("Market volatility increasing".to_string()),
            }),
        )
        .unwrap();

        // Register panel for round 0
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Value Analyst",
            ExpertTier::Core,
            ExpertSource::Pool,
            Some("Analyzes intrinsic value"),
            Some("Margin of safety"),
            Some(0.95),
            None,
            None,
            Some(0),
        )
        .unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "donut",
            "Risk Manager",
            ExpertTier::Core,
            ExpertSource::Pool,
            Some("Manages downside risk"),
            Some("Tail events"),
            Some(0.90),
            None,
            None,
            Some(0),
        )
        .unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "cupcake",
            "Income Analyst",
            ExpertTier::Adjacent,
            ExpertSource::Pool,
            Some("Focuses on yield"),
            Some("Dividend sustainability"),
            Some(0.75),
            None,
            None,
            Some(0),
        )
        .unwrap();

        // Create round 0
        create_round(&conn, &dialogue_id, 0, Some("Opening arguments"), 35).unwrap();

        // Register perspectives from round 0
        let p1 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "Valuation stretched",
            "Current position is 40% above fair value estimates",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p1, "P0001");

        let p2 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "Income gap identified",
            "Zero dividend creates 4% shortfall vs trust mandate",
            &["cupcake".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p2, "P0002");

        // Register tension
        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Growth vs income conflict",
            "Position's growth profile incompatible with income mandate",
            &["cupcake".to_string(), "muffin".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(t1, "T0001");

        // Register recommendation
        let r1 = register_recommendation(
            &conn,
            &dialogue_id,
            0,
            "Options overlay",
            "Implement covered call strategy to generate income",
            &["donut".to_string()],
            Some(&serde_json::json!({"delta": "0.30", "dte": "30-45"})),
            None,
        )
        .unwrap();
        assert_eq!(r1, "R0001");

        // Register cross-references
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Recommendation,
            &r1,
            RefType::Address,
            EntityType::Tension,
            &t1,
        )
        .unwrap();
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Perspective,
            &p2,
            RefType::Support,
            EntityType::Tension,
            &t1,
        )
        .unwrap();

        // Update scores
        update_expert_score(&conn, &dialogue_id, "muffin", 0, 12).unwrap();
        update_expert_score(&conn, &dialogue_id, "donut", 0, 15).unwrap();
        update_expert_score(&conn, &dialogue_id, "cupcake", 0, 8).unwrap();

        // === Round 1: Continue with created expert ===

        // Judge creates new expert to address supply chain concern
        register_expert(
            &conn,
            &dialogue_id,
            "palmier",
            "Supply Chain Analyst",
            ExpertTier::Adjacent,
            ExpertSource::Created,
            Some("Analyzes supply chain risks"),
            Some("Geographic concentration"),
            None,
            Some("Emerging tension around manufacturing concentration"),
            None,
            Some(1),
        )
        .unwrap();

        create_round(&conn, &dialogue_id, 1, Some("Deepening analysis"), 28).unwrap();

        // New perspective from created expert
        let p3 = register_perspective(
            &conn,
            &dialogue_id,
            1,
            "Concentration risk",
            "Single-source manufacturing creates tail risk",
            &["palmier".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p3, "P0101");

        // Refinement of earlier perspective
        let p4 = register_perspective(
            &conn,
            &dialogue_id,
            1,
            "Valuation context",
            "Stretched valuation justified by growth trajectory if supply chain stable",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(p4, "P0102");
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Perspective,
            &p4,
            RefType::Refine,
            EntityType::Perspective,
            &p1,
        )
        .unwrap();

        // Evidence supporting the recommendation
        let e1 = register_evidence(
            &conn,
            &dialogue_id,
            1,
            "Historical premium data",
            "30-delta calls yielded 2.1-2.8% monthly over 24 months",
            &["donut".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(e1, "E0101");

        // Claim based on evidence
        let c1 = register_claim(
            &conn,
            &dialogue_id,
            1,
            "Income mandate achievable",
            "Options overlay can satisfy 4% requirement based on historical premiums",
            &["donut".to_string(), "cupcake".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(c1, "C0101");
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Claim,
            &c1,
            RefType::Depend,
            EntityType::Evidence,
            &e1,
        )
        .unwrap();
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Claim,
            &c1,
            RefType::Resolve,
            EntityType::Tension,
            &t1,
        )
        .unwrap();

        // Update tension status
        update_tension_status(
            &conn,
            &dialogue_id,
            &t1,
            TensionStatus::Addressed,
            &["donut".to_string()],
            Some(&c1),
            1,
        )
        .unwrap();

        update_expert_score(&conn, &dialogue_id, "muffin", 1, 10).unwrap();
        update_expert_score(&conn, &dialogue_id, "donut", 1, 18).unwrap();
        update_expert_score(&conn, &dialogue_id, "cupcake", 1, 5).unwrap();
        update_expert_score(&conn, &dialogue_id, "palmier", 1, 12).unwrap();

        // === Round 2: Convergence ===
        create_round(&conn, &dialogue_id, 2, Some("Convergence"), 15).unwrap();

        // Final tension resolution
        update_tension_status(
            &conn,
            &dialogue_id,
            &t1,
            TensionStatus::Resolved,
            &[
                "muffin".to_string(),
                "cupcake".to_string(),
                "donut".to_string(),
            ],
            Some(&r1),
            2,
        )
        .unwrap();

        // Register interim verdict
        let interim_verdict = Verdict {
            dialogue_id: dialogue_id.clone(),
            verdict_id: "interim-r2".to_string(),
            verdict_type: VerdictType::Interim,
            round: 2,
            author_expert: None,
            recommendation: "APPROVE with conditions".to_string(),
            description: "Panel converging on options overlay approach".to_string(),
            conditions: Some(vec!["Monitor supply chain quarterly".to_string()]),
            vote: Some("3-1".to_string()),
            confidence: Some("strong".to_string()),
            tensions_resolved: Some(vec![t1.clone()]),
            tensions_accepted: None,
            recommendations_adopted: Some(vec![r1.clone()]),
            key_evidence: Some(vec![e1.clone()]),
            key_claims: Some(vec![c1.clone()]),
            supporting_experts: None,
            ethos_compliance: None,
            created_at: chrono::Utc::now(),
        };
        register_verdict(&conn, &interim_verdict).unwrap();

        // Register final verdict
        let final_verdict = Verdict {
            dialogue_id: dialogue_id.clone(),
            verdict_id: "final".to_string(),
            verdict_type: VerdictType::Final,
            round: 2,
            author_expert: None,
            recommendation: "APPROVE: Implement 30-delta covered call overlay".to_string(),
            description: "Panel unanimously supports options strategy to satisfy income mandate while maintaining growth exposure".to_string(),
            conditions: Some(vec![
                "Roll calls at 21 DTE".to_string(),
                "Monitor supply chain concentration quarterly".to_string(),
            ]),
            vote: Some("4-0".to_string()),
            confidence: Some("unanimous".to_string()),
            tensions_resolved: Some(vec![t1.clone()]),
            tensions_accepted: None,
            recommendations_adopted: Some(vec![r1.clone()]),
            key_evidence: Some(vec![e1.clone()]),
            key_claims: Some(vec![c1.clone()]),
            supporting_experts: Some(vec!["muffin".to_string(), "donut".to_string(), "cupcake".to_string(), "palmier".to_string()]),
            ethos_compliance: None,
            created_at: chrono::Utc::now(),
        };
        register_verdict(&conn, &final_verdict).unwrap();

        // === Verify final state ===
        let dialogue = get_dialogue(&conn, &dialogue_id).unwrap();
        assert_eq!(dialogue.total_rounds, 3); // 0, 1, 2
        assert_eq!(dialogue.total_alignment, 35 + 28 + 15); // Sum of round scores

        let experts = get_experts(&conn, &dialogue_id).unwrap();
        assert_eq!(experts.len(), 4);

        // Find palmier and verify created source
        let palmier = experts.iter().find(|e| e.expert_slug == "palmier").unwrap();
        assert_eq!(palmier.source, ExpertSource::Created);
        assert_eq!(palmier.first_round, Some(1));
        assert!(palmier.creation_reason.is_some());

        let perspectives = get_perspectives(&conn, &dialogue_id).unwrap();
        assert_eq!(perspectives.len(), 4); // P0001, P0002, P0101, P0102

        let tensions = get_tensions(&conn, &dialogue_id).unwrap();
        assert_eq!(tensions.len(), 1);
        assert_eq!(tensions[0].status, TensionStatus::Resolved);

        let recommendations = get_recommendations(&conn, &dialogue_id).unwrap();
        assert_eq!(recommendations.len(), 1);

        let evidence = get_evidence(&conn, &dialogue_id).unwrap();
        assert_eq!(evidence.len(), 1);

        let claims = get_claims(&conn, &dialogue_id).unwrap();
        assert_eq!(claims.len(), 1);

        let verdicts = get_verdicts(&conn, &dialogue_id).unwrap();
        assert_eq!(verdicts.len(), 2);
        assert!(verdicts
            .iter()
            .any(|v| v.verdict_type == VerdictType::Interim));
        assert!(verdicts
            .iter()
            .any(|v| v.verdict_type == VerdictType::Final));

        // Verify total scores
        let muffin = experts.iter().find(|e| e.expert_slug == "muffin").unwrap();
        assert_eq!(muffin.total_score, 22); // 12 + 10
        let donut = experts.iter().find(|e| e.expert_slug == "donut").unwrap();
        assert_eq!(donut.total_score, 33); // 15 + 18
    }

    #[test]
    fn test_integration_minority_verdict() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(
            &conn,
            "Contested Decision",
            Some("A divisive topic"),
            None,
            None,
        )
        .unwrap();

        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Advocate",
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
        register_expert(
            &conn,
            &dialogue_id,
            "donut",
            "Skeptic",
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

        create_round(&conn, &dialogue_id, 0, None, 20).unwrap();

        // Register a tension that won't be resolved
        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Fundamental disagreement",
            "Core values conflict",
            &["muffin".to_string(), "donut".to_string()],
            None,
        )
        .unwrap();

        // Majority verdict
        let majority = Verdict {
            dialogue_id: dialogue_id.clone(),
            verdict_id: "final".to_string(),
            verdict_type: VerdictType::Final,
            round: 0,
            author_expert: None,
            recommendation: "APPROVE".to_string(),
            description: "Majority supports approval".to_string(),
            conditions: None,
            vote: Some("1-1".to_string()),
            confidence: Some("split".to_string()),
            tensions_resolved: None,
            tensions_accepted: Some(vec![t1.clone()]),
            recommendations_adopted: None,
            key_evidence: None,
            key_claims: None,
            supporting_experts: Some(vec!["muffin".to_string()]),
            ethos_compliance: None,
            created_at: chrono::Utc::now(),
        };
        register_verdict(&conn, &majority).unwrap();

        // Minority dissent
        let minority = Verdict {
            dialogue_id: dialogue_id.clone(),
            verdict_id: "dissent-donut".to_string(),
            verdict_type: VerdictType::Dissent,
            round: 0,
            author_expert: Some("donut".to_string()),
            recommendation: "REJECT".to_string(),
            description: "Cannot support without addressing fundamental tension".to_string(),
            conditions: Some(vec!["Require tension T0001 resolution".to_string()]),
            vote: None,
            confidence: None,
            tensions_resolved: None,
            tensions_accepted: None,
            recommendations_adopted: None,
            key_evidence: None,
            key_claims: None,
            supporting_experts: Some(vec!["donut".to_string()]),
            ethos_compliance: None,
            created_at: chrono::Utc::now(),
        };
        register_verdict(&conn, &minority).unwrap();

        let verdicts = get_verdicts(&conn, &dialogue_id).unwrap();
        assert_eq!(verdicts.len(), 2);

        let dissent = verdicts
            .iter()
            .find(|v| v.verdict_type == VerdictType::Dissent)
            .unwrap();
        assert_eq!(dissent.author_expert, Some("donut".to_string()));
        assert_eq!(dissent.recommendation, "REJECT");
    }

    #[test]
    fn test_integration_tension_reopening() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Evolving Discussion", None, None, None).unwrap();

        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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

        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        // Create and resolve a tension
        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Initial concern",
            "Something needs attention",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        update_tension_status(
            &conn,
            &dialogue_id,
            &t1,
            TensionStatus::Addressed,
            &["muffin".to_string()],
            None,
            0,
        )
        .unwrap();
        update_tension_status(
            &conn,
            &dialogue_id,
            &t1,
            TensionStatus::Resolved,
            &["muffin".to_string()],
            None,
            0,
        )
        .unwrap();

        // Create round 1 and reopen the tension
        create_round(&conn, &dialogue_id, 1, None, 12).unwrap();

        // New evidence causes reopening
        let e1 = register_evidence(
            &conn,
            &dialogue_id,
            1,
            "New data",
            "Previously unknown information surfaces",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        update_tension_status(
            &conn,
            &dialogue_id,
            &t1,
            TensionStatus::Reopened,
            &["muffin".to_string()],
            Some(&e1),
            1,
        )
        .unwrap();

        let tensions = get_tensions(&conn, &dialogue_id).unwrap();
        assert_eq!(tensions.len(), 1);
        assert_eq!(tensions[0].status, TensionStatus::Reopened);

        // The tension still has its original ID
        assert_eq!(
            display_id(EntityType::Tension, tensions[0].round, tensions[0].seq),
            "T0001"
        );
    }

    #[test]
    fn test_integration_cross_reference_graph() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Complex References", None, None, None).unwrap();

        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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

        create_round(&conn, &dialogue_id, 0, None, 20).unwrap();

        // Create a web of related entities
        let p1 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "Base perspective",
            "Foundation viewpoint",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Core tension",
            "Central issue",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let e1 = register_evidence(
            &conn,
            &dialogue_id,
            0,
            "Supporting data",
            "Facts backing the perspective",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let r1 = register_recommendation(
            &conn,
            &dialogue_id,
            0,
            "Proposed solution",
            "How to address the tension",
            &["muffin".to_string()],
            None,
            None,
        )
        .unwrap();

        let c1 = register_claim(
            &conn,
            &dialogue_id,
            0,
            "Synthesis",
            "Bringing it together",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        // Build the reference graph
        // E0001 supports P0001
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Evidence,
            &e1,
            RefType::Support,
            EntityType::Perspective,
            &p1,
        )
        .unwrap();
        // P0001 supports T0001 (highlights the tension)
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Perspective,
            &p1,
            RefType::Support,
            EntityType::Tension,
            &t1,
        )
        .unwrap();
        // R0001 addresses T0001
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Recommendation,
            &r1,
            RefType::Address,
            EntityType::Tension,
            &t1,
        )
        .unwrap();
        // C0001 depends on E0001
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Claim,
            &c1,
            RefType::Depend,
            EntityType::Evidence,
            &e1,
        )
        .unwrap();
        // C0001 resolves T0001
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Claim,
            &c1,
            RefType::Resolve,
            EntityType::Tension,
            &t1,
        )
        .unwrap();

        // Verify all entities created
        assert_eq!(get_perspectives(&conn, &dialogue_id).unwrap().len(), 1);
        assert_eq!(get_tensions(&conn, &dialogue_id).unwrap().len(), 1);
        assert_eq!(get_evidence(&conn, &dialogue_id).unwrap().len(), 1);
        assert_eq!(get_recommendations(&conn, &dialogue_id).unwrap().len(), 1);
        assert_eq!(get_claims(&conn, &dialogue_id).unwrap().len(), 1);

        // Query refs (would need a get_refs function for full testing)
        // For now, verify no errors occurred during registration
    }

    // ==================== Phase 6: Tooling Tests ====================

    #[test]
    fn test_expand_citation_perspective() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Citation Test", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        let p1 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "Test perspective",
            "This is the full content of the perspective for testing expansion",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let expanded = expand_citation(&conn, &dialogue_id, &p1).unwrap();

        assert_eq!(expanded.display_id, "P0001");
        assert_eq!(expanded.entity_type, EntityType::Perspective);
        assert_eq!(expanded.round, 0);
        assert_eq!(expanded.seq, 1);
        assert_eq!(expanded.label, "Test perspective");
        assert!(expanded.content.contains("full content"));
        assert_eq!(expanded.contributors, vec!["muffin"]);
        assert!(expanded.status.is_none()); // Perspectives don't have status
    }

    #[test]
    fn test_expand_citation_tension() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Tension Expansion", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Core conflict",
            "Two competing priorities",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let expanded = expand_citation(&conn, &dialogue_id, &t1).unwrap();

        assert_eq!(expanded.display_id, "T0001");
        assert_eq!(expanded.entity_type, EntityType::Tension);
        assert_eq!(expanded.label, "Core conflict");
        assert_eq!(expanded.status, Some("open".to_string())); // Tensions have status
    }

    #[test]
    fn test_expand_citation_recommendation_with_params() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Rec Expansion", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "donut",
            "Strategist",
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
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        let r1 = register_recommendation(
            &conn,
            &dialogue_id,
            0,
            "Options strategy",
            "Implement covered calls",
            &["donut".to_string()],
            Some(&serde_json::json!({"delta": 0.30, "dte": 45})),
            None,
        )
        .unwrap();

        let expanded = expand_citation(&conn, &dialogue_id, &r1).unwrap();

        assert_eq!(expanded.display_id, "R0001");
        assert_eq!(expanded.entity_type, EntityType::Recommendation);
        assert!(expanded.parameters.is_some());
        let params = expanded.parameters.unwrap();
        assert_eq!(params["delta"], 0.30);
    }

    #[test]
    fn test_expand_multiple_citations() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Multi Citation", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        let p1 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "First",
            "Content 1",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        let p2 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "Second",
            "Content 2",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Conflict",
            "Issue",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        let results = expand_citations(&conn, &dialogue_id, &[p1, p2, t1, "X9999".to_string()]);

        assert_eq!(results.len(), 4);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_ok());
        assert!(results[3].is_err()); // Invalid ID
    }

    #[test]
    fn test_cross_dialogue_stats() {
        let conn = setup_test_db();

        // Create multiple dialogues
        let d1 = create_dialogue(&conn, "Dialogue One", None, None, None).unwrap();
        let d2 = create_dialogue(&conn, "Dialogue Two", None, None, None).unwrap();

        register_expert(
            &conn,
            &d1,
            "muffin",
            "Analyst",
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
        register_expert(
            &conn,
            &d2,
            "muffin",
            "Analyst",
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

        create_round(&conn, &d1, 0, None, 30).unwrap();
        create_round(&conn, &d2, 0, None, 20).unwrap();

        register_perspective(
            &conn,
            &d1,
            0,
            "P1",
            "Content",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        register_perspective(
            &conn,
            &d1,
            0,
            "P2",
            "Content",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        register_perspective(
            &conn,
            &d2,
            0,
            "P3",
            "Content",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        register_tension(&conn, &d1, 0, "T1", "Issue", &["muffin".to_string()], None).unwrap();
        update_tension_status(
            &conn,
            &d1,
            "T0001",
            TensionStatus::Resolved,
            &["muffin".to_string()],
            None,
            0,
        )
        .unwrap();

        register_tension(&conn, &d2, 0, "T2", "Issue", &["muffin".to_string()], None).unwrap();

        update_expert_score(&conn, &d1, "muffin", 0, 15).unwrap();
        update_expert_score(&conn, &d2, "muffin", 0, 10).unwrap();

        let stats = get_cross_dialogue_stats(&conn).unwrap();

        assert_eq!(stats.total_dialogues, 2);
        assert_eq!(stats.total_perspectives, 3);
        assert_eq!(stats.total_tensions, 2);
        assert_eq!(stats.resolved_tensions, 1);
        assert_eq!(stats.open_tensions, 1);
        assert!(stats.avg_alignment > 0.0);
        assert_eq!(stats.total_experts, 1); // Same expert in both dialogues
        assert!(!stats.top_experts.is_empty());
        assert_eq!(stats.top_experts[0].0, "muffin");
    }

    #[test]
    fn test_dialogue_progress() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Progress Test", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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
        register_expert(
            &conn,
            &dialogue_id,
            "donut",
            "Skeptic",
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

        create_round(&conn, &dialogue_id, 0, None, 30).unwrap();
        create_round(&conn, &dialogue_id, 1, None, 25).unwrap();
        create_round(&conn, &dialogue_id, 2, None, 20).unwrap();

        register_tension(
            &conn,
            &dialogue_id,
            0,
            "Open tension",
            "Still unresolved",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        let t2 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "Resolved tension",
            "This was resolved",
            &["donut".to_string()],
            None,
        )
        .unwrap();
        update_tension_status(
            &conn,
            &dialogue_id,
            &t2,
            TensionStatus::Resolved,
            &["muffin".to_string(), "donut".to_string()],
            None,
            1,
        )
        .unwrap();

        update_expert_score(&conn, &dialogue_id, "muffin", 0, 12).unwrap();
        update_expert_score(&conn, &dialogue_id, "muffin", 1, 8).unwrap();
        update_expert_score(&conn, &dialogue_id, "donut", 0, 10).unwrap();
        update_expert_score(&conn, &dialogue_id, "donut", 1, 15).unwrap();

        let progress = get_dialogue_progress(&conn, &dialogue_id).unwrap();

        assert_eq!(progress.dialogue_id, dialogue_id);
        assert_eq!(progress.status, "open");
        assert_eq!(progress.total_rounds, 3);
        assert_eq!(progress.round_scores, vec![30, 25, 20]);
        assert_eq!(progress.velocity, -5); // 20 - 25
        assert_eq!(progress.open_tensions, 1);
        assert_eq!(progress.resolved_tensions, 1);
        assert_eq!(progress.active_experts.len(), 2);
        assert!(!progress.leaderboard.is_empty());
        // With velocity of -5, converging check needs 3 rounds of low velocity
        // Current data: 30->25 (-5), 25->20 (-5) - both are -5, abs(-5) < 5 is false
        assert!(!progress.converging);
    }

    #[test]
    fn test_dialogue_progress_convergence() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Converging Test", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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

        // Create rounds with diminishing velocity
        create_round(&conn, &dialogue_id, 0, None, 30).unwrap();
        create_round(&conn, &dialogue_id, 1, None, 32).unwrap(); // +2
        create_round(&conn, &dialogue_id, 2, None, 33).unwrap(); // +1

        let progress = get_dialogue_progress(&conn, &dialogue_id).unwrap();

        // velocity = 33 - 32 = 1
        assert_eq!(progress.velocity, 1);
        // v1 = 1, v2 = 2; both < 5, so converging
        assert!(progress.converging);
    }

    #[test]
    fn test_find_similar_dialogues() {
        let conn = setup_test_db();

        create_dialogue(
            &conn,
            "Investment Portfolio Review",
            Some("Should we rebalance?"),
            None,
            None,
        )
        .unwrap();
        create_dialogue(
            &conn,
            "Marketing Strategy",
            Some("How to increase reach"),
            None,
            None,
        )
        .unwrap();
        create_dialogue(
            &conn,
            "Investment Risk Assessment",
            Some("What are the risks?"),
            None,
            None,
        )
        .unwrap();

        let results = find_similar_dialogues(&conn, "investment", 10).unwrap();

        assert_eq!(results.len(), 2); // Two dialogues match "investment"
        assert!(results
            .iter()
            .any(|(_, title, _)| title.contains("Portfolio")));
        assert!(results.iter().any(|(_, title, _)| title.contains("Risk")));
    }

    // ==================== Performance & Isolation Tests ====================

    #[test]
    fn test_output_directory_isolation() {
        let conn = setup_test_db();

        // Create dialogues with similar titles - each gets unique output dir via unique ID
        let d1 = create_dialogue(
            &conn,
            "Investment Analysis",
            Some("First analysis"),
            Some("/tmp/blue-dialogue/investment-analysis"),
            None,
        )
        .unwrap();
        let d2 = create_dialogue(
            &conn,
            "Investment Analysis",
            Some("Second analysis"),
            Some("/tmp/blue-dialogue/investment-analysis-2"),
            None,
        )
        .unwrap();
        let d3 = create_dialogue(
            &conn,
            "Investment Analysis",
            Some("Third analysis"),
            Some("/tmp/blue-dialogue/investment-analysis-3"),
            None,
        )
        .unwrap();

        // Verify unique IDs
        assert_eq!(d1, "investment-analysis");
        assert_eq!(d2, "investment-analysis-2");
        assert_eq!(d3, "investment-analysis-3");

        // Verify dialogues are isolated
        let dialogue1 = get_dialogue(&conn, &d1).unwrap();
        let dialogue2 = get_dialogue(&conn, &d2).unwrap();
        let dialogue3 = get_dialogue(&conn, &d3).unwrap();

        assert_eq!(
            dialogue1.output_dir,
            Some("/tmp/blue-dialogue/investment-analysis".to_string())
        );
        assert_eq!(
            dialogue2.output_dir,
            Some("/tmp/blue-dialogue/investment-analysis-2".to_string())
        );
        assert_eq!(
            dialogue3.output_dir,
            Some("/tmp/blue-dialogue/investment-analysis-3".to_string())
        );

        // Add experts to different dialogues - they remain isolated
        register_expert(
            &conn,
            &d1,
            "muffin",
            "Analyst A",
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
        register_expert(
            &conn,
            &d2,
            "muffin",
            "Analyst B",
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

        let experts1 = get_experts(&conn, &d1).unwrap();
        let experts2 = get_experts(&conn, &d2).unwrap();

        assert_eq!(experts1.len(), 1);
        assert_eq!(experts2.len(), 1);
        assert_eq!(experts1[0].role, "Analyst A");
        assert_eq!(experts2[0].role, "Analyst B");
    }

    #[test]
    fn test_performance_many_perspectives() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Large Dialogue", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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

        // Create 10 rounds with 10 perspectives each = 100 perspectives
        for round in 0..10 {
            create_round(&conn, &dialogue_id, round, None, 10).unwrap();
            for _seq in 0..10 {
                register_perspective(
                    &conn,
                    &dialogue_id,
                    round,
                    "Test perspective",
                    "Content for testing performance",
                    &["muffin".to_string()],
                    None,
                )
                .unwrap();
            }
        }

        // Query all perspectives - should be fast with indices
        let start = std::time::Instant::now();
        let perspectives = get_perspectives(&conn, &dialogue_id).unwrap();
        let duration = start.elapsed();

        assert_eq!(perspectives.len(), 100);
        // Should complete in under 100ms with proper indices
        assert!(
            duration.as_millis() < 100,
            "Query took too long: {:?}",
            duration
        );
    }

    #[test]
    fn test_indices_exist() {
        let conn = setup_test_db();

        // Query SQLite for index info
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'")
            .unwrap();

        let indices: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        // Verify key indices exist
        assert!(indices.iter().any(|n| n.contains("experts_dialogue")));
        assert!(indices.iter().any(|n| n.contains("perspectives_dialogue")));
        assert!(indices.iter().any(|n| n.contains("tensions_dialogue")));
        assert!(indices.iter().any(|n| n.contains("tensions_status")));
        assert!(indices.iter().any(|n| n.contains("refs_dialogue")));
        assert!(indices.iter().any(|n| n.contains("refs_target")));
    }

    #[test]
    fn test_no_orphaned_entities() {
        let conn = setup_test_db();

        let dialogue_id = create_dialogue(&conn, "Orphan Test", None, None, None).unwrap();
        register_expert(
            &conn,
            &dialogue_id,
            "muffin",
            "Analyst",
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
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        // Register entities
        let p1 = register_perspective(
            &conn,
            &dialogue_id,
            0,
            "P1",
            "Content",
            &["muffin".to_string()],
            None,
        )
        .unwrap();
        let t1 = register_tension(
            &conn,
            &dialogue_id,
            0,
            "T1",
            "Issue",
            &["muffin".to_string()],
            None,
        )
        .unwrap();

        // Register a ref between entities
        register_ref(
            &conn,
            &dialogue_id,
            EntityType::Perspective,
            &p1,
            RefType::Address,
            EntityType::Tension,
            &t1,
        )
        .unwrap();

        // All entities should be queryable and connected
        let perspectives = get_perspectives(&conn, &dialogue_id).unwrap();
        let tensions = get_tensions(&conn, &dialogue_id).unwrap();

        assert_eq!(perspectives.len(), 1);
        assert_eq!(tensions.len(), 1);

        // Refs should exist
        let ref_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM alignment_refs WHERE dialogue_id = ?1",
                params![dialogue_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ref_count, 1);

        // Verify ref connects to valid entities
        let ref_row: (String, String) = conn
            .query_row(
                "SELECT source_id, target_id FROM alignment_refs WHERE dialogue_id = ?1",
                params![dialogue_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(ref_row.0, p1);
        assert_eq!(ref_row.1, t1);
    }

    // ==================== RFC 0057 Tests ====================

    #[test]
    fn test_score_components() {
        let sc = ScoreComponents {
            wisdom: 10,
            consistency: 5,
            truth: 8,
            relationships: 3,
        };
        assert_eq!(sc.total(), 26);

        let empty = ScoreComponents::default();
        assert_eq!(empty.total(), 0);
    }

    #[test]
    fn test_convergence_metrics() {
        // Can converge: velocity=0, 100% signals
        let cm = ConvergenceMetrics {
            open_tensions: 0,
            new_perspectives: 0,
            converge_signals: 12,
            panel_size: 12,
        };
        assert_eq!(cm.velocity(), 0);
        assert_eq!(cm.converge_percent(), 100.0);
        assert!(cm.can_converge());

        // Cannot converge: velocity > 0
        let cm2 = ConvergenceMetrics {
            open_tensions: 2,
            new_perspectives: 1,
            converge_signals: 12,
            panel_size: 12,
        };
        assert_eq!(cm2.velocity(), 3);
        assert!(!cm2.can_converge());

        // Cannot converge: not 100% signals
        let cm3 = ConvergenceMetrics {
            open_tensions: 0,
            new_perspectives: 0,
            converge_signals: 10,
            panel_size: 12,
        };
        assert_eq!(cm3.velocity(), 0);
        assert!(cm3.converge_percent() < 100.0);
        assert!(!cm3.can_converge());

        // Edge case: panel_size = 0
        let cm4 = ConvergenceMetrics {
            open_tensions: 0,
            new_perspectives: 0,
            converge_signals: 0,
            panel_size: 0,
        };
        assert_eq!(cm4.converge_percent(), 0.0);
        assert!(!cm4.can_converge());
    }

    #[test]
    fn test_create_round_with_metrics() {
        let conn = setup_test_db();
        let dialogue_id = create_dialogue(&conn, "Metrics Test", None, None, None).unwrap();

        let sc = ScoreComponents {
            wisdom: 5,
            consistency: 3,
            truth: 4,
            relationships: 2,
        };
        let cm = ConvergenceMetrics {
            open_tensions: 3,
            new_perspectives: 2,
            converge_signals: 8,
            panel_size: 12,
        };

        create_round_with_metrics(
            &conn,
            &dialogue_id,
            0,
            Some("Round 0"),
            14, // 5+3+4+2
            Some(&sc),
            Some(&cm),
        )
        .unwrap();

        // Verify the round was created with correct metrics
        let row: (i32, i32, i32, i32, i32, i32, i32, i32) = conn
            .query_row(
                "SELECT score_wisdom, score_consistency, score_truth, score_relationships,
                    open_tensions, new_perspectives, converge_signals, panel_size
             FROM alignment_rounds WHERE dialogue_id = ?1 AND round = 0",
                params![dialogue_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, 5); // wisdom
        assert_eq!(row.1, 3); // consistency
        assert_eq!(row.2, 4); // truth
        assert_eq!(row.3, 2); // relationships
        assert_eq!(row.4, 3); // open_tensions
        assert_eq!(row.5, 2); // new_perspectives
        assert_eq!(row.6, 8); // converge_signals
        assert_eq!(row.7, 12); // panel_size
    }

    #[test]
    fn test_convergence_signals() {
        let conn = setup_test_db();
        let dialogue_id = create_dialogue(&conn, "Signals Test", None, None, None).unwrap();
        create_round(&conn, &dialogue_id, 0, None, 10).unwrap();

        // Record signals from multiple experts
        record_convergence_signal(&conn, &dialogue_id, 0, "muffin").unwrap();
        record_convergence_signal(&conn, &dialogue_id, 0, "cupcake").unwrap();
        record_convergence_signal(&conn, &dialogue_id, 0, "scone").unwrap();

        // Retrieve signals
        let signals = get_convergence_signals(&conn, &dialogue_id, 0).unwrap();
        assert_eq!(signals.len(), 3);
        assert!(signals.contains(&"muffin".to_string()));
        assert!(signals.contains(&"cupcake".to_string()));
        assert!(signals.contains(&"scone".to_string()));

        // Re-recording same signal should replace (OR REPLACE)
        record_convergence_signal(&conn, &dialogue_id, 0, "muffin").unwrap();
        let signals2 = get_convergence_signals(&conn, &dialogue_id, 0).unwrap();
        assert_eq!(signals2.len(), 3); // Still 3, not 4
    }

    #[test]
    fn test_get_scoreboard() {
        let conn = setup_test_db();
        let dialogue_id = create_dialogue(&conn, "Scoreboard Test", None, None, None).unwrap();

        // Create round 0
        let sc0 = ScoreComponents {
            wisdom: 10,
            consistency: 5,
            truth: 8,
            relationships: 3,
        };
        let cm0 = ConvergenceMetrics {
            open_tensions: 5,
            new_perspectives: 3,
            converge_signals: 4,
            panel_size: 12,
        };
        create_round_with_metrics(
            &conn,
            &dialogue_id,
            0,
            Some("Round 0"),
            26,
            Some(&sc0),
            Some(&cm0),
        )
        .unwrap();

        // Create round 1
        let sc1 = ScoreComponents {
            wisdom: 8,
            consistency: 6,
            truth: 4,
            relationships: 2,
        };
        let cm1 = ConvergenceMetrics {
            open_tensions: 2,
            new_perspectives: 1,
            converge_signals: 10,
            panel_size: 12,
        };
        create_round_with_metrics(
            &conn,
            &dialogue_id,
            1,
            Some("Round 1"),
            20,
            Some(&sc1),
            Some(&cm1),
        )
        .unwrap();

        // Get scoreboard
        let scoreboard = get_scoreboard(&conn, &dialogue_id).unwrap();
        assert_eq!(scoreboard.len(), 2);

        // Round 0
        let r0 = &scoreboard[0];
        assert_eq!(r0.round, 0);
        assert_eq!(r0.w, 10);
        assert_eq!(r0.c, 5);
        assert_eq!(r0.t, 8);
        assert_eq!(r0.r, 3);
        assert_eq!(r0.total, 26);
        assert_eq!(r0.velocity, 8); // 5 + 3
        assert_eq!(r0.cumulative_score, 26);

        // Round 1
        let r1 = &scoreboard[1];
        assert_eq!(r1.round, 1);
        assert_eq!(r1.w, 8);
        assert_eq!(r1.velocity, 3); // 2 + 1
        assert_eq!(r1.cumulative_score, 46); // 26 + 20
        assert_eq!(r1.cumulative_w, 18); // 10 + 8
    }

    #[test]
    fn test_can_dialogue_converge() {
        let conn = setup_test_db();
        let dialogue_id = create_dialogue(&conn, "Converge Test", None, None, None).unwrap();

        // No rounds yet
        let (can, blockers) = can_dialogue_converge(&conn, &dialogue_id).unwrap();
        assert!(!can);
        assert!(blockers.iter().any(|b| b.contains("no rounds")));

        // Round with velocity > 0 and incomplete signals
        let sc = ScoreComponents {
            wisdom: 5,
            consistency: 3,
            truth: 4,
            relationships: 2,
        };
        let cm = ConvergenceMetrics {
            open_tensions: 2,
            new_perspectives: 1,
            converge_signals: 8,
            panel_size: 12,
        };
        create_round_with_metrics(&conn, &dialogue_id, 0, None, 14, Some(&sc), Some(&cm)).unwrap();

        let (can, blockers) = can_dialogue_converge(&conn, &dialogue_id).unwrap();
        assert!(!can);
        assert!(blockers.iter().any(|b| b.contains("velocity=3")));
        assert!(blockers.iter().any(|b| b.contains("converge_percent=")));

        // Round where convergence is possible
        let sc2 = ScoreComponents {
            wisdom: 2,
            consistency: 1,
            truth: 1,
            relationships: 1,
        };
        let cm2 = ConvergenceMetrics {
            open_tensions: 0,
            new_perspectives: 0,
            converge_signals: 12,
            panel_size: 12,
        };
        create_round_with_metrics(&conn, &dialogue_id, 1, None, 5, Some(&sc2), Some(&cm2)).unwrap();

        let (can, blockers) = can_dialogue_converge(&conn, &dialogue_id).unwrap();
        assert!(can);
        assert!(blockers.is_empty());
    }
}
