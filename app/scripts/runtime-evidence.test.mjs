import assert from "node:assert/strict";
import test from "node:test";
import {
  assertCommandDenied,
  assertExternalRequestBlocked,
  assertPageLoaded,
  assertStandardToken,
  externalProbeUrl,
} from "./runtime-evidence.mjs";

test("token evidence must identify Medium IL, not missing or elevated identity", () => {
  const medium = '"Mandatory Label\\Medium Mandatory Level","Label","S-1-16-8192",""';
  assertStandardToken(medium);
  for (const groups of [
    "",
    medium.replace("8192", "12288"),
    medium.replace("8192", "16384"),
    medium.replace("8192", "81920"),
    `${medium}\n${medium.replace("8192", "12288")}`,
  ]) {
    assert.throws(() => assertStandardToken(groups));
  }
});

test("a missing bridge or command cannot pass the permission check", () => {
  assertCommandDenied({ status: "rejected", error: "app.version not allowed" });
  for (const result of [
    { status: "resolved", value: "0.1.0" },
    { status: "rejected", error: "unknown command" },
    { status: "rejected", error: "invoke is not a function" },
  ]) {
    assert.throws(() => assertCommandDenied(result));
  }
});

test("a rejected request needs an enforced violation for its exact origin", () => {
  const violation = {
    effectiveDirective: "connect-src",
    disposition: "enforce",
    blockedURI: externalProbeUrl,
  };
  assertExternalRequestBlocked({ status: "rejected", violations: [violation] });
  for (const violations of [
    [],
    [{ ...violation, disposition: "report" }],
    [{ ...violation, effectiveDirective: "img-src" }],
    [{ ...violation, blockedURI: "https://unrelated.invalid/" }],
  ]) {
    assert.throws(() =>
      assertExternalRequestBlocked({ status: "rejected", violations }),
    );
  }
});

test("rendered text alone cannot hide a blocked stylesheet or load failure", () => {
  const page = {
    title: "GTO Solver APP",
    heading: "GTO Solver APP",
    display: "flex",
    width: 1280,
    height: 800,
    lines: ["Placeholder", "Scaffold"],
    diagnostics: { errors: [], violations: [] },
  };
  assertPageLoaded(page);
  assert.throws(() => assertPageLoaded({ ...page, display: "block" }));
  assert.throws(() =>
    assertPageLoaded({
      ...page,
      diagnostics: { errors: ["script load failed"], violations: [] },
    }),
  );
  assert.throws(() =>
    assertPageLoaded({
      ...page,
      diagnostics: { errors: [], violations: [{ blockedURI: "inline" }] },
    }),
  );
});
