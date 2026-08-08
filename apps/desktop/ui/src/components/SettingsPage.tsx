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

// Kept in sync by hand with `recommendation_loop::NOTIFICATION_QUIET_HOURS_SETTING_KEY`
// the same way the key above is kept in sync with its own Rust constant.
// Value shape: `{ start_hour, end_hour }`, both 0-23, compared against the
// current hour in UTC (see that Rust constant's doc comment for why UTC, not
// wall-clock-local time) — recommendation_loop's notification sweep reads
// this every tick and simply doesn't send a notification while the current
// hour falls in this window, without losing or dropping the recommendation:
// the very next sweep after quiet hours end sends it.
const NOTIFICATION_QUIET_HOURS_SETTING_KEY = "notification_quiet_hours";

interface QuietHours {
  start_hour: number;
  end_hour: number;
}

function isQuietHours(value: unknown): value is QuietHours {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as QuietHours).start_hour === "number" &&
    typeof (value as QuietHours).end_hour === "number"
  );
}

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
  const [quietHours, setQuietHoursState] = useState<QuietHours | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [
        nextStatus,
        nextProviders,
        nextCloudConsent,
        nextDeepModeSetting,
        nextBrowserBridge,
        nextQuietHours,
      ] = await Promise.all([
        tauriBridge.getObservationStatus(),
        tauriBridge.listLlmProviders(),
        tauriBridge.getCloudConsent(),
        tauriBridge.getSettings(DEEP_MODE_SCREENSHOT_OCR_SETTING_KEY),
        tauriBridge.getBrowserBridgeStatus(),
        tauriBridge.getSettings(NOTIFICATION_QUIET_HOURS_SETTING_KEY),
      ]);
      setStatus(nextStatus);
      setProviders(nextProviders);
      setCloudConsentState(nextCloudConsent);
      setDeepModeScreenshotOcr(nextDeepModeSetting === true);
      setBrowserBridge(nextBrowserBridge);
      setQuietHoursState(isQuietHours(nextQuietHours) ? nextQuietHours : null);
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

  const updateQuietHours = async (startHour: number, endHour: number) => {
    try {
      await tauriBridge.updateSettings(NOTIFICATION_QUIET_HOURS_SETTING_KEY, {
        start_hour: startHour,
        end_hour: endHour,
      });
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

      <div className="card section-block">
        <h2>Notifications</h2>
        <p>
          Quiet hours (UTC): a new recommendation still gets generated as
          usual during this window, but its OS notification is delayed until
          quiet hours end rather than sent — nothing is lost, just deferred.
          Set both hours the same to disable quiet hours entirely.
        </p>
        <p>
          <label>
            Start hour{" "}
            <input
              type="number"
              min={0}
              max={23}
              aria-label="Quiet hours start"
              value={quietHours?.start_hour ?? 22}
              onChange={(e) =>
                updateQuietHours(Number(e.target.value), quietHours?.end_hour ?? 7)
              }
            />
          </label>{" "}
          <label>
            End hour{" "}
            <input
              type="number"
              min={0}
              max={23}
              aria-label="Quiet hours end"
              value={quietHours?.end_hour ?? 7}
              onChange={(e) =>
                updateQuietHours(quietHours?.start_hour ?? 22, Number(e.target.value))
              }
            />
          </label>
        </p>
      </div>
    </section>
  );
}
