---
type: review-findings
from: Astra
to: Fable
date: 2026-09-05
verdict: verified for the stated scope
---

# Dependency advisory review for the Windows foundation

**Verdict: verified for the stated scope.** The locked-version advisory checks and
platform triage are complete. There are maintenance risks to track and a known
defect in the Linux native dependency graph. This is not a clean-bill-of-health
claim for all dependencies or proof of application security.

## Snapshot and evidence

Astra ran `pnpm audit --json` against the unchanged pnpm lockfile: zero reported
advisories across 226 dependency entries. The
[saved result](../astra/development-takeover/npm-advisories.json) records its lock hash.

Astra queried all 430 unique public registry package versions in both Cargo lockfiles
through the [OSV batch API](https://google.github.io/osv.dev/post-v1-querybatch/).
The [query script](../astra/development-takeover/audit_cargo.py),
[result and lock hashes](../astra/development-takeover/cargo-advisories.json), and
[advisory details](../astra/development-takeover/cargo-advisory-details.json)
make the check reproducible. All responses completed; no pages were omitted.

The scan found 17 affected package versions and 17 distinct RustSec notices. The
extra GHSA entry aliases the glib advisory. All flagged versions are in the app
lockfile. The scaffold agent traced published target-specific manifests; Astra
inspected the records and pinned Tauri manifests personally.

## Findings

### D01. Medium: the Linux native graph retains an affected glib version

**Location and version:** `app/src-tauri/Cargo.lock`, glib 0.18.5 in the GTK3 graph.

**Entry point, precondition, and impact:** Calling the affected string iterator in a
native build that includes glib can cause undefined behavior and a null-pointer
crash. RustSec identifies affected versions from 0.15.0 through versions below
0.20.0; the locked version is affected.
[RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html).

**Applicability:** The reviewed Tauri, runtime, windowing, and menu manifests select
GTK/WebKit dependencies for Linux/BSD targets. This Windows app uses WebView2.
The Ubuntu CI job builds the Rust solver and web frontend, not the native GTK app.
[Pinned Tauri target dependencies](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri/Cargo.toml).
This is a dependency-level defect; no affected call was demonstrated in the Windows
application. The absence of that target path does not fix the Linux dependency.

**Correction and closure:** Before supporting a Linux native app, update the compatible
GTK/Tauri dependency chain and verify the resolved versions and affected behavior.
An isolated incompatible glib version override is not a verified repair. Keep this
platform gate recorded; no blanket advisory ignore was added.

### D02. Low: maintenance notices remain in the dependency graph

**Location and versions:** Five UNIC 0.9.0 crates are present through
`tauri-utils 2.9.3 -> urlpattern 0.3.0 -> unic-ucd-ident` and its Unicode helpers.
These are normal dependencies on Windows as well as inputs to build tooling.
[Pinned tauri-utils manifest](https://github.com/tauri-apps/tauri/blob/tauri-v2.11.5/crates/tauri-utils/Cargo.toml).

**Evidence and impact:** The five UNIC notices report lack of maintenance, with no
patched versions. They do not demonstrate an exploit in this application.
[Representative UNIC notice](https://rustsec.org/advisories/RUSTSEC-2025-0100.html).
Ten GTK3 maintenance notices and the proc-macro-error 1.0.4 maintenance notice
apply to the reviewed Linux/BSD graph. Exact package/notice mappings are in the scan.

**Correction and closure:** Recheck upstream Tauri/urlpattern replacement work when
updating dependencies and before public distribution. Review any compatible replacement
against the app's URL/capability behavior. Do not label the Windows dependency graph
free of maintenance concerns merely because its known glib defect is target-excluded.

## Limits and next action

The scans check published version advisories at the recorded time. They do not audit
all source code, prove exploit reachability, or replace the separate permission and
runtime tests. This review makes no licence-audit or public-launch claim.

The current Windows phase 1 work can proceed with these findings recorded. Linux
native support and launch dependency maintenance remain explicit future gates.
