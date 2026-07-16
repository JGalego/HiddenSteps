import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { tauriBridge } from "./tauriBridge";

vi.mock("./tauriBridge", () => ({
  tauriBridge: {
    getOnboardingState: vi.fn(),
    listRecommendations: vi.fn(),
    getObservationStatus: vi.fn(),
    getRecentEvents: vi.fn(),
    listLlmProviders: vi.fn(),
    getDiagnostics: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

describe("App", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.listRecommendations.mockResolvedValue([]);
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 1,
      consented_manifest_version: 1,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    });
    mockedBridge.getRecentEvents.mockResolvedValue([]);
    mockedBridge.listLlmProviders.mockResolvedValue([]);
    mockedBridge.getDiagnostics.mockResolvedValue({
      privacy_level: 1,
      observation_active: true,
      active_provider: null,
      event_count: 0,
      pattern_count: 0,
      recommendation_count: 0,
      audit_log_count: 0,
      storage_bytes: null,
      encryption_status: "SQLCipher (AES-256), key in OS credential vault",
    });
  });

  it("shows the onboarding wizard when onboarding is not complete", async () => {
    mockedBridge.getOnboardingState.mockResolvedValue({ completed: false });
    render(<App />);
    expect(await screen.findByTestId("onboarding-wizard")).toBeInTheDocument();
  });

  it("shows the main app with navigation once onboarding is complete", async () => {
    mockedBridge.getOnboardingState.mockResolvedValue({ completed: true });
    render(<App />);
    expect(await screen.findByRole("navigation")).toBeInTheDocument();
    expect(screen.getByLabelText("Privacy Dashboard")).toBeInTheDocument();
  });

  it("switching tabs never re-triggers onboarding", async () => {
    mockedBridge.getOnboardingState.mockResolvedValue({ completed: true });
    const user = userEvent.setup();
    render(<App />);
    await screen.findByRole("navigation");

    await user.click(screen.getByRole("button", { name: "settings" }));
    expect(screen.getByLabelText("Settings")).toBeInTheDocument();
    expect(mockedBridge.getOnboardingState).toHaveBeenCalledOnce();

    await user.click(screen.getByRole("button", { name: "diagnostics" }));
    expect(screen.getByLabelText("Diagnostics")).toBeInTheDocument();
  });
});
