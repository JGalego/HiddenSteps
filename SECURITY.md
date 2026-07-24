# Security Policy

HiddenSteps is a local-first tool whose entire value proposition rests on
handling sensitive personal data trustworthily (see
[docs/design/04-trust-model.md](docs/design/04-trust-model.md) and
[docs/research/06-threat-model.md](docs/research/06-threat-model.md)). We take
security reports seriously and want to make responsible disclosure easy — this
policy operationalizes the "public responsible-disclosure process ... should
exist before GA" commitment in
[docs/roadmap/04-security-testing.md](docs/roadmap/04-security-testing.md) §2.

## Supported versions

This is an early, pre-1.0 project. Security fixes are made against the latest
released version and `main`. Older prereleases are not maintained — please
reproduce on the latest build before reporting.

## Reporting a vulnerability

**Please do not open a public GitHub issue for a security vulnerability.**

Report privately through one of:

- **GitHub private vulnerability reporting** — the preferred channel: open the
  repository's **Security → Report a vulnerability** tab
  (`https://github.com/JGalego/HiddenSteps/security/advisories/new`). This
  keeps the report private until a fix is ready and gives us a coordinated
  place to work with you.
- If that is unavailable to you, open a regular issue containing **only** the
  words "security report — please contact me" (no details) and a maintainer
  will arrange a private channel.

Please include, to the extent you can:

- the version / commit you tested,
- your platform (OS + version),
- a description of the issue and its impact,
- reproduction steps or a proof-of-concept,
- any suggested remediation.

## What to expect

- **Acknowledgement** within **3 business days** that we've received your
  report.
- An initial **assessment** (severity, whether we can reproduce) within
  **10 business days**.
- We aim to ship a fix, or agree a timeline with you, within **90 days** of
  the acknowledged report. We will keep you updated on progress.
- We practice **coordinated disclosure**: we ask that you give us the window
  above before public disclosure, and we will credit you (if you wish) in the
  release notes and any published advisory.

## Scope

In scope: the HiddenSteps application and the crates in this repository —
especially anything that would let observation, redaction, the cloud-dispatch
gate, the plugin sandbox, or the encrypted store behave contrary to the
guarantees documented in `docs/`.

Out of scope: vulnerabilities in third-party dependencies (report those
upstream; we track dependency advisories via `cargo audit` / `npm audit` in
CI and Dependabot), and issues requiring a already-fully-compromised host
(e.g. an attacker who is already root on the user's machine).

## Our commitments to you

- We will not pursue legal action against good-faith security research
  conducted in line with this policy.
- We will treat your report as confidential and not share your identity
  without your permission.
