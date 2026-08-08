import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RecommendationCard } from "./RecommendationCard";
import { tauriBridge, type Recommendation } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    setRecommendationStatus: vi.fn(),
    getRecommendationDetail: vi.fn(),
    snoozeRecommendation: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

// Mirrors PROMPT.md's own worked example almost verbatim — this is the
// "Hybrid workflow using Playwright + local LLM" dialogue, as data.
const sample: Recommendation = {
  id: 42,
  pattern_id: 7,
  created_at: "2026-07-16T00:00:00Z",
  title: "Copying ticket data into the weekly report",
  category: "hybrid",
  why: "This exact app sequence recurs with high regularity and low variation.",
  confidence: 0.8,
  estimated_time_saved_minutes: 660,
  difficulty: "medium",
  maintenance_burden: "low",
  privacy_implications: "Fully local, no cloud dispatch required.",
  implementation_effort: "About 2-3 hours one-time setup.",
  alternatives: [
    { approach: "Excel macro", tradeoff: "Low effort, but brittle to format changes." },
    { approach: "Python script", tradeoff: "Medium effort, medium maintenance." },
  ],
  assumptions: ["You have API access to your Jira instance."],
  ignored_information: ["3 occurrences on a different machine were not correlated."],
  generating_provider: "ollama",
  status: "suggested",
  dismissal_reason: null,
  notified_at: null,
  snoozed_until: null,
};

describe("RecommendationCard", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.getRecommendationDetail.mockResolvedValue({
      ...sample,
      contributing_events: [],
    });
  });

  it("shows the estimated time saved and category from PROMPT.md's own example shape", () => {
    render(<RecommendationCard recommendation={sample} />);
    expect(screen.getByText("Copying ticket data into the weekly report")).toBeInTheDocument();
    expect(screen.getByText(/660 minutes/)).toBeInTheDocument();
    expect(screen.getByText(/hybrid/)).toBeInTheDocument();
  });

  it("hides the full explainability detail until Why? is clicked", async () => {
    const user = userEvent.setup();
    render(<RecommendationCard recommendation={sample} />);

    expect(screen.queryByTestId("recommendation-detail")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Why?" }));

    const detail = screen.getByTestId("recommendation-detail");
    expect(detail).toBeInTheDocument();
    expect(detail).toHaveTextContent(sample.why);
    expect(detail).toHaveTextContent(sample.assumptions[0]);
    expect(detail).toHaveTextContent(sample.ignored_information[0]);
    expect(detail).toHaveTextContent("Excel macro");
    expect(detail).toHaveTextContent("Python script");
  });

  it("fetches and shows the contributing-events evidence trail when expanded", async () => {
    const user = userEvent.setup();
    mockedBridge.getRecommendationDetail.mockResolvedValue({
      ...sample,
      contributing_events: [
        {
          id: 101,
          occurred_at: "2026-07-16T10:42:03Z",
          source_id: "linux.active_window",
          signal_type: "app_focus_change",
          privacy_level_at_capture: 1,
          summary: { app: "Jira" },
          is_deep_mode: false,
          ttl_expires_at: null,
        },
      ],
    });
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Why?" }));

    expect(mockedBridge.getRecommendationDetail).toHaveBeenCalledWith(42);
    const events = await screen.findByTestId("contributing-events");
    expect(events).toHaveTextContent("linux.active_window");
    expect(events).toHaveTextContent("app_focus_change");
  });

  it("marking implemented calls set_recommendation_status with the implemented status", async () => {
    const user = userEvent.setup();
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Mark implemented" }));

    expect(mockedBridge.setRecommendationStatus).toHaveBeenCalledWith(42, "implemented");
  });

  it("dismissing calls set_recommendation_status with a dismissal reason attached", async () => {
    const user = userEvent.setup();
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Dismiss" }));

    expect(mockedBridge.setRecommendationStatus).toHaveBeenCalledWith(
      42,
      "dismissed",
      "not worth the effort"
    );
  });

  it("shows an error rather than silently failing when marking implemented fails", async () => {
    const user = userEvent.setup();
    mockedBridge.setRecommendationStatus.mockRejectedValueOnce(new Error("store unavailable"));
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Mark implemented" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("store unavailable");
  });

  it("snoozing calls snooze_recommendation with this recommendation's id", async () => {
    const user = userEvent.setup();
    mockedBridge.snoozeRecommendation.mockResolvedValue(true);
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Snooze" }));

    expect(mockedBridge.snoozeRecommendation).toHaveBeenCalledWith(42);
  });

  it("shows an error rather than silently failing when snoozing fails", async () => {
    const user = userEvent.setup();
    mockedBridge.snoozeRecommendation.mockRejectedValueOnce(new Error("store unavailable"));
    render(<RecommendationCard recommendation={sample} />);

    await user.click(screen.getByRole("button", { name: "Snooze" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("store unavailable");
  });
});
