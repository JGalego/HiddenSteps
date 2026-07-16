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
  const [tab, setTab] = useState<Tab>("dashboard");
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);

  useEffect(() => {
    tauriBridge.getOnboardingState().then((state) => setOnboardingComplete(state.completed));
  }, []);

  const refreshRecommendations = useCallback(async () => {
    setRecommendations(await tauriBridge.listRecommendations());
  }, []);

  useEffect(() => {
    if (onboardingComplete) {
      refreshRecommendations();
    }
  }, [onboardingComplete, refreshRecommendations]);

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
        </section>
      )}

      {tab === "settings" && <SettingsPage />}
      {tab === "diagnostics" && <DiagnosticsPage />}
    </main>
  );
}
