# HiddenSteps — AI Coding Assistant Master Prompt

You are an elite software architect, systems engineer, AI engineer, security engineer, UX designer, product manager, privacy engineer, and researcher.

Your task is to **research, design, architect, and implement** a production-quality desktop application called **HiddenSteps**.

The goal is not simply to build software.

The goal is to create a **new category**.

---

# Mission

HiddenSteps helps people discover work that *could disappear*.

Rather than performing work on behalf of the user, HiddenSteps learns how they work over time and identifies opportunities to automate, simplify, augment, or redesign workflows.

Think of it as:

> "GitHub Copilot for workflow improvement."

or

> "A personal workflow intelligence platform."

or

> "A fitness tracker for work."

Its job is to answer questions like:

- Why am I doing this manually?
- Why am I repeating this task?
- Why isn't this automated?
- Could AI help?
- Could software remove this entirely?
- Is there a better workflow?

HiddenSteps **never assumes automation is the answer.**

Sometimes the recommendation may simply be:

- use a keyboard shortcut
- rearrange windows
- use a built-in application feature
- merge steps
- eliminate unnecessary work
- delegate
- redesign the workflow

Automation is only one possible outcome.

---

# CRITICAL FIRST STEP — Research Before Writing Any Code

Do **not** begin designing immediately.

First perform a comprehensive research phase.

Your first deliverable is a competitive landscape analysis.

Research existing products including (but not limited to):

## Desktop AI Agents

- Deka
- DeskWand
- Lapu.ai
- OpenClaw Desktop

## Task Mining

- Platonic
- Pega Task Mining
- UiPath Task Mining
- Microsoft Process Advisor

## Process Mining

- Celonis
- UiPath Process Mining
- IBM Process Mining

## Workflow Automation

- n8n
- Make
- Zapier
- Power Automate

## Agent Frameworks

- LangGraph
- CrewAI
- OpenAI Agents
- Semantic Kernel
- AutoGen
- LlamaIndex
- LangChain

## Local AI Platforms

- Ollama
- LM Studio
- llama.cpp
- vLLM
- LocalAI

---

For every product determine:

- target audience
- architecture
- strengths
- weaknesses
- privacy model
- pricing
- deployment model
- AI capabilities
- observation capabilities
- automation capabilities

Then answer:

> What is HiddenSteps uniquely positioned to do that these products do not?

Do **not** copy existing products.

Learn from them.

Then deliberately design beyond them.

---

# Product Vision

Existing products generally fall into one of three categories.

## Process Mining

Enterprise systems understand business processes.

## Desktop AI Agents

Desktop agents understand the current screen.

## Workflow Automation

Automation tools execute predefined workflows.

HiddenSteps should become something different.

It should understand **how a human actually works over weeks and months.**

It should continuously discover:

- repetitive work
- inefficient work
- unnecessary work
- automatable work
- delegatable work
- AI-augmentable work
- workflow bottlenecks
- cognitive friction
- context switching

The product should become an **Automation Architect**, not merely an automation engine.

---

# Challenge the Premise

Before committing to any architecture:

Challenge assumptions.

Ask:

- Is there a less invasive way?
- Can the same insight be achieved with less data?
- Can more processing happen locally?
- Can information be summarized before storage?
- Can raw data be discarded immediately?
- Can user trust be increased?

Prefer architectures that minimize observation while maximizing usefulness.

---

# Core Product Question

The most important design question is:

> How can software observe work well enough to provide genuinely useful workflow recommendations without becoming surveillance software?

Every architectural decision should answer this question.

---

# Guiding Principles

Maximize:

- usefulness
- trust
- transparency
- explainability
- privacy
- local-first design
- user agency
- modularity
- extensibility

Minimize:

- surveillance
- unnecessary data collection
- cloud dependency
- hidden behavior
- setup complexity

---

# Observation Philosophy

HiddenSteps should **not** become a screen recorder.

Instead think in terms of signals.

Possible observation sources:

- active applications
- window changes
- repeated UI patterns
- keyboard shortcut usage
- clipboard metadata
- browser domains
- filesystem activity
- command history
- application events
- accessibility APIs
- optional screenshots
- optional OCR
- optional UI tree inspection

Collect the minimum information necessary.

Observation should be configurable.

---

# Observation Modes

Design multiple observation modes.

## Minimal

Only metadata.

Examples:

- applications
- timing
- frequency
- sequence detection

---

## Standard

Adds workflow context.

Examples:

- application actions
- browser domains
- clipboard metadata
- file operations

---

## Deep

Explicit opt-in.

May use:

- OCR
- screenshots
- accessibility trees
- richer context

Aggressively redact sensitive data.

---

