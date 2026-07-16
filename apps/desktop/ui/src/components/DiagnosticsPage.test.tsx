import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { DiagnosticsPage } from "./DiagnosticsPage";
import { tauriBridge } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getDiagnostics: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

describe("DiagnosticsPage", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("renders real counts and storage size from get_diagnostics, not placeholders", async () => {
    mockedBridge.getDiagnostics.mockResolvedValue({
      privacy_level: 2,
      observation_active: true,
      active_provider: {
        id: "ollama-local",
        provider_type: "ollama",
        is_local: true,
        model_name: "qwen3:0.6b",
        endpoint: "http://localhost:11434",
        vault_key_ref: null,
        active: true,
      },
      event_count: 142,
      pattern_count: 3,
      recommendation_count: 1,
      audit_log_count: 9,
      storage_bytes: 2_097_152,
      encryption_status: "SQLCipher (AES-256), key in OS credential vault",
    });

    render(<DiagnosticsPage />);

    expect(await screen.findByText("142")).toBeInTheDocument();
    expect(screen.getByText("2.0 MB")).toBeInTheDocument();
    expect(screen.getByTestId("diag-provider")).toHaveTextContent("ollama-local (local)");
    expect(screen.getByText("SQLCipher (AES-256), key in OS credential vault")).toBeInTheDocument();
  });

  it("shows 'None configured' rather than fabricating a provider", async () => {
    mockedBridge.getDiagnostics.mockResolvedValue({
      privacy_level: 0,
      observation_active: false,
      active_provider: null,
      event_count: 0,
      pattern_count: 0,
      recommendation_count: 0,
      audit_log_count: 0,
      storage_bytes: null,
      encryption_status: "SQLCipher (AES-256), key in OS credential vault",
    });

    render(<DiagnosticsPage />);
    expect(await screen.findByTestId("diag-provider")).toHaveTextContent("None configured");
    expect(screen.getByText("unknown")).toBeInTheDocument();
  });

  it("surfaces a real error rather than silently showing empty diagnostics", async () => {
    mockedBridge.getDiagnostics.mockRejectedValue(new Error("store unavailable"));
    render(<DiagnosticsPage />);
    expect(await screen.findByRole("alert")).toHaveTextContent("store unavailable");
  });
});
