import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsPage } from "./SettingsPage";
import { tauriBridge } from "../tauriBridge";
import { acknowledgedPermissionsFor } from "../privacyLevels";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getObservationStatus: vi.fn(),
    listLlmProviders: vi.fn(),
    setPrivacyLevel: vi.fn(),
    getCloudConsent: vi.fn(),
    setCloudConsent: vi.fn(),
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
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
    mockedBridge.getCloudConsent.mockResolvedValue(false);
    mockedBridge.setCloudConsent.mockResolvedValue(true);
    mockedBridge.getSettings.mockResolvedValue(false);
    mockedBridge.updateSettings.mockResolvedValue(true);
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
    expect(mockedBridge.setPrivacyLevel).toHaveBeenCalledWith(3, acknowledgedPermissionsFor(3));
  });

  it("lowering the level calls set_privacy_level with level-1", async () => {
    const user = userEvent.setup();
    render(<SettingsPage />);
    await screen.findByText("2");

    await user.click(screen.getByRole("button", { name: "Lower" }));
    expect(mockedBridge.setPrivacyLevel).toHaveBeenCalledWith(1, acknowledgedPermissionsFor(1));
  });

  it("shows a message when no provider is configured", async () => {
    mockedBridge.listLlmProviders.mockResolvedValue([]);
    render(<SettingsPage />);
    expect(await screen.findByText("No provider configured yet.")).toBeInTheDocument();
  });

  it("does not show the cloud-consent toggle when every provider is local", async () => {
    render(<SettingsPage />);
    await screen.findByTestId("provider-list");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("shows the cloud-consent toggle when a cloud provider is configured, and toggling it calls set_cloud_consent", async () => {
    const user = userEvent.setup();
    mockedBridge.listLlmProviders.mockResolvedValue([
      {
        id: "anthropic-main",
        provider_type: "anthropic",
        is_local: false,
        model_name: "claude-sonnet-5",
        endpoint: null,
        vault_key_ref: "provider-key-anthropic-main",
        active: true,
      },
    ]);
    render(<SettingsPage />);
    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).not.toBeChecked();

    await user.click(checkbox);
    expect(mockedBridge.setCloudConsent).toHaveBeenCalledWith(true);
  });

  it("shows an error rather than silently failing when changing the level fails", async () => {
    const user = userEvent.setup();
    mockedBridge.setPrivacyLevel.mockRejectedValueOnce(new Error("policy floor violation"));
    render(<SettingsPage />);
    await screen.findByText("2");

    await user.click(screen.getByRole("button", { name: "Raise" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("policy floor violation");
  });

  it("does not show the Deep-mode screenshot+OCR toggle below Level 4", async () => {
    render(<SettingsPage />);
    await screen.findByText("2");
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("shows the Deep-mode screenshot+OCR toggle at Level 4, reflecting its current setting", async () => {
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 4,
      consented_manifest_version: 2,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    });
    mockedBridge.getSettings.mockResolvedValue(true);
    render(<SettingsPage />);
    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).toBeChecked();
    expect(mockedBridge.getSettings).toHaveBeenCalledWith("deep_mode_screenshot_ocr_enabled");
  });

  it("toggling the Deep-mode screenshot+OCR checkbox calls update_settings with the flipped value", async () => {
    const user = userEvent.setup();
    mockedBridge.getObservationStatus.mockResolvedValue({
      current_level: 4,
      consented_manifest_version: 2,
      observation_active: true,
      updated_at: "2026-07-16T00:00:00Z",
    });
    mockedBridge.getSettings.mockResolvedValue(false);
    render(<SettingsPage />);
    const checkbox = await screen.findByRole("checkbox");
    expect(checkbox).not.toBeChecked();

    await user.click(checkbox);
    expect(mockedBridge.updateSettings).toHaveBeenCalledWith(
      "deep_mode_screenshot_ocr_enabled",
      true
    );
  });
});
