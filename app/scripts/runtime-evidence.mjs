import assert from "node:assert/strict";

export const externalProbeUrl =
  "https://astra-csp-probe.invalid/scaffold-runtime-check";

/** Require evidence of authorization failure, not a missing command or transport. */
export function assertCommandDenied(result) {
  assert.equal(result.status, "rejected", "The ungranted core command succeeded");
  assert.match(result.error, /not allowed|denied|not permitted/i);
  assert.doesNotMatch(result.error, /not found|unknown command|not a function/i);
}

/** A DNS or connection error alone never establishes CSP enforcement. */
export function assertExternalRequestBlocked(result) {
  assert.equal(result.status, "rejected", "The external web request succeeded");
  assert.ok(
    result.violations.some(
      (event) =>
        event.effectiveDirective === "connect-src" &&
        event.disposition === "enforce" &&
        [externalProbeUrl, new URL(externalProbeUrl).origin].includes(event.blockedURI),
    ),
    "No enforced connect-src violation identifies the external probe",
  );
}

export function assertPageLoaded(page) {
  assert.equal(page.title, "GTO Solver APP");
  assert.equal(page.heading, "GTO Solver APP");
  assert.equal(page.display, "flex", "Bundled stylesheet did not apply");
  assert.ok(page.height > 0 && page.width > 0, "The placeholder has no layout");
  assert.equal(page.lines.length, 2);
  assert.ok(page.lines.every((line) => line.length > 0));
  assert.deepEqual(page.diagnostics.errors, [], "Page load reported an error");
  assert.deepEqual(page.diagnostics.violations, [], "Page load violated CSP");
}
