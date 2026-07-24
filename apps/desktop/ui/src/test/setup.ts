import "@testing-library/jest-dom/vitest";
import { expect } from "vitest";
import { toHaveNoViolations } from "jest-axe";

// Register jest-axe's matcher so a11y.test.tsx can assert
// `.toHaveNoViolations()` — the automated accessibility gate from
// docs/ux/06-accessibility.md §5.
expect.extend(toHaveNoViolations);
