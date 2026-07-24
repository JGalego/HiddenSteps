/// The privacy manifest version this build of HiddenSteps ships with, per
/// `docs/design/05-privacy-model.md` §5. Bump this whenever a release changes
/// what any privacy level captures — `requires_reconsent` below is what
/// actually gates observation on a user re-consenting once this changes; a
/// bump with no caller checking it would be exactly as inert as this
/// constant's absence was before.
pub const CURRENT_MANIFEST_VERSION: i64 = 1;

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
