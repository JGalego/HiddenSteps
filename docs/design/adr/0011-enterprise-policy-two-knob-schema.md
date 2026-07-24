# ADR-0011: Enterprise policy as a fixed two-knob, restrict-only schema

Status: Accepted

## Context

PROMPT.md's Enterprise Deployment section asks for policy management,
preconfigured providers, and managed/air-gapped deployment. But HiddenSteps'
whole premise is that it answers to the user, not their employer — NG2 in
[../01-prd.md](../01-prd.md) and [../../research/04-ethical-analysis.md](../../research/04-ethical-analysis.md)
are explicit that there is no "manager view" and never will be, and the
architecture assumes individual consent regardless of employer deployment.

An enterprise-policy mechanism is therefore a live tension: IT admins have
legitimate needs (mandate a minimum privacy posture, restrict which AI
providers corporate data may reach), but a general-purpose policy engine is
exactly the shape of thing that could quietly turn a user-serving tool into a
surveillance channel. The question this ADR settles is *what a policy is
structurally allowed to express* — not how it's loaded.

## Decision

An enterprise policy has **exactly two knobs**, and both can only ever make the
product **more** restrictive, never less:

1. `privacy_level_floor` — a *minimum* privacy level the device must run at if
   it observes at all. It can only *raise* a user's chosen level toward more
   observation being disallowed-below; it is applied via
   `effective_privacy_level(user_choice)`, which returns `max(floor,
   user_choice)` and so can never lower what the user picked.
2. `provider_allowlist` — if present, the set of AI provider ids that may be
   selected/activated. It can only *narrow* the providers available.

There is deliberately **no field** for anything else a policy might want:
no redaction-confidence override, no field to disable the
Level-4-never-cloud-eligible rule, no field to disable any trust-dashboard
feature, no retention-extension or deletion-disable. Those hard rules are
enforced inside `hiddensteps-privacy-engine`, `hiddensteps-redaction`, and
`hiddensteps-event-store`, none of which take an `EnterprisePolicy` as input at
all. The closed schema *is* the enforcement mechanism: a policy author cannot
request those things because there is nowhere in the type to write the request,
and any extra JSON keys are dropped at parse time (verified by test).

Because both knobs are restrict-only, the policy file needs no signature/
integrity check to be safe (see the note in `hiddensteps-enterprise-policy`):
tampering with it can at worst return the device to its unconstrained default,
never weaken a hard guarantee.

## Consequences

- The "employer can mandate a floor and an allowlist, but can never see the
  user's data or weaken a privacy guarantee" boundary is structural, not a
  convention the loader happens to respect — the same discipline ADR-0009
  applies to plugin capabilities.
- PROMPT.md's "preconfigured AI providers" is only *partially* met: the
  allowlist restricts choice, but the schema intentionally does not let a
  policy *pin* or *inject* a provider configuration (endpoint, key), which
  would be a channel for pushing a cloud endpoint at a user. A managed
  deployment preconfigures providers by shipping them as normal
  provider config plus an allowlist, not by a policy field that writes
  provider credentials.
- Any future need to let a policy *relax* a default (rather than tighten it)
  would require adding signing/integrity verification first, and its own ADR —
  it is not a schema addition, it is a trust-model change.

## Implementation status (v0.1.x)

Built and enforced. The schema lives in `hiddensteps-enterprise-policy`; it's
loaded from an `enterprise-policy.json` file in the app data directory (an
interim mechanism — the full `PolicyLoader` plugin connector in
[08-plugin-architecture.md](../08-plugin-architecture.md) §6 isn't built),
persisted via `hiddensteps-event-store`, and enforced in the desktop shell's
`set_privacy_level` / `set_ai_provider` commands — the two points a level or
provider is ever chosen. See [`../../../crates/README.md`](../../../crates/README.md).

## Alternatives considered

- **A general key/value policy schema** (arbitrary settings an admin can set):
  rejected outright — it is precisely the shape that could express a
  surveillance or guarantee-weakening policy, and "the loader validates it
  won't" is not a structural boundary.
- **MDM/registry-based policy instead of a file**: not rejected, deferred —
  the file mechanism is the interim form; a real `PolicyLoader` connector to an
  enterprise config-management system is the designed end state
  (08-plugin-architecture.md §6). The *schema* this ADR fixes is independent of
  how the policy is delivered.
