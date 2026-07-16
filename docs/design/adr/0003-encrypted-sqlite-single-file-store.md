# ADR-0003: Encrypted single-file SQLite (SQLCipher) as the primary and only durable store

Status: Accepted

## Context

PROMPT.md requires an encrypted local database, encrypted embeddings/caches/settings, and a Portable Mode that "leaves no traces after deletion" and is "suitable for USB drives." The privacy pipeline ([../../research/05-privacy-analysis.md](../../research/05-privacy-analysis.md)) requires that summaries, embeddings, recommendations, settings, and the audit log all be encrypted at rest, and that deleting a user's data be a complete, verifiable operation.

## Decision

Use **SQLite compiled with SQLCipher** (via `rusqlite`'s `bundled-sqlcipher` feature) as the single durable store for everything: workflow-event summaries, detected patterns, the workflow graph, recommendations, embeddings (via the `sqlite-vec` extension loaded into the same database), settings, and the audit log. One encrypted file per user profile (`hiddensteps.db`), keyed from a master key held in the OS credential vault (ADR-0008).

## Consequences

- **Portable Mode becomes trivial**: the entire user state is one file; "leaves no traces" reduces to "delete this file and its vault entry," which is directly verifiable rather than requiring a filesystem audit across multiple stores/caches.
- **Backup/export is one file**: the Export Data trust feature (PROMPT.md) is a controlled decrypt-and-serialize of a single source, not a multi-store reconciliation.
- **No separate vector database process or embedded service** is needed — `sqlite-vec` runs in-process, keeping the "no setup complexity" onboarding goal intact and avoiding a second attack surface / second key to manage.
- Write concurrency is more limited than a client-server DB (SQLite's single-writer model), which is acceptable: this is a single-user, single-process application with no concurrent-writer requirement.
- Embedding-similarity search at very large scale (multi-year histories, hundreds of thousands of events) may eventually need `sqlite-vec`'s approximate-search mode or a data-retention/compaction policy rather than exhaustive scan; tracked as a forward-looking scaling note, not a Phase 2 blocker given the pipeline's design intent to retain abstractions, not raw events (ADR-0006).

## Alternatives considered

- **Postgres/pgvector or a standalone vector DB (Chroma, LanceDB) as a separate process**: rejected — adds a second running service, a second thing to encrypt/secure/back up, and directly works against the "no setup complexity" and single-file-portability requirements.
- **Plaintext SQLite + OS-level full-disk encryption reliance**: rejected — full-disk encryption is not guaranteed to be enabled, doesn't protect against a compromised OS-user-level attacker, and doesn't satisfy PROMPT.md's explicit "encrypted local database" requirement as an application-level guarantee independent of OS configuration.
