import { useCallback, useEffect, useState } from "react";
import { PrivacyDashboard } from "./components/PrivacyDashboard";
import { RecommendationCard } from "./components/RecommendationCard";
import { tauriBridge, type Recommendation } from "./tauriBridge";

/**
 * Minimal shell tying the dashboard and recommendations views together.
 * Onboarding (docs/ux/02-onboarding-flow.md), Settings
 * (docs/ux/05-settings-and-complexity-tiers.md), and full navigation are not
 * implemented here — this covers the two components this milestone built and
 * tested (the dashboard and the recommendation card), not the full app shell.
 */
export function App() {
  const [recommendations, setRecommendations] = useState<Recommendation[]>([]);

  const refreshRecommendations = useCallback(async () => {
    setRecommendations(await tauriBridge.listRecommendations());
  }, []);

  useEffect(() => {
    refreshRecommendations();
  }, [refreshRecommendations]);

  return (
    <main>
      <PrivacyDashboard />
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
    </main>
  );
}
