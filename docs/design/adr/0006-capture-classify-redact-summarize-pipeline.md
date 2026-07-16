# ADR-0006: Event Pipeline as a strict, one-directional, in-memory-first pipeline

Status: Accepted

## Context

[../../research/05-privacy-analysis.md](../../research/05-privacy-analysis.md) establishes Capture → Classify → Redact → Summarize → Embed → Delete raw → Retain as the required data pipeline, with the explicit discipline that raw capture must never reach durable, unencrypted, or long-lived storage as an intermediate convenience.

## Decision

Implement the Event Pipeline as an in-process, in-memory ring buffer of raw `CapturedSignal` values that only ever flows forward through Classify → Redact → Summarize stages before anything is handed to the Embedding Layer or `EventStore` for durable write. No stage may write raw or partially-redacted content to disk (including temp files, swap-eligible memory, or crash/telemetry reporters — enforced by using a zeroize-on-drop buffer type and explicitly excluding pipeline buffers from any diagnostic dump). The pipeline is synchronous per-event through Redact (redaction must complete before an event is eligible for storage) and only Summarize/Embed/Store are eligible for batching or idle-time scheduling.

## Consequences

- The "raw data expires quickly, insights remain" principle becomes a type-level property (the raw `CapturedSignal` type is never `Serialize`/never reaches an `EventStore` API) rather than a policy that could be violated by a future careless change.
- If the Redaction stage is uncertain about content sensitivity, the pipeline's default is **drop the event rather than store it under-redacted** (per [../../research/05-privacy-analysis.md](../../research/05-privacy-analysis.md)'s proportionality principle) — implemented as a `RedactionOutcome::Drop` variant the Summarize stage must handle, not an error path that could be silently ignored.
- Batching Summarize/Embed/Store for idle-time scheduling (per the resource-consumption mitigation in [../../research/03-risk-analysis.md](../../research/03-risk-analysis.md)) is safe precisely because redaction has already happened before batching — nothing sensitive sits in the batch queue.

## Alternatives considered

- **Async/eventually-consistent pipeline with a durable raw-event queue (e.g., writing raw events to disk for later batch processing)**: rejected — this is exactly the "we'll just cache screenshots for a day for debugging" anti-pattern the privacy analysis calls out; any durable raw-event queue becomes the highest-value breach target in the threat model.
