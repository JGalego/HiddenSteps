import { useCallback, useEffect, useState } from "react";
import {
  tauriBridge,
  type BrowserBridgeStatus,
  type LlmProviderConfig,
  type PrivacyState,
} from "../tauriBridge";
import { acknowledgedPermissionsFor } from "../privacyLevels";

// The settings-table key `observation_loop`'s screenshot+OCR gate reads every
// tick (`commands::DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY` on the Rust side) —
// kept in sync with that constant's literal value by hand, the same way this
// file already has no compile-time link to any other Rust constant it calls
// by string (e.g. `set_privacy_level`'s command name itself).
const DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY = "deep_mode_screenshot_ocr_enabled";

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
  const [cloudConsent, setCloudConsentState] = useState(false);
  const [deepModeScreenshotOcr, setDeepModeScreenshotOcr] = useState(false);
  const [browserBridge, setBrowserBridge] = useState<BrowserBridgeStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextStatus, nextProviders, nextCloudConsent, nextDeepModeSetting, nextBrowserBridge] =
        await Promise.all([
          tauriBridge.getObservationStatus(),
          tauriBridge.listLlmProviders(),
          tauriBridge.getCloudConsent(),
          tauriBridge.getSettings(DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY),
          tauriBridge.getBrowserBridgeStatus(),
        ]);
      setStatus(nextStatus);
      setProviders(nextProviders);
      setCloudConsentState(nextCloudConsent);
      setDeepModeScreenshotOcr(nextDeepModeSetting === true);
      setBrowserBridge(nextBrowserBridge);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const changeLevel = async (level: number) => {
    try {
      await tauriBridge.setPrivacyLevel(level, acknowledgedPermissionsFor(level));
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleCloudConsent = async () => {
    try {
      await tauriBridge.setCloudConsent(!cloudConsent);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleDeepModeScreenshotOcr = async () => {
    try {
      await tauriBridge.updateSettings(
        DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY,
        !deepModeScreenshotOcr
      );
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const copyBridgeToken = async () => {
    if (!browserBridge) return;
    try {
      await navigator.clipboard.writeText(browserBridge.token);
    } catch {
      // Clipboard access can be denied/unavailable (permissions, a
      // non-secure context, a test environment) — the token is still shown
      // in the page for manual copy/paste, so a failed clipboard write isn't
      // worth surfacing as an error banner.
    }
  };

  const hasCloudProvider = providers.some((p) => !p.is_local);
  const isMaximumAssistance = status?.current_level === 4;

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
        {isMaximumAssistance && (
          <p className="deep-mode-screenshot-ocr-toggle">
            <label>
              <input
                type="checkbox"
                checked={deepModeScreenshotOcr}
                onChange={toggleDeepModeScreenshotOcr}
              />{" "}
              Capture screenshots and read on-screen text (OCR). This is
              Level 4's own separate opt-in — turning it off stops screenshot
              capture immediately, even while Level 4 stays selected.
            </label>
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
        {hasCloudProvider && (
          <p className="cloud-consent-toggle">
            <label>
              <input
                type="checkbox"
                checked={cloudConsent}
                onChange={toggleCloudConsent}
              />{" "}
              Allow sending pattern summaries to your cloud AI provider for
              recommendations. Without this, recommendations are only
              generated using a local provider.
            </label>
          </p>
        )}
      </div>

      <div className="card section-block">
        <h2>Browser Extension</h2>
        {browserBridge && (
          <>
            <p data-testid="browser-bridge-status">
              {browserBridge.receiving_data
                ? "Receiving browser activity from the extension."
                : "Not yet receiving data from the extension."}
            </p>
            <p>
              Install the HiddenSteps Companion extension (Chrome/Chromium),
              then paste this pairing token into its options page:
            </p>
            <p>
              <code data-testid="browser-bridge-token">{browserBridge.token}</code>{" "}
              <button className="btn" type="button" onClick={copyBridgeToken}>
                Copy
              </button>
            </p>
            <p>
              Bridge port: <strong>{browserBridge.port}</strong>. Browser
              domain reporting requires Privacy Level 2 or above; page titles
              require Level 3 or above.
            </p>
          </>
        )}
      </div>
    </section>
  );
}
