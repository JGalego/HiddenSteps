/// The privacy manifest version this build of HiddenSteps ships with, per
/// `docs/design/05-privacy-model.md` §5. Bump this whenever a release changes
/// what any privacy level captures — `requires_reconsent` below is what
/// actually gates observation on a user re-consenting once this changes; a
/// bump with no caller checking it would be exactly as inert as this
/// constant's absence was before.
///
/// Bumped 1 -> 2: Level 4 ("Maximum assistance") gained its first real
/// capture capability — `hiddensteps_observation::ScreenshotSource` plus
/// `hiddensteps_pipeline::OcrsTextExtractor` (screenshot + OCR). Before this,
/// Level 4 was a documented, consentable privacy level that captured nothing
/// at all; anyone who already consented to it consented to a manifest that
/// didn't yet describe an actual screen-content capture, so this bump forces
/// a fresh, accurate re-consent for exactly the users this newly affects —
/// per `docs/design/05-privacy-model.md` §5's rule that a manifest change
/// requires re-consent from anyone on the affected level, not just new users.
///
/// Bumped 2 -> 3: Levels 2 and 3 gained their first real browser-activity
/// capture capability — `hiddensteps_observation::BrowserBridgeSource`
/// (`CapturedPayload::BrowserDomainVisited` at Level 2,
/// `CapturedPayload::BrowserPageTitleViewed` at Level 3), fed by a new
/// browser-extension component (`apps/browser-extension`) rather than
/// inferred from a window title as before. Before this, both levels were
/// documented, consentable manifest entries that this implementation never
/// actually produced — same situation as Level 4 pre-bump-2 — so anyone
/// already on Level 2 or 3 consented to a manifest that didn't yet describe
/// real domain/title capture, and needs the same fresh, accurate re-consent
/// this bump forces for exactly those two levels.
pub const CURRENT_MANIFEST_VERSION: i64 = 3;

/// Per `docs/design/05-privacy-model.md` §5: each privacy level's signal manifest
/// is versioned. If a future release changes what a level captures, the affected
/// level's manifest version increases, and any user currently on that level must
/// see a re-consent prompt describing what changed before the new manifest takes
/// effect.
pub fn requires_reconsent(consented_manifest_version: i64, current_manifest_version: i64) -> bool {
    consented_manifest_version < current_manifest_version
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_reconsent_needed_when_versions_match() {
        assert!(!requires_reconsent(3, 3));
    }

    #[test]
    fn reconsent_required_when_current_version_is_newer() {
        assert!(requires_reconsent(2, 3));
    }

    #[test]
    fn a_consented_version_ahead_of_current_does_not_spuriously_require_reconsent() {
        // Shouldn't happen in practice (a user can't have consented to a manifest
        // version that doesn't exist yet), but the function should still behave
        // sanely rather than assume `consented <= current`.
        assert!(!requires_reconsent(5, 3));
    }
}
