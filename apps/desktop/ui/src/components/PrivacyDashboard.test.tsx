import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PrivacyDashboard } from "./PrivacyDashboard";
import { tauriBridge } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getObservationStatus: vi.fn(),
    getRecentEvents: vi.fn(),
    pauseObservation: vi.fn(),
    resumeObservation: vi.fn(),
    deleteAllData: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

describe("PrivacyDashboard", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 1,
      consented_manifest_version: 1,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    });
    mockedBridge.getRecentEvents.mockResolvedValue([
      {
        id: 1,
        occurred_at: "2026-07-16T10:42:03Z",
        source_id: "linux.active_window",
        signal_type: "app_focus_change",
        privacy_level_at_capture: 1,
        summary: { app: "VS Code" },
        is_deep_mode: false,
        ttl_expires_at: null,
      },
    ]);
  });

  it("shows the observing status and current level", async () => {
    render(<PrivacyDashboard />);
    await waitFor(() => {
      expect(screen.getByTestId("status-indicator")).toHaveTextContent("● Observing");
    });
    expect(screen.getByTestId("status-line")).toHaveTextContent("App awareness");
  });

  it("renders exactly what get_recent_events returns, with no added or missing rows", async () => {
    render(<PrivacyDashboard />);
    const list = await screen.findByTestId("recent-events");
    const items = list.querySelectorAll("li");
    expect(items).toHaveLength(1);
    expect(items[0].textContent).toContain("linux.active_window");
    expect(items[0].textContent).toContain("app_focus_change");
  });

  it("pausing calls pause_observation and refreshes status", async () => {
    const user = userEvent.setup();
    mockedBridge.pauseObservation.mockResolvedValue(false);
    render(<PrivacyDashboard />);

    const pauseButton = await screen.findByRole("button", { name: "Pause" });
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 1,
      consented_manifest_version: 1,
      observation_active: false,
      updated_at: "2026-07-16T00:01:00Z",
    });
    await user.click(pauseButton);

    expect(mockedBridge.pauseObservation).toHaveBeenCalledOnce();
    await waitFor(() => {
      expect(screen.getByTestId("status-indicator")).toHaveTextContent("○ Paused");
    });
  });

  it("delete-all requires a real confirmation click, not the first click", async () => {
    const user = userEvent.setup();
    render(<PrivacyDashboard />);

    const deleteButton = await screen.findByRole("button", { name: "Delete all data" });
    await user.click(deleteButton);

    // The irreversible action must not have happened yet — only the
    // confirmation dialog should have appeared.
    expect(mockedBridge.deleteAllData).not.toHaveBeenCalled();
    expect(
      screen.getByRole("alertdialog", { name: "Delete all HiddenSteps data?" })
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete everything" }));
    expect(mockedBridge.deleteAllData).toHaveBeenCalledOnce();
  });

  it("cancelling the delete confirmation never calls delete_all_data", async () => {
    const user = userEvent.setup();
    render(<PrivacyDashboard />);

    await user.click(await screen.findByRole("button", { name: "Delete all data" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));

    expect(mockedBridge.deleteAllData).not.toHaveBeenCalled();
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});
