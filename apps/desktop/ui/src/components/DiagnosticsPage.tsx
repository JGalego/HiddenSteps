import { useEffect, useState } from "react";
import { tauriBridge, type Diagnostics } from "../tauriBridge";

function formatBytes(bytes: number | null): string {
  if (bytes === null) return "unknown";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * PROMPT.md's Self-Diagnostics page. Every value rendered here comes straight
 * from `get_diagnostics` — real counts and a real file size, not placeholder
 * numbers. See `hiddensteps-desktop`'s `Diagnostics` struct doc comment for
 * the fields this deliberately does not report yet (GPU/CPU/memory,
 * observation OS-permission status, update status) — a disclosed gap, not a
 * fabricated "OK" for something never actually checked.
 */
export function DiagnosticsPage() {
  const [diagnostics, setDiagnostics] = useState<Diagnostics | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    tauriBridge
      .getDiagnostics()
      .then(setDiagnostics)
      .catch((e) => setError(String(e)));
  }, []);

  if (error) {
    return (
      <section aria-label="Diagnostics">
        <h1>Diagnostics</h1>
        <p role="alert">{error}</p>
      </section>
    );
  }

  if (!diagnostics) {
    return (
      <section aria-label="Diagnostics">
        <h1>Diagnostics</h1>
        <p>Loading…</p>
      </section>
    );
  }

  return (
    <section aria-label="Diagnostics">
      <h1>Diagnostics</h1>
      <dl>
        <dt>Observation</dt>
        <dd>{diagnostics.observation_active ? "Active" : "Paused"} — Level {diagnostics.privacy_level}</dd>

        <dt>AI provider</dt>
        <dd data-testid="diag-provider">
          {diagnostics.active_provider
            ? `${diagnostics.active_provider.id} (${diagnostics.active_provider.is_local ? "local" : "cloud"})`
            : "None configured"}
        </dd>

        <dt>Storage</dt>
        <dd>{formatBytes(diagnostics.storage_bytes)}</dd>

        <dt>Encryption</dt>
        <dd>{diagnostics.encryption_status}</dd>

        <dt>Observed events</dt>
        <dd>{diagnostics.event_count}</dd>

        <dt>Detected patterns</dt>
        <dd>{diagnostics.pattern_count}</dd>

        <dt>Recommendations</dt>
        <dd>{diagnostics.recommendation_count}</dd>

        <dt>Audit log entries</dt>
        <dd>{diagnostics.audit_log_count}</dd>
      </dl>
      <p>
        Not yet reported here: GPU/CPU/memory usage, observation OS-permission status, update
        status.
      </p>
    </section>
  );
}
