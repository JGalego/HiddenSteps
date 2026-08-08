import { fireEvent, render, screen } from "@testing-library/react";
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
    getBrowserBridgeStatus: vi.fn(),
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
    mockedBridge.getBrowserBridgeStatus.mockResolvedValue({
      token: "abc123token",
      port: 49231,
      last_seen: null,
      receiving_data: false,
    });
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

  it("shows the browser bridge pairing token and port, and a not-yet-receiving status", async () => {
    render(<SettingsPage />);
    expect(await screen.findByTestId("browser-bridge-token")).toHaveTextContent("abc123token");
    expect(screen.getByTestId("browser-bridge-status")).toHaveTextContent(
      "Not yet receiving data from the extension."
    );
    expect(screen.getByText("49231")).toBeInTheDocument();
  });

  it("shows a receiving-data status once the bridge has recorded recent activity", async () => {
    mockedBridge.getBrowserBridgeStatus.mockResolvedValue({
      token: "abc123token",
      port: 49231,
      last_seen: "2026-07-16T00:00:00Z",
      receiving_data: true,
    });
    render(<SettingsPage />);
    expect(await screen.findByTestId("browser-bridge-status")).toHaveTextContent(
      "Receiving browser activity from the extension."
    );
  });

  it("copies the pairing token to the clipboard when Copy is clicked", async () => {
    // `userEvent.setup()` installs its own `navigator.clipboard` stub
    // (Testing Library's built-in clipboard support) — spy on *that* rather
    // than replacing it beforehand, since setup() would otherwise clobber a
    // pre-installed replacement.
    const user = userEvent.setup();
    const writeText = vi.spyOn(navigator.clipboard, "writeText").mockResolvedValue(undefined);
    render(<SettingsPage />);
    await screen.findByTestId("browser-bridge-token");

    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(writeText).toHaveBeenCalledWith("abc123token");
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

  it("shows a default quiet-hours window when none is stored yet", async () => {
    render(<SettingsPage />);
    await screen.findByText("2");

    expect(screen.getByLabelText("Quiet hours start")).toHaveValue(22);
    expect(screen.getByLabelText("Quiet hours end")).toHaveValue(7);
  });

  it("shows the stored quiet-hours window once loaded", async () => {
    mockedBridge.getSettings.mockImplementation((key: string) =>
      key === "notification_quiet_hours"
        ? Promise.resolve({ start_hour: 20, end_hour: 6 })
        : Promise.resolve(false)
    );
    render(<SettingsPage />);
    await screen.findByText("2");

    expect(await screen.findByLabelText("Quiet hours start")).toHaveValue(20);
    expect(screen.getByLabelText("Quiet hours end")).toHaveValue(6);
  });

  it("changing the quiet-hours start hour calls update_settings with the new range", async () => {
    mockedBridge.getSettings.mockImplementation((key: string) =>
      key === "notification_quiet_hours"
        ? Promise.resolve({ start_hour: 22, end_hour: 7 })
        : Promise.resolve(false)
    );
    render(<SettingsPage />);
    const startInput = await screen.findByLabelText("Quiet hours start");

    // A single synchronous `fireEvent.change` rather than `userEvent.type`'s
    // keystroke-by-keystroke input: this component's inputs are controlled by
    // state that only updates once the async update_settings + refresh round
    // trip resolves, so typing digit-by-digit against a `value` prop that
    // doesn't move in between is exactly the kind of race a real user typing
    // quickly could also hit — not what this test is trying to exercise.
    fireEvent.change(startInput, { target: { value: "23" } });

    expect(mockedBridge.updateSettings).toHaveBeenCalledWith("notification_quiet_hours", {
      start_hour: 23,
      end_hour: 7,
    });
  });
});
