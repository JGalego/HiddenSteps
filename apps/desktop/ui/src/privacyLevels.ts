// The concrete permission descriptors each privacy level introduces, per
// docs/design/05-privacy-model.md §1. These are what the user is asked to
// acknowledge when raising to a level, and what set_privacy_level records in
// the audit log — so the acknowledgment is a real, inspectable record of what
// was consented to, not a cosmetic literal. Each level is cumulative: the
// descriptors for a level are everything it introduces over the one below it.

const LEVEL_PERMISSIONS: Record<number, string[]> = {
  0: [], // Manual — observes nothing, so nothing to acknowledge.
  1: ["app_focus", "window_title", "shortcut_used"],
  2: ["browser_domain", "clipboard_metadata", "file_operation_metadata"],
  3: ["in_app_context", "browser_page_context", "file_operation_context"],
  4: ["ocr_text"],
};

/**
 * Every permission descriptor in effect at `level` — cumulative across all
 * levels up to and including it, since each level is a superset of the one
 * below. This is what a caller sends to `set_privacy_level` as the
 * acknowledged set.
 */
export function acknowledgedPermissionsFor(level: number): string[] {
  const permissions: string[] = [];
  for (let l = 1; l <= level; l++) {
    permissions.push(...(LEVEL_PERMISSIONS[l] ?? []));
  }
  return permissions;
}
