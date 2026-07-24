import { describe, expect, it } from "vitest";
import { acknowledgedPermissionsFor } from "./privacyLevels";

describe("acknowledgedPermissionsFor", () => {
  it("returns nothing for Manual (level 0), which observes nothing", () => {
    expect(acknowledgedPermissionsFor(0)).toEqual([]);
  });

  it("returns level 1's own permissions at level 1", () => {
    expect(acknowledgedPermissionsFor(1)).toEqual([
      "app_focus",
      "window_title",
      "shortcut_used",
    ]);
  });

  it("is cumulative — a higher level includes every lower level's permissions", () => {
    const level2 = acknowledgedPermissionsFor(2);
    for (const p of acknowledgedPermissionsFor(1)) {
      expect(level2).toContain(p);
    }
    expect(level2.length).toBeGreaterThan(acknowledgedPermissionsFor(1).length);
  });

  it("includes OCR only at the maximum level", () => {
    expect(acknowledgedPermissionsFor(3)).not.toContain("ocr_text");
    expect(acknowledgedPermissionsFor(4)).toContain("ocr_text");
  });
});
