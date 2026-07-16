//! The Privacy Engine (`docs/design/02-system-architecture.md`): the
//! cloud-dispatch gate every `LlmProvider` call site must pass through
//! (`docs/design/03-data-flow-diagrams.md` §5, ADR-0004), plus consent-versioning
//! per `docs/design/05-privacy-model.md` §5.
//!
//! Enterprise policy interaction (§6 of the same doc — a policy pack may raise a
//! privacy-level floor or narrow the provider allowlist, but cannot loosen any
//! rule in this crate) is implemented in `hiddensteps-enterprise-policy`, which
//! composes with `DispatchGate` rather than modifying it — this crate has no API
//! that would let a policy weaken what's enforced here.

mod consent;
mod gate;
mod gated_provider;

pub use consent::requires_reconsent;
pub use gate::{cloud_eligibility, CloudEligibility, DispatchDecision, DispatchGate};
pub use gated_provider::{GateError, PrivacyGatedProvider};
