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

// Sensible starting points for cloud providers, since we can't detect what
// models are available without an API key. The user can type over these.
const CLOUD_MODEL_DEFAULTS: Record<string, string> = {
  openai: "gpt-4o-mini",
  anthropic: "claude-sonnet-5",
};

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
  const [selectedModel, setSelectedModel] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [validation, setValidation] = useState<{ checked: boolean; ok: boolean; error: string | null }>({
    checked: false,
    ok: false,
    error: null,
  });
  const [consented, setConsented] = useState(false);
  const [starting, setStarting] = useState(false);

  const isLocal = providerType === "ollama";
  const detectedModels = runtimes?.find((r) => r.name === providerType)?.models ?? [];

  useEffect(() => {
    if (step === 5 && runtimes === null) {
      tauriBridge.getProviderDetection().then(setRuntimes);
    }
  }, [step, runtimes]);

  // Real bug this fixed: onboarding used to let you reach the validation
  // step with no model chosen at all, which silently probed a model named
  // "default" that doesn't exist on a real Ollama instance. Pick a sensible
  // one automatically as soon as we know what's available, rather than
  // requiring the user to notice an empty field.
  useEffect(() => {
    if (selectedModel) return;
    if (isLocal && detectedModels.length > 0) {
      setSelectedModel(detectedModels[0]);
    } else if (!isLocal && CLOUD_MODEL_DEFAULTS[providerType]) {
      setSelectedModel(CLOUD_MODEL_DEFAULTS[providerType]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- intentionally only fills an *empty* field
  }, [providerType, detectedModels]);

  // A previously-passed check shouldn't silently carry over once the user
  // changes what they're actually about to validate.
  useEffect(() => {
    setValidation({ checked: false, ok: false, error: null });
  }, [providerType, selectedModel, apiKey]);

  const next = () => setStep((s) => Math.min(TOTAL_STEPS, s + 1));
  const back = () => setStep((s) => Math.max(1, s - 1));

  const runValidation = async () => {
    const result = await tauriBridge.testProviderConnectivity({
      provider_type: providerType,
      model: selectedModel,
      api_key: apiKey || undefined,
    });
    setValidation({ checked: true, ok: result.ok, error: result.error });
  };

  const startObserving = async () => {
    setStarting(true);
    // Real bug this fixed: onboarding used to finish without ever calling
    // set_ai_provider, so the provider/model the user just picked and
    // validated was never actually persisted — Settings would show "No
    // provider configured yet." immediately after finishing onboarding.
    await tauriBridge.setAiProvider({
      id: providerType,
      provider_type: providerType,
      is_local: isLocal,
      model_name: selectedModel,
      api_key: apiKey || undefined,
    });
    await tauriBridge.setPrivacyLevel(privacyLevel, ["acknowledged"]);
    await tauriBridge.completeOnboarding();
    setStarting(false);
    next(); // to the confirmation screen
    onComplete();
  };

  return (
    <section className="onboarding-shell" aria-label="HiddenSteps setup" data-testid="onboarding-wizard">
      <div className="step-progress" aria-hidden="true">
        {Array.from({ length: TOTAL_STEPS }, (_, i) => (
          <span key={i} className={`step-progress-dot ${i < step ? "is-complete" : ""}`} />
        ))}
      </div>
      <p className="step-indicator-text" data-testid="step-indicator">
        Step {step} of {TOTAL_STEPS}
      </p>

      {step === 1 && (
        <div className="card">
          <h1>Welcome to HiddenSteps</h1>
          <p>
            HiddenSteps learns how you work, over time, and shows you specific ways to work less
            hard at the repetitive parts.
          </p>
          <p>It never acts on your behalf. It only observes and suggests. You decide everything.</p>
          <div className="btn-row">
            <button className="btn btn-primary" type="button" onClick={next}>
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 2 && (
        <div className="card">
          <h1>What HiddenSteps will never do</h1>
          <ul>
            <li>Record video or audio of your screen by default</li>
            <li>Take actions on your computer without you explicitly approving each one</li>
            <li>Send your data anywhere without telling you first</li>
            <li>Share what it learns with your employer, IT admin, or anyone else</li>
            <li>Require an account or internet connection to work</li>
          </ul>
          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn-primary" type="button" onClick={next}>
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 3 && (
        <div className="card">
          <h1>Permissions HiddenSteps may ask for</h1>
          <p>
            You will only be asked for the permissions your chosen level actually needs — not all
            of them upfront.
          </p>
          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn-primary" type="button" onClick={next}>
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 4 && (
        <div className="card">
          <h1>How much should HiddenSteps see?</h1>
          {LEVEL_OPTIONS.map((option) => (
            <label key={option.level} className="radio-option">
              <input
                type="radio"
                name="privacy-level"
                value={option.level}
                checked={privacyLevel === option.level}
                onChange={() => setPrivacyLevel(option.level)}
              />
              <span>
                <span className="radio-option-label">{option.label}</span>
                <span className="radio-option-description">{option.description}</span>
              </span>
            </label>
          ))}
          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn-primary" type="button" onClick={next}>
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 5 && (
        <div className="card">
          <h1>Choose how HiddenSteps thinks</h1>
          {runtimes === null && <p>Checking for local AI runtimes…</p>}
          {runtimes?.map((runtime) => (
            <p key={runtime.name} className={`runtime-status-line ${runtime.reachable ? "is-reachable" : "is-unreachable"}`}>
              {runtime.reachable ? "✓" : "✗"} {runtime.name}
              {runtime.reachable
                ? ` — running locally (${runtime.models.length} model${runtime.models.length === 1 ? "" : "s"})`
                : " — not detected"}
            </p>
          ))}

          <label className="field">
            Provider:
            <select value={providerType} onChange={(e) => { setProviderType(e.target.value); setSelectedModel(""); }}>
              <option value="ollama">Ollama (local)</option>
              <option value="openai">OpenAI (cloud)</option>
              <option value="anthropic">Anthropic (cloud)</option>
            </select>
          </label>

          {isLocal ? (
            detectedModels.length > 0 ? (
              <label className="field">
                Model:
                <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)}>
                  {detectedModels.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <label className="field">
                Model (none detected — enter one you've pulled)
                <input
                  type="text"
                  value={selectedModel}
                  onChange={(e) => setSelectedModel(e.target.value)}
                  placeholder="e.g. qwen2.5:7b-instruct"
                />
              </label>
            )
          ) : (
            <>
              <label className="field">
                Model:
                <input
                  type="text"
                  value={selectedModel}
                  onChange={(e) => setSelectedModel(e.target.value)}
                />
              </label>
              <label className="field">
                API key:
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder="Sent only to this provider, stored only in your OS credential vault"
                />
              </label>
            </>
          )}

          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn-primary" type="button" onClick={next}>
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 6 && (
        <div className="card">
          <h1>Checking your setup…</h1>
          <div className="btn-row">
            <button className="btn" type="button" onClick={runValidation} disabled={!selectedModel.trim()}>
              Run checks
            </button>
          </div>
          {!selectedModel.trim() && <p>Choose or enter a model on the previous screen first.</p>}
          {validation.checked && (
            <p
              className={`validation-result ${validation.ok ? "is-ok" : "is-error"}`}
              data-testid="validation-result"
            >
              {validation.ok ? "✓ Connection OK" : `✗ ${validation.error}`}
            </p>
          )}
          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button
              className="btn btn-primary"
              type="button"
              onClick={next}
              disabled={!validation.checked || !validation.ok}
            >
              Continue
            </button>
          </div>
        </div>
      )}

      {step === 7 && (
        <div className="card">
          <h1>Ready to start</h1>
          <p>
            Level {privacyLevel} · {providerType} ({selectedModel})
          </p>
          <p>You can change any of this — or pause or delete everything — at any time.</p>
          <label className="field-inline">
            <input
              type="checkbox"
              checked={consented}
              onChange={(e) => setConsented(e.target.checked)}
            />
            I understand what HiddenSteps will observe and consent to start.
          </label>
          <div className="btn-row">
            <button className="btn" type="button" onClick={back}>
              Back
            </button>
            <button className="btn btn-primary" type="button" onClick={startObserving} disabled={!consented || starting}>
              Start observing
            </button>
          </div>
        </div>
      )}

      {step === 8 && (
        <div className="card onboarding-confirmation">
          <h1>✓ HiddenSteps is now observing</h1>
          <p>You'll usually hear from us within a day — sooner if something obvious turns up.</p>
        </div>
      )}
    </section>
  );
}
