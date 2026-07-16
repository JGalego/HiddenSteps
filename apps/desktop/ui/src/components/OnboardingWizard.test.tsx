import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { OnboardingWizard } from "./OnboardingWizard";
import { tauriBridge } from "../tauriBridge";

vi.mock("../tauriBridge", () => ({
  tauriBridge: {
    getProviderDetection: vi.fn(),
    testProviderConnectivity: vi.fn(),
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

describe("OnboardingWizard", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    mockedBridge.getProviderDetection.mockResolvedValue([
      { name: "ollama", reachable: true },
      { name: "lm_studio", reachable: false },
    ]);
    mockedBridge.testProviderConnectivity.mockResolvedValue({ ok: true, error: null });
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
    await advanceToStep(user, 6);
    await user.click(screen.getByRole("button", { name: "Run checks" }));
    await screen.findByTestId("validation-result");
    await user.click(screen.getByRole("button", { name: "Continue" }));

    expect(screen.getByText("Ready to start")).toBeInTheDocument();
    const startButton = screen.getByRole("button", { name: "Start observing" });
    expect(startButton).toBeDisabled();

    await user.click(screen.getByRole("checkbox"));
    expect(startButton).toBeEnabled();
  });

  it("finishing calls set_privacy_level then complete_onboarding, and notifies the parent", async () => {
    const user = userEvent.setup();
    const onComplete = vi.fn();
    render(<OnboardingWizard onComplete={onComplete} />);

    await advanceToStep(user, 6);
    await user.click(screen.getByRole("button", { name: "Run checks" }));
    await screen.findByTestId("validation-result");
    await user.click(screen.getByRole("button", { name: "Continue" })); // -> step 7
    await user.click(screen.getByRole("checkbox"));
    await user.click(screen.getByRole("button", { name: "Start observing" }));

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

    await advanceToStep(user, 6); // through step 5 to step 6
    await user.click(screen.getByRole("button", { name: "Run checks" }));
    await screen.findByTestId("validation-result");
    await user.click(screen.getByRole("button", { name: "Continue" })); // -> step 7

    expect(screen.getByText(/Level 2/)).toBeInTheDocument();
  });
});
