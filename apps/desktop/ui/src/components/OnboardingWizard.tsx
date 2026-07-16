import { useEffect, useState } from "react";
import { tauriBridge, type DetectedRuntime } from "../tauriBridge";

const TOTAL_STEPS = 8;

const LEVEL_OPTIONS: Array<{ level: number; label: string; description: string }> = [
  { level: 0, label: "Manual", description: "I'll add data myself. No observation at all." },
  {
    level: 1,
    label: "App awareness (recommended to start)",
    description: "Which apps you use, and when.",
  },
  {
    level: 2,
    label: "Workflow awareness",
    description: "+ browser domains, clipboard activity type, file operations — never content.",
  },
  { level: 3, label: "Context-aware", description: "+ fuller in-app and browser context." },
  {
    level: 4,
    label: "Maximum assistance",
    description: "+ optional screen text reading (OCR), heavily filtered. Off unless you turn it on here.",
  },
];

/**
 * The 8-screen first-run flow from docs/ux/02-onboarding-flow.md, in the
 * mandated order (FR-17) — see that doc for why the order itself, not just
 * the content, is a requirement: no OS permission is requested before its
 * screen, and "Start observing" is a real, separate affirmative action, not
 * reachable by advancing through the earlier "Continue" clicks alone.
 */
export function OnboardingWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(1);
  const [privacyLevel, setPrivacyLevel] = useState(1);
  const [runtimes, setRuntimes] = useState<DetectedRuntime[] | null>(null);
  const [providerType, setProviderType] = useState("ollama");
  const [validation, setValidation] = useState<{ checked: boolean; ok: boolean; error: string | null }>({
    checked: false,
    ok: false,
    error: null,
  });
  const [consented, setConsented] = useState(false);
  const [starting, setStarting] = useState(false);

  useEffect(() => {
    if (step === 5 && runtimes === null) {
      tauriBridge.getProviderDetection().then(setRuntimes);
    }
  }, [step, runtimes]);

  const next = () => setStep((s) => Math.min(TOTAL_STEPS, s + 1));
  const back = () => setStep((s) => Math.max(1, s - 1));

  const runValidation = async () => {
    const result = await tauriBridge.testProviderConnectivity({ provider_type: providerType });
    setValidation({ checked: true, ok: result.ok, error: result.error });
  };

  const startObserving = async () => {
    setStarting(true);
    await tauriBridge.setPrivacyLevel(privacyLevel, ["acknowledged"]);
    await tauriBridge.completeOnboarding();
    setStarting(false);
    next(); // to the confirmation screen
    onComplete();
  };

  return (
    <section aria-label="HiddenSteps setup" data-testid="onboarding-wizard">
      <p data-testid="step-indicator">
        Step {step} of {TOTAL_STEPS}
      </p>

      {step === 1 && (
        <div>
          <h1>Welcome to HiddenSteps</h1>
          <p>
            HiddenSteps learns how you work, over time, and shows you specific ways to work less
            hard at the repetitive parts.
          </p>
          <p>It never acts on your behalf. It only observes and suggests. You decide everything.</p>
          <button type="button" onClick={next}>
            Continue
          </button>
        </div>
      )}

      {step === 2 && (
        <div>
          <h1>What HiddenSteps will never do</h1>
          <ul>
            <li>Record video or audio of your screen by default</li>
            <li>Take actions on your computer without you explicitly approving each one</li>
            <li>Send your data anywhere without telling you first</li>
            <li>Share what it learns with your employer, IT admin, or anyone else</li>
            <li>Require an account or internet connection to work</li>
          </ul>
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={next}>
            Continue
          </button>
        </div>
      )}

      {step === 3 && (
        <div>
          <h1>Permissions HiddenSteps may ask for</h1>
          <p>
            You will only be asked for the permissions your chosen level actually needs — not all
            of them upfront.
          </p>
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={next}>
            Continue
          </button>
        </div>
      )}

      {step === 4 && (
        <div>
          <h1>How much should HiddenSteps see?</h1>
          {LEVEL_OPTIONS.map((option) => (
            <label key={option.level} style={{ display: "block" }}>
              <input
                type="radio"
                name="privacy-level"
                value={option.level}
                checked={privacyLevel === option.level}
                onChange={() => setPrivacyLevel(option.level)}
              />
              {option.label} — {option.description}
            </label>
          ))}
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={next}>
            Continue
          </button>
        </div>
      )}

      {step === 5 && (
        <div>
          <h1>Choose how HiddenSteps thinks</h1>
          {runtimes === null && <p>Checking for local AI runtimes…</p>}
          {runtimes?.map((runtime) => (
            <p key={runtime.name}>
              {runtime.reachable ? "✓" : "✗"} {runtime.name}
              {runtime.reachable ? " — running locally" : " — not detected"}
            </p>
          ))}
          <label>
            Provider:{" "}
            <select value={providerType} onChange={(e) => setProviderType(e.target.value)}>
              <option value="ollama">Ollama (local)</option>
              <option value="openai">OpenAI (cloud)</option>
              <option value="anthropic">Anthropic (cloud)</option>
            </select>
          </label>
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={next}>
            Continue
          </button>
        </div>
      )}

      {step === 6 && (
        <div>
          <h1>Checking your setup…</h1>
          <button type="button" onClick={runValidation}>
            Run checks
          </button>
          {validation.checked && (
            <p data-testid="validation-result">
              {validation.ok ? "✓ Connection OK" : `✗ ${validation.error}`}
            </p>
          )}
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={next} disabled={!validation.checked || !validation.ok}>
            Continue
          </button>
        </div>
      )}

      {step === 7 && (
        <div>
          <h1>Ready to start</h1>
          <p>
            Level {privacyLevel} · {providerType}
          </p>
          <p>You can change any of this — or pause or delete everything — at any time.</p>
          <label>
            <input
              type="checkbox"
              checked={consented}
              onChange={(e) => setConsented(e.target.checked)}
            />
            I understand what HiddenSteps will observe and consent to start.
          </label>
          <button type="button" onClick={back}>
            Back
          </button>
          <button type="button" onClick={startObserving} disabled={!consented || starting}>
            Start observing
          </button>
        </div>
      )}

      {step === 8 && (
        <div>
          <h1>✓ HiddenSteps is now observing</h1>
          <p>You'll usually hear from us within a day — sooner if something obvious turns up.</p>
        </div>
      )}
    </section>
  );
}
