import { useCallback, useEffect, useState } from "react";
import { tauriBridge, type LlmProviderConfig, type PrivacyState } from "../tauriBridge";

/**
 * docs/ux/05-settings-and-complexity-tiers.md's Privacy/AI-Provider sections.
 * Per that doc's closing design rule: privacy level is never tier-gated, so
 * it's always shown here regardless of complexity tier (this component
 * doesn't implement the tier filter itself — see the README's disclosed-gap
 * note — it shows the "Beginner"-visible fields, which is the correct subset
 * either way).
 */
export function SettingsPage() {
  const [status, setStatus] = useState<PrivacyState | null>(null);
  const [providers, setProviders] = useState<LlmProviderConfig[]>([]);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatus, nextProviders] = await Promise.all([
        tauriBridge.getObservationStatus(),
        tauriBridge.listLlmProviders(),
      ]);
      setStatus(nextStatus);
      setProviders(nextProviders);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const changeLevel = async (level: number) => {
    await tauriBridge.setPrivacyLevel(level, ["acknowledged"]);
    await refresh();
  };

  return (
    <section aria-label="Settings">
      <h1>Settings</h1>
      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}

      <div className="card section-block">
        <h2>Privacy</h2>
        {status && (
          <p>
            Level: <strong>{status.current_level}</strong>
            <span className="inline-btn-group">
              <button
                className="btn"
                type="button"
                onClick={() => changeLevel(Math.max(0, status.current_level - 1))}
              >
                Lower
              </button>
              <button
                className="btn"
                type="button"
                onClick={() => changeLevel(Math.min(4, status.current_level + 1))}
              >
                Raise
              </button>
            </span>
          </p>
        )}
      </div>

      <div className="card section-block">
        <h2>AI Provider</h2>
        {providers.length === 0 && <p>No provider configured yet.</p>}
        <ul className="provider-list" data-testid="provider-list">
          {providers.map((p) => (
            <li key={p.id}>
              {p.active ? "● " : "○ "}
              {p.id} ({p.provider_type}
              {p.is_local ? ", local" : ", cloud"})
              {p.model_name && <> — {p.model_name}</>}
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
