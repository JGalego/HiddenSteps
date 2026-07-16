# Ethical Analysis

## The core tension

Software that watches how someone works, in order to help them, is structurally identical — at the level of raw capability — to software that watches how someone works in order to control, evaluate, or replace them. The difference is entirely in *who the tool serves, what it retains, who can see it, and who consented*. Every ethical judgment below flows from that single fact: **HiddenSteps and employee-monitoring software are the same technology aimed in different directions**, and the design has to make the direction structurally verifiable, not just claimed in marketing copy.

## Power asymmetry and the "employer looks over your shoulder" failure mode

The single largest ethical risk to this product category is that it gets deployed *at* people rather than chosen *by* them:

- An employer mandates installation ("part of our efficiency initiative") and demands report exports.
- A manager gains informal access to a shared machine's HiddenSteps data.
- A "time saved" or "task frequency" report becomes evidence in a performance review or headcount-reduction decision — even though the tool never intended that use.

**Design consequence:** there should be no first-class feature for anyone other than the observed individual to view their data, no aggregate/team dashboard, and no export format optimized for management reporting. If enterprise deployment is supported at all (per PROMPT.md's Enterprise Deployment section), the enterprise policy layer should be able to *configure privacy floors and constrain AI-provider choice*, but must not be able to grant an employer visibility into an individual's captured observations or recommendations without that individual's explicit, revocable, per-export consent. This is a harder constraint than most enterprise software accepts, and it is the correct one for this category.

## Consent quality, not just consent existence

A checkbox clicked during onboarding is not meaningful consent if the user doesn't understand what's being collected. Ethically adequate consent requires:

- Plain-language explanation of what is and isn't collected *at each privacy level*, before any observation starts (PROMPT.md's First Run Experience already specifies this — it should be treated as a hard requirement, not a nice-to-have).
- The ability to *change your mind* at any time, with immediate effect (pause, downgrade privacy level, delete history) — not buried in settings.
- No dark patterns that make "Deep" observation the path of least resistance (e.g., no "recommended" badge nudging users toward more invasive modes just because it produces flashier demos).

## Dual-use and downstream harm

- **A recommendation itself can cause harm even if well-intentioned.** "This task takes 11 hours/month — automate it" is also, unavoidably, "this role's justification for 11 hours/month of work just went away." HiddenSteps should be honest that workflow-efficiency insight is not ethically neutral — it can be read by the same person as empowering and by their employer as a case for redundancy. The product cannot solve this by design, but it can refuse to make it worse: no aggregate reporting, no employer-facing surfaces, explicit user framing that this is *personal* leverage, not a productivity audit.
- **Automation recommendations can encode bad practice.** Recommending a Playwright script against a third party's ToS, or an RPA flow that silently handles data it shouldn't (e.g., scraping credentials, automating around access controls) needs guardrails in the recommendation engine itself — “can this be automated” must be checked against “should this be automated, and is it lawful/appropriate to do so.”
- **Local AI models can hallucinate confident-sounding but wrong workflow advice.** Because recommendations touch real work (scripts, macros, RPA), a wrong-but-confident suggestion has more real-world blast radius than a wrong chatbot answer. Explainability requirements (confidence, assumptions, alternatives — already specified in PROMPT.md) are an ethical requirement here, not just a UX nicety.

## Sensitive-population considerations

PROMPT.md assumes users may handle PII, PHI, trade secrets, financial records, legal documents, credentials, or classified information. Ethically, this means:

- Default observation modes must be **overwhelmingly likely to avoid capturing regulated data categories** (health records, legal privilege, classified material) even before any redaction engine runs — architecture (what's captured) is a stronger guarantee than after-the-fact filtering (what's redacted). See [05](05-privacy-analysis.md).
- Certain user populations (healthcare workers, lawyers, government/classified-cleared staff, journalists) may face professional or legal liability for *any* automated capture of their screen activity, regardless of redaction quality. The product should make it trivially easy to fully disable observation for specific applications, domains, or windows (e.g., an EHR system, a legal case-management tool), and should recommend Level 0/Manual mode proactively when it detects use of such applications, rather than requiring the user to know to configure this themselves.

## Environmental and access-equity note (secondary, but worth naming)

Local-first AI inference shifts compute (and therefore energy/battery cost) onto the user's own device rather than a shared cloud data center, and requires reasonably capable hardware (RAM, GPU) to get good local-model quality. This has a real equity dimension: users with older or lower-spec hardware get either worse recommendations or a worse experience (slower, hotter, more battery drain) than users with modern hardware — the opposite of typical cloud-SaaS economics, where hardware doesn't gate quality. The onboarding hardware-detection/benchmarking flow (already specified in PROMPT.md) should be honest about this tradeoff rather than presenting local-first as a costless default, and cloud fallback should remain a legitimate, non-shamed choice for under-resourced hardware.

## Summary judgment

HiddenSteps is ethically defensible specifically to the extent that it (1) never becomes visible or useful to anyone but the person being observed, (2) treats consent as ongoing and revocable rather than a one-time gate, (3) is honest that its insights have a dual-use dimension it cannot fully neutralize, and (4) defaults toward under-collection over over-collection whenever the two trade off. Every architectural decision in later phases should be checked against these four commitments.
