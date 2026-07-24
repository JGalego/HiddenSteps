import { useCallback, useEffect, useState } from "react";
import { DiagnosticsPage } from "./components/DiagnosticsPage";
import { OnboardingWizard } from "./components/OnboardingWizard";
import { PrivacyDashboard } from "./components/PrivacyDashboard";
import { RecommendationCard } from "./components/RecommendationCard";
import { SettingsPage } from "./components/SettingsPage";
import { tauriBridge, type Recommendation } from "./tauriBridge";

type Tab = "dashboard" | "recommendations" | "settings" | "diagnostics";

/**
 * Per FR-17: no observation starts, and no other screen renders, before
 * onboarding completes — `get_onboarding_state` is checked before anything
 * else mounts. Once complete, the four screens built and tested in this
 * milestone (dashboard, recommendations, settings, diagnostics) are reachable
 * via simple tab state — full navigation chrome is a disclosed gap, not this
 * component's job to fake with more polish than what's actually here.
 */
export function App() {
  const [onboardingComplete, setOnboardingComplete] = useState<boolean | null>(null);
  const [onboardingError, setOnboardingError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("dashboard");
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);
  const [recommendationsError, setRecommendationsError] = useState<string | null>(null);

  const checkOnboardingState = useCallback(() => {
    setOnboardingError(null);
    tauriBridge
      .getOnboardingState()
      .then((state) => setOnboardingComplete(state.completed))
      .catch((e) => {
        // Without this, a rejected first IPC call left onboardingComplete
        // stuck at null forever — the whole app frozen on the loading
        // spinner below with no way out.
        setOnboardingError(String(e));
      });
  }, []);

  useEffect(() => {
    checkOnboardingState();
  }, [checkOnboardingState]);

  const refreshRecommendations = useCallback(async () => {
    try {
      setRecommendations(await tauriBridge.listRecommendations());
      setRecommendationsError(null);
    } catch (e) {
      setRecommendationsError(String(e));
    }
  }, []);

  useEffect(() => {
    if (onboardingComplete) {
      refreshRecommendations();
    }
  }, [onboardingComplete, refreshRecommendations]);

  if (onboardingError) {
    return (
      <div className="app-loading">
        <p className="alert" role="alert">
          {onboardingError}
        </p>
        <button className="btn" type="button" onClick={checkOnboardingState}>
          Retry
        </button>
      </div>
    );
  }

  if (onboardingComplete === null) {
    return <p className="app-loading">Loading…</p>;
  }

  if (!onboardingComplete) {
    return <OnboardingWizard onComplete={() => setOnboardingComplete(true)} />;
  }

  return (
    <main className="app-shell">
      <nav className="main-nav" aria-label="Main navigation">
        {(["dashboard", "recommendations", "settings", "diagnostics"] as Tab[]).map((t) => (
          <button
            key={t}
            type="button"
            aria-current={tab === t}
            onClick={() => setTab(t)}
          >
            {t}
          </button>
        ))}
      </nav>

      {tab === "dashboard" && <PrivacyDashboard />}

      {tab === "recommendations" && (
        <section aria-label="Recommendations">
          <h1>Recommendations</h1>
          {recommendationsError && (
            <p className="alert" role="alert">
              {recommendationsError}
            </p>
          )}
          {/* aria-live="polite" per docs/ux/06-accessibility.md §1: a new
              recommendation is not an emergency and should not seize focus or
              interrupt a screen-reader user — it's announced, not forced. */}
          <div aria-live="polite">
            {recommendations.length === 0 && (
              <p>Still learning your patterns. Nothing has repeated often enough yet to suggest a change.</p>
            )}
            {recommendations.map((rec) => (
              <RecommendationCard
                key={rec.id}
                recommendation={rec}
                onStatusChange={refreshRecommendations}
              />
            ))}
          </div>
        </section>
      )}

      {tab === "settings" && <SettingsPage />}
      {tab === "diagnostics" && <DiagnosticsPage />}
    </main>
  );
}
