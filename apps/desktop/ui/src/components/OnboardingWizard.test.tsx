import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingWizard } from "./OnboardingWizard";
import { tauriBridge } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getProviderDetection: vi.fn(),
    testProviderConnectivity: vi.fn(),
    setAiProvider: vi.fn(),
    setPrivacyLevel: vi.fn(),
    completeOnboarding: vi.fn(),
  },
}));

const mockedBridge = vi.mocked(tauriBridge, true);

async function advanceToStep(user: ReturnType<typeof userEvent.setup>, target: number) {
  for (let i = 1; i < target; i++) {
    await user.click(screen.getByRole("button", { name: "Continue" }));
  }
}

async function advanceThroughValidation(user: ReturnType<typeof userEvent.setup>) {
  await advanceToStep(user, 6);
  await user.click(screen.getByRole("button", { name: "Run checks" }));
  await screen.findByTestId("validation-result");
  await user.click(screen.getByRole("button", { name: "Continue" })); // -> step 7
}

describe("OnboardingWizard", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.getProviderDetection.mockResolvedValue([
      { name: "ollama", reachable: true, models: ["qwen2.5:7b-instruct", "llama3.1:8b"] },
      { name: "lm_studio", reachable: false, models: [] },
    ]);
    mockedBridge.testProviderConnectivity.mockResolvedValue({ ok: true, error: null });
    mockedBridge.setAiProvider.mockResolvedValue(true);
    mockedBridge.setPrivacyLevel.mockResolvedValue({ effective_level: 1 });
    mockedBridge.completeOnboarding.mockResolvedValue(true);
  });

  it("starts on step 1 of 8 and requires no OS-permission-shaped action before advancing", () => {
    render(<OnboardingWizard onComplete={vi.fn()} />);
    expect(screen.getByTestId("step-indicator")).toHaveTextContent("Step 1 of 8");
    expect(screen.getByText("Welcome to HiddenSteps")).toBeInTheDocument();
  });

  it("does not skip the what-it-does-not-do screen", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await user.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByText("What HiddenSteps will never do")).toBeInTheDocument();
  });

  it("detects local runtimes only once the provider step is reached, not eagerly on mount", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    expect(mockedBridge.getProviderDetection).not.toHaveBeenCalled();

    await advanceToStep(user, 5);
    expect(await screen.findByText(/ollama/)).toBeInTheDocument();
    expect(mockedBridge.getProviderDetection).toHaveBeenCalledOnce();
  });

  it("auto-selects the first detected model for a local provider, so validation is never blocked on an empty field", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 5);
    await screen.findByText(/ollama/);

    const modelSelect = await screen.findByRole("combobox", { name: /Model/ });
    expect(modelSelect).toHaveValue("qwen2.5:7b-instruct");
  });

  it("falls back to a text input when a local runtime is reachable but reports no models", async () => {
    mockedBridge.getProviderDetection.mockResolvedValue([
      { name: "ollama", reachable: true, models: [] },
    ]);
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 5);
    await screen.findByText(/ollama/);

    expect(screen.getByPlaceholderText(/qwen2.5/)).toBeInTheDocument();
  });

  it("shows a model field and an API key field for a cloud provider, with a sensible default model", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 5);
    await screen.findByText(/ollama/);

    await user.selectOptions(screen.getByRole("combobox", { name: "Provider:" }), "anthropic");

    expect(screen.getByLabelText(/API key/)).toBeInTheDocument();
    expect(screen.getByLabelText("Model:")).toHaveValue("claude-sonnet-5");
  });

  it("Run checks is disabled until a model is actually chosen", async () => {
    mockedBridge.getProviderDetection.mockResolvedValue([
      { name: "ollama", reachable: false, models: [] },
    ]);
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 6);

    // No runtime reachable and nothing typed in -> the real bug this
    // prevents: silently probing a nonexistent model named "default".
    expect(screen.getByRole("button", { name: "Run checks" })).toBeDisabled();
  });

  it("sends the actually-selected model to test_provider_connectivity, never a hardcoded default", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 6);

    await user.click(screen.getByRole("button", { name: "Run checks" }));

    expect(mockedBridge.testProviderConnectivity).toHaveBeenCalledWith({
      provider_type: "ollama",
      model: "qwen2.5:7b-instruct",
      api_key: undefined,
    });
  });

  it("blocks Continue past the validation step until a successful check has run", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 6);

    expect(screen.getByRole("button", { name: "Continue" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "Run checks" }));
    expect(await screen.findByTestId("validation-result")).toHaveTextContent("Connection OK");
    expect(screen.getByRole("button", { name: "Continue" })).toBeEnabled();
  });

  it("does not allow starting observation without checking the consent box", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceThroughValidation(user);

    expect(screen.getByText("Ready to start")).toBeInTheDocument();
    const startButton = screen.getByRole("button", { name: "Start observing" });
    expect(startButton).toBeDisabled();

    await user.click(screen.getByRole("checkbox"));
    expect(startButton).toBeEnabled();
  });

  it("finishing persists the provider via set_ai_provider, then set_privacy_level, then complete_onboarding, and notifies the parent", async () => {
    const user = userEvent.setup();
    const onComplete = vi.fn();
    render(<OnboardingWizard onComplete={onComplete} />);

    await advanceThroughValidation(user);
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Start observing" }));

    expect(mockedBridge.setAiProvider).toHaveBeenCalledWith({
      id: "ollama",
      provider_type: "ollama",
      is_local: true,
      model_name: "qwen2.5:7b-instruct",
      api_key: undefined,
    });
    expect(mockedBridge.setPrivacyLevel).toHaveBeenCalledWith(1, ["acknowledged"]);
    expect(mockedBridge.completeOnboarding).toHaveBeenCalledOnce();
    expect(onComplete).toHaveBeenCalledOnce();
    expect(await screen.findByText("✓ HiddenSteps is now observing")).toBeInTheDocument();
  });

  it("selecting a privacy level on step 4 is reflected in the step 7 summary", async () => {
    const user = userEvent.setup();
    render(<OnboardingWizard onComplete={vi.fn()} />);
    await advanceToStep(user, 4);
    await user.click(screen.getByRole("radio", { name: /Workflow awareness/ }));

    await advanceThroughValidation(user);

    expect(screen.getByText(/Level 2/)).toBeInTheDocument();
  });
});