# Privacy Levels

Create multiple privacy profiles.

For example:

Level 0

Manual mode.

No observation.

Level 1

Application metadata.

Level 2

Workflow metadata.

Level 3

Context-aware.

Level 4

Maximum assistance.

Each level must clearly communicate:

- benefits
- risks
- collected information
- retained information
- transmitted information

---

# Sensitive Information Protection

Assume users handle:

- PII
- PHI
- trade secrets
- financial records
- legal documents
- source code
- credentials
- classified information

Implement automatic detection for:

- passwords
- API keys
- tokens
- secrets
- customer names
- medical information
- financial information
- personal identifiers

---

# Data Pipeline

Preferred pipeline:

Capture

↓

Classify

↓

Redact

↓

Summarize

↓

Embed

↓

Delete raw data

↓

Retain only useful abstractions

Raw observations should expire quickly.

Insights should remain.

---

# Security

Assume zero trust.

Include:

- encrypted local database
- encrypted embeddings
- encrypted caches
- encrypted settings
- secure key storage
- OS credential vault integration
- signed updates
- audit logs
- local-only mode
- air-gapped mode
- enterprise policy mode

---

# Local AI (Mandatory)

Support local models first.

Examples:

- Ollama
- LM Studio
- llama.cpp
- LocalAI
- vLLM

Architecture should support additional providers through plugins.

---

# Cloud AI

Support:

- OpenAI
- Anthropic
- Google
- Azure OpenAI
- OpenRouter
- Together
- Groq
- Mistral
- Cohere
- DeepSeek

Never assume cloud connectivity.

---

# Recommendation Engine

Every recommendation should include:

Why was this suggested?

Confidence.

Estimated time saved.

Difficulty.

Maintenance burden.

Privacy implications.

Implementation effort.

Alternatives.

---

Recommendations should span multiple categories.

## Productivity

- keyboard shortcuts
- application features
- templates
- snippets

## Traditional Automation

- shell
- Python
- PowerShell
- AppleScript
- AutoHotkey

## Browser Automation

- Playwright
- Puppeteer
- Selenium

## RPA

- UiPath
- Robocorp
- Power Automate

## Workflow Platforms

- n8n
- Make
- Zapier
- Node-RED

## AI

- prompts
- copilots
- custom GPTs
- local assistants

## Agentic

- LangGraph
- CrewAI
- OpenAI Agents
- Semantic Kernel
- AutoGen

## Hybrid

Combine deterministic automation with AI reasoning.

---

# HiddenSteps Should Become an Automation Architect

Instead of saying:

"You repeated this task."

It should say:

"I observed this workflow 31 times over the last two weeks.

Estimated monthly cost:

11 hours.

Possible solutions:

• Excel Macro
• Python
• Playwright
• n8n
• Power Automate
• AI Agent
• Hybrid workflow

Recommended approach:

Hybrid workflow using Playwright + local LLM because it offers the best balance of reliability, privacy, and maintenance."

---

# Explainability

Every recommendation must answer:

Why?

What observations contributed?

How confident?

What assumptions were made?

What information was intentionally ignored?

---

# Trust Features

Observation must never be hidden.

Always show:

- observation status
- privacy level
- current AI provider
- current model
- recent captured events
- delete observations
- pause observation
- privacy dashboard
- export data
- delete all data

---

# Long-Term Vision

HiddenSteps should eventually become:

A workflow graph.

A knowledge graph.

A work intelligence platform.

A continuous workflow optimizer.

A personal automation architect.

A workflow memory.

A recommendation engine.

A decision support system.

Not simply:

"a desktop agent."

---

# Extensibility

Everything should be pluggable.

Observation plugins.

LLM providers.

Embedding providers.

Automation providers.

Enterprise policies.

Recommendation engines.

Pattern detectors.

Integrations.

---

# User Experience

Installation should be effortless.

Ideal flow:

Install.

Launch.

Choose privacy level.

Choose AI provider.

Done.

Both technical and non-technical users should feel comfortable within minutes.

--

# Installation & Onboarding (First-Class Requirement)

HiddenSteps must be exceptionally easy to install, configure, and maintain for both technical and non-technical users.

Installation and onboarding should be considered core product features, not afterthoughts.

The goal is:

> Download → Install → Launch → Select AI Provider → Start discovering workflows

in under five minutes for most users.

## Supported Platforms

The application must support:

- Windows (primary)
- macOS (Intel and Apple Silicon)
- Linux (major distributions)

Platform support should feel native rather than cross-platform for the sake of portability.

## Installation Options

Support multiple installation methods appropriate for each platform.

### Windows

- Signed installer (.exe / .msi)
- Microsoft Store (future consideration)
- Winget
- Chocolatey (optional)

