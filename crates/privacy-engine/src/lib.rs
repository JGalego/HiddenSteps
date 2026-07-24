//! The Privacy Engine (`docs/design/02-system-architecture.md`): the
//! cloud-dispatch gate every `LlmProvider` call site must pass through
//! (`docs/design/03-data-flow-diagrams.md` §5, ADR-0004), plus consent-versioning
//! per `docs/design/05-privacy-model.md` §5.
//!
//! Enterprise policy interaction (§6 of the same doc — a policy pack may raise a
//! privacy-level floor or narrow the provider allowlist, but cannot loosen any
//! rule in this crate) is **not** implemented inside this crate: a policy's two
//! knobs (`hiddensteps_enterprise_policy::EnterprisePolicy::effective_privacy_level`/
//! `is_provider_allowed`) constrain *which* privacy level and provider a user
//! can select in the first place — that's enforced where those selections are
//! made (`apps/desktop/src-tauri/src/commands.rs`'s `set_privacy_level`/
//! `set_ai_provider`), not inside `DispatchGate`, whose own job (per-dispatch
//! cloud-eligibility given whatever level and provider are already in effect)
//! has no notion of provider identity or policy at all. This crate has no API
//! that would let a policy weaken what's enforced here — nor does it need one,
//! since a policy never gets the chance to affect a level/provider `DispatchGate`
//! wasn't going to see anyway.

mod consent;
mod gate;
mod gated_provider;

pub use consent::requires_reconsent;
pub use gate::{cloud_eligibility, CloudEligibility, DispatchDecision, DispatchGate};
pub use gated_provider::{GateError, PrivacyGatedProvider};
