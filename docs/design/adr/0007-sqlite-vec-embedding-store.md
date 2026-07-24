# ADR-0007: `sqlite-vec` in-process vector search over summary embeddings

Status: Accepted (decision) — **shipped implementation deviates; see the note below**

> **Implementation note (current, as of v0.1.x).** The shipped
> `hiddensteps-event-store` does **not** load the `sqlite-vec` native
> extension. Embeddings are stored as plain `BLOB`s in the SQLCipher file and
> similarity is computed in Rust (brute-force cosine). This was a deliberate,
> disclosed simplification — loading a native SQLite extension wasn't
> verifiable in the build environment — and it has the *same semantics* this
> ADR accepts: at a single user's realistic volume, `sqlite-vec` itself does
> brute-force exact search (see the Consequences below). The decision to keep
> embeddings inside the one encrypted file, embedding only summaries, is
> unchanged; only the mechanism differs. `crates/README.md` is the ground
> truth for what's built. Moving to the real `sqlite-vec` extension is a
> drop-in swap behind the same store API, not an architecture change.

## Context

Pattern Detection and the Workflow Graph need similarity search over embedded workflow summaries ("has this app-sequence-shape recurred before?") to detect repetition across weeks/months. ADR-0003 commits to a single encrypted SQLite file as the only durable store; the embedding layer needs to fit inside that constraint.

## Decision

Use the `sqlite-vec` extension, loaded into the same SQLCipher-encrypted `hiddensteps.db`, storing embeddings of **pattern/workflow summaries**, not raw captured content (consistent with ADR-0006: only post-redaction, post-summarize data is ever embedded). Embedding generation itself runs through the `LlmProvider` trait's embedding method (ADR-0004), defaulting to a small local embedding model (e.g., served via Ollama) so this step never requires network access.

## Consequences

- No second database, no second encryption key, no second backup/export target — embeddings inherit every guarantee ADR-0003 already established (single-file portability, one-shot deletion, encrypted-at-rest).
- Similarity search is exact (brute-force cosine/L2 over stored vectors) at `sqlite-vec`'s current maturity level, which is sufficient for a single user's realistic event volume (thousands to low tens of thousands of pattern-level, not raw-event-level, embeddings); if volume growth ever makes exact search too slow, `sqlite-vec`'s partitioned/ANN modes are a drop-in upgrade within the same file, not an architecture change.
- Because only summaries are embedded (never raw window/clipboard/OCR content), an embedding-inversion attack against the vector store has a much smaller blast radius than it would against raw-content embeddings — a direct consequence of the pipeline discipline in ADR-0006.

## Alternatives considered

- **A dedicated vector database (Qdrant, Chroma, LanceDB) as a sidecar process**: rejected for the same reasons as ADR-0003 — second process, second key, second attack surface, worse fit for Portable Mode.
- **No vector search; pure rule-based sequence matching for pattern detection**: rejected as the sole approach — exact sequence matching alone misses near-duplicate workflows (same intent, slightly different app order or timing) that embedding similarity catches; rule-based detection remains the first-pass, cheap filter (see [02-system-architecture.md](../02-system-architecture.md)), with embeddings as the fuzzy-matching layer on top.
