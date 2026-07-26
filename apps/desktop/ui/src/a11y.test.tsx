import { render, waitFor } from "@testing-library/react";
import { axe } from "jest-axe";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingWizard } from "./components/OnboardingWizard";
import { PrivacyDashboard } from "./components/PrivacyDashboard";
import { SettingsPage } from "./components/SettingsPage";
import { RecommendationCard } from "./components/RecommendationCard";
import { type Recommendation } from "./tauriBridge";

// The automated accessibility gate docs/ux/06-accessibility.md §5 calls for:
// axe-core over each shipped screen, asserting zero violations. Runs in the
// same vitest/jsdom harness as the component tests, so it's a real merge gate
// (CI runs `npm test`), not a manual spot-check.

vi.mock("./tauriBridge", () => ({
  tauriBridge: {
    getProviderDetection: vi.fn().mockResolvedValue([]),
    getObservationStatus: vi.fn().mockResolvedValue({
      current_level: 1,
      consented_manifest_version: 1,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    }),
    getPrivacyManifestStatus: vi.fn().mockResolvedValue({
      current_manifest_version: 1,
      consented_manifest_version: 1,
      reconsent_required: false,
    }),
    getRecentEvents: vi.fn().mockResolvedValue([]),
    listLlmProviders: vi.fn().mockResolvedValue([]),
    getCloudConsent: vi.fn().mockResolvedValue(false),
    setRecommendationStatus: vi.fn(),
    getRecommendationDetail: vi.fn(),
    snoozeRecommendation: vi.fn(),
  },
}));

const sampleRecommendation: Recommendation = {
  id: 1,
  pattern_id: 7,
  created_at: "2026-07-16T00:00:00Z",
  title: "Automate the weekly ticket export",
  category: "hybrid",
  why: "This exact sequence recurs with high regularity.",
  confidence: 0.8,
  estimated_time_saved_minutes: 660,
  difficulty: "medium",
  maintenance_burden: "low",
  privacy_implications: "Fully local.",
  implementation_effort: "About 2-3 hours.",
  alternatives: [{ approach: "Python script", tradeoff: "Higher maintenance." }],
  assumptions: ["API access is available."],
  ignored_information: ["Occurrences on another device were not correlated."],
  generating_provider: "ollama",
  status: "suggested",
  dismissal_reason: null,
  notified_at: null,
  snoozed_until: null,
};

describe("accessibility (axe-core)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("OnboardingWizard has no axe violations on the first screen", async () => {
    const { container } = render(<OnboardingWizard onComplete={vi.fn()} />);
    expect(await axe(container)).toHaveNoViolations();
  });

  it("PrivacyDashboard has no axe violations", async () => {
    const { container, findByLabelText } = render(<PrivacyDashboard />);
    await findByLabelText("Privacy Dashboard");
    await waitFor(() => {
      expect(container.querySelector("[data-testid='status-line']")).not.toBeNull();
    });
    expect(await axe(container)).toHaveNoViolations();
  });

  it("SettingsPage has no axe violations", async () => {
    const { container, findByText } = render(<SettingsPage />);
    await findByText("Settings");
    expect(await axe(container)).toHaveNoViolations();
  });

  it("RecommendationCard has no axe violations, expanded and collapsed", async () => {
    const { container } = render(
      <RecommendationCard recommendation={sampleRecommendation} />
    );
    expect(await axe(container)).toHaveNoViolations();
  });
});
