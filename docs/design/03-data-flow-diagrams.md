# Data Flow Diagrams

## 1. Capture → recommendation (the core loop)

```mermaid
flowchart LR
    A[Observation plugin\ncaptures raw signal] --> B{Classify}
    B -->|sensitive-category\ncandidate?| C[Redact]
    B -->|clearly benign| C
    C -->|confident redaction\nor nothing to redact| D[Summarize]
    C -->|uncertain| X[Drop — never stored]
    D --> E[Embed summary]
    D --> F[(EventStore\nSQLCipher)]
    E --> G[(sqlite-vec\nembeddings)]
    F --> H[Pattern Detection\nsequence match + similarity search]
    G --> H
    H -->|DetectedPattern\nfrequency, timing, cost| I[Recommendation Engine\nLayer 1: deterministic]
    I --> J[Recommendation Engine\nLayer 2: LLM synthesis]
    J -->|validated: numbers\nunchanged from Layer 1| K[Recommendation\nstored + surfaced to UI]
    J -.mismatch.-> I

    style X fill:#622,stroke:#900
```

Raw signal (A) never reaches durable storage (F/G) directly — only the post-Summarize abstraction does, per ADR-0006. The dashed "mismatch" edge is the ADR-0010 validator rejecting an LLM output that altered Layer 1's numeric claims and forcing regeneration.

## 2. Onboarding consent flow

```mermaid
sequenceDiagram
    participant U as User
    participant UI as Onboarding UI
    participant Core as Rust Core

    U->>UI: Launch app (first run)
    UI->>U: Explain what HiddenSteps does / does not do
    UI->>U: Explain each OS permission and why (per privacy level)
    U->>UI: Choose privacy level (0-4)
    UI->>Core: get_provider_detection()
    Core->>Core: Probe Ollama/LM Studio/LocalAI/vLLM,\nscan for llama.cpp; benchmark hardware
    Core-->>UI: Detected local runtimes + hardware suitability
    U->>UI: Choose AI provider (local or cloud)
    alt cloud provider chosen
        UI->>Core: test_provider_connectivity(key, endpoint)
        Core-->>UI: ok / error
    end
    UI->>U: Show final summary: level, provider, exact data scope
    U->>UI: Explicit consent ("Start observing")
    UI->>Core: start_observation(level, provider)
    Core->>Core: Request only the OS permissions\nthis level requires
    Core-->>UI: observation_status = active
```

No `start_observation` call is possible before this sequence completes — enforced by the Application layer refusing the command outside a completed-onboarding state, not just by UI flow order.

## 3. Privacy-level change (at any time post-onboarding)

```mermaid
sequenceDiagram
    participant U as User
    participant UI as Privacy Dashboard
    participant PE as Privacy Engine
    participant Obs as Observation plugins
    participant Audit as Audit Log

    U->>UI: Change privacy level (e.g., 2 -> 1, downgrade)
    UI->>PE: change_privacy_level(new_level)
    PE->>Obs: Deactivate plugins outside new level's manifest
    PE->>Audit: append(PrivacyLevelChanged{old, new, timestamp})
    PE-->>UI: new effective observation scope
    UI-->>U: Confirmation + updated "what's being observed" list
```

An **upgrade** (e.g., 1 → 3) follows the same path but re-enters the relevant slice of the onboarding permission-explanation step for any newly-required OS permission before activating the corresponding plugins — a privacy-level increase never silently expands observation.

## 4. Export / delete-all

```mermaid
sequenceDiagram
    participant U as User
    participant UI as Privacy Dashboard
    participant Sec as Security Layer
    participant DB as EventStore (SQLCipher)
    participant Vault as OS Credential Vault

    alt Export
        U->>UI: Export my data
        UI->>Sec: export_data()
        Sec->>DB: decrypt + serialize (summaries, patterns,\nrecommendations, settings, audit log)
        Sec-->>UI: portable archive (user-chosen location)
    else Delete all
        U->>UI: Delete all data
        UI->>Sec: delete_all_data()
        Sec->>DB: secure-erase all tables + vacuum
        Sec->>Vault: remove master key entry
        Sec-->>UI: confirmation
    end
```

Delete-all removes the vault key entry as well as the database contents — even a recovered copy of the encrypted file is unusable afterward, satisfying "leaves no traces" for Portable Mode and the general delete-all guarantee (FR-6).

## 5. Cloud-provider dispatch gating

```mermaid
flowchart TD
    A[Recommendation Engine\nneeds LLM call] --> B{Provider is_local\ni.e. Ollama/LM Studio/etc?}
    B -->|yes| E[Dispatch directly]
    B -->|no, cloud provider| C{Content's privacy level\n<= cloud-eligible threshold?}
    C -->|yes, e.g. Level 1-3 summary| D{Has user given\nseparate cloud-send consent\nfor this content class?}
    C -->|no, Level 4 raw-derived content| F[Blocked — must downgrade\ncontent or use local provider]
    D -->|yes| E
    D -->|no| G[Prompt user for\nexplicit per-class consent]
    G -->|granted| E
    G -->|denied| F
```

This gate lives in the Privacy Engine (ADR-0004) and wraps every `LlmProvider` call site — the Recommendation Engine cannot bypass it by calling a provider directly.
