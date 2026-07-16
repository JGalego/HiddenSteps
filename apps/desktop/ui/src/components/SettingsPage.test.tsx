import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";
import { tauriBridge } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getObservationStatus: vi.fn(),
    listLlmProviders: vi.fn(),
    setPrivacyLevel: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 2,
      consented_manifest_version: 1,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    });
    mockedBridge.listLlmProviders.mockResolvedValue([
      {
        id: "ollama-local",
        provider_type: "ollama",
        is_local: true,
        model_name: "qwen3:0.6b",
        endpoint: "http://localhost:11434",
        vault_key_ref: null,
        active: true,
      },
    ]);
    mockedBridge.setPrivacyLevel.mockResolvedValue({ effective_level: 2 });
  });

  it("shows the current privacy level and the active provider", async () => {
    render(<SettingsPage />);
    expect(await screen.findByText("2")).toBeInTheDocument();
    const list = await screen.findByTestId("provider-list");
    expect(list).toHaveTextContent("ollama-local");
    expect(list).toHaveTextContent("qwen3:0.6b");
  });

  it("raising the level calls set_privacy_level with level+1", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);
    await screen.findByText("2");

    await user.click(screen.getByRole("button", { name: "Raise" }));
    expect(mockedBridge.setPrivacyLevel).toHaveBeenCalledWith(3, ["acknowledged"]);
  });

  it("lowering the level calls set_privacy_level with level-1", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);
    await screen.findByText("2");

    await user.click(screen.getByRole("button", { name: "Lower" }));
    expect(mockedBridge.setPrivacyLevel).toHaveBeenCalledWith(1, ["acknowledged"]);
  });

  it("shows a message when no provider is configured", async () => {
    mockedBridge.listLlmProviders.mockResolvedValue([]);
    render(<SettingsPage />);
    expect(await screen.findByText("No provider configured yet.")).toBeInTheDocument();
  });
});
