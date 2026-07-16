//! Pattern Detection and Workflow Graph (two modules in
//! `docs/design/02-system-architecture.md`, implemented as one crate here since
//! both operate on the same chronological event history and Workflow Graph
//! explicitly "feeds" from Pattern Detection's output per the architecture doc —
//! a pragmatic implementation-level merge, not a design change).
//!
//! Both are pure, deterministic functions over `&[EventSummary]` — no I/O, no
//! `EventStore` dependency, no LLM. This is ADR-0010's Layer 1: the facts a
//! recommendation's numeric claims (frequency, time span) trace back to.

mod detector;
mod workflow_graph;

pub use detector::{action_key, DetectedPattern, PatternDetector};
pub use workflow_graph::{build_workflow_graph, WorkflowEdge, WorkflowGraph};