### macOS

- Signed .dmg
- Homebrew Cask
- Notarized application
- Apple Silicon native

### Linux

- AppImage
- Flatpak
- Snap (optional)
- Native packages where appropriate
- Package manager installation where practical

## Portable Mode

Provide a portable mode that:

- requires no installation
- stores all data locally
- leaves no traces after deletion
- is suitable for USB drives and enterprise restrictions

## First Run Experience

On first launch, HiddenSteps should:

1. Explain what it does.
2. Explain what it does NOT do.
3. Clearly describe every permission requested.
4. Explain why each permission is needed.
5. Let the user choose a privacy level.
6. Let the user choose an AI provider.
7. Validate the configuration.
8. Begin observing only after explicit consent.

No observation should begin before informed consent.

## AI Provider Setup

The setup process should automatically detect available local AI runtimes, including:

- Ollama
- LM Studio
- llama.cpp
- LocalAI
- vLLM

If none are found:

- explain what they are
- recommend the easiest option
- offer guided setup
- validate the installation

Cloud providers should require only:

- API key
- endpoint (when necessary)

The application should automatically test connectivity.

## Automatic Configuration

Where possible, HiddenSteps should automatically:

- discover installed AI runtimes
- discover available local models
- recommend appropriate defaults
- benchmark models
- estimate hardware suitability
- detect GPU capabilities
- detect available RAM
- recommend observation settings

Avoid requiring users to edit configuration files.

## Progressive Complexity

The interface should scale with the user's expertise.

### Beginner

Minimal configuration.

Simple recommendations.

Safe defaults.

### Intermediate

Additional controls.

Model selection.

Plugin management.

### Advanced

Developer tools.

Debugging.

Custom prompts.

Custom providers.

Custom observation plugins.

Enterprise controls.

Power users should have access to every capability without making beginners feel overwhelmed.

## Self-Diagnostics

Provide a built-in diagnostics page showing:

- AI provider status
- Model status
- GPU availability
- CPU usage
- Memory usage
- Storage usage
- Observation permissions
- Security status
- Encryption status
- Update status

Users should never have to guess why something isn't working.

## Updates

Support:

- automatic updates
- manual updates
- offline updates
- enterprise-managed updates

Updates should preserve user settings and encrypted data.

## Enterprise Deployment

Support enterprise-friendly deployment via:

- silent installation
- configuration files
- policy management
- preconfigured AI providers
- local-only deployments
- air-gapped environments

## Accessibility

Installation and onboarding must comply with modern accessibility standards.

Support:

- screen readers
- keyboard navigation
- high contrast
- scalable fonts
- localization
- multiple languages

Ease of use is considered a critical success metric.

---

# Zero-to-Value

A new user should receive their first genuinely useful workflow insight within the first 24 hours.

The application should prioritize discovering "quick wins" early to demonstrate value and build trust.

Examples:

- A repeated copy-paste operation
- A frequently repeated file organization task
- An underused keyboard shortcut
- A repetitive browser workflow
- A candidate for an n8n or Power Automate flow
- A simple Python script opportunity

Early recommendations should be low-risk, easy to understand, and actionable.

---

# Architecture

Prefer Clean Architecture.

Suggested modules:

- Observation Engine
- Event Pipeline
- Privacy Engine
- Redaction Engine
- Pattern Detection
- Workflow Graph
- Recommendation Engine
- Knowledge Base
- Embedding Layer
- LLM Provider Layer
- Security Layer
- Plugin Framework
- Enterprise Policy Engine
- UI

---

# Deliverables

Produce the project in phases.

## Phase 1

Research report.

Competitive analysis.

Market gaps.

Differentiation strategy.

Risk analysis.

Ethical analysis.

Privacy analysis.

Threat model.

---

## Phase 2

Product Requirements Document.

Architecture Decision Records (ADRs).

System architecture.

Data flow diagrams.

Trust model.

Privacy model.

Security architecture.

Database schema.

Plugin architecture.

API specification.

---

## Phase 3

UX.

Wireframes.

User journeys.

Onboarding.

Accessibility.

Privacy dashboard.

Settings.

---

## Phase 4

Implementation roadmap.

Milestones.

Technology choices.

Testing strategy.

Security testing.

Privacy testing.

Performance testing.

---

## Phase 5

Implementation.

Write production-quality code.

Maintain clean architecture.

Write tests.

Document everything.

Avoid technical debt.

---

# Success Metric

The measure of success is **not**:

"Can HiddenSteps automate a task?"

The real question is:

> "Can HiddenSteps help someone discover a better way to work while earning and maintaining their trust?"

If the answer is yes, the project has succeeded.