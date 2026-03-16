//! PM Repo as Ground Truth (RFC 0068, RFC 0070)
//!
//! The PM repo is the single source of truth for an org's project artifacts.
//! Every RFC belongs to a story, every story belongs to an epic.
//! Epics use the org key (e.g., TMS-01). Stories use the area key (e.g., CON-001).
//! Components map to Jira components for functional ownership.
//! Areas map to product surfaces and provide story key prefixes.

pub mod domain;
pub mod id;
pub mod locator;
pub mod sync;
