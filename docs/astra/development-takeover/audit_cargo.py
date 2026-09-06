"""Query OSV for public registry packages in the committed Cargo lockfiles."""

import hashlib
import json
import tomllib
import urllib.request
from datetime import UTC, datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
LOCKFILES = [ROOT / "Cargo.lock", ROOT / "app/src-tauri/Cargo.lock"]


def main():
    packages = set()
    hashes = {}
    for path in LOCKFILES:
        raw = path.read_bytes()
        hashes[str(path.relative_to(ROOT))] = hashlib.sha256(raw).hexdigest()
        for package in tomllib.loads(raw.decode("utf-8"))["package"]:
            if package.get("source", "").startswith("registry+"):
                packages.add((package["name"], package["version"]))
    queries = [
        {"package": {"name": name, "ecosystem": "crates.io"}, "version": version}
        for name, version in sorted(packages)
    ]
    findings = []
    for start in range(0, len(queries), 100):
        pending = queries[start : start + 100]
        for _ in range(20):
            request = urllib.request.Request(
                "https://api.osv.dev/v1/querybatch",
                data=json.dumps({"queries": pending}).encode("utf-8"),
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            with urllib.request.urlopen(request, timeout=30) as response:
                results = json.load(response)["results"]
            if len(results) != len(pending):
                raise ValueError("OSV response count does not match requested packages")
            next_page = []
            for query, result in zip(pending, results, strict=True):
                if result.get("vulns"):
                    findings.append({"query": query, "vulns": result["vulns"]})
                if result.get("next_page_token"):
                    next_page.append({**query, "page_token": result["next_page_token"]})
            if not next_page:
                break
            pending = next_page
        else:
            raise RuntimeError("OSV pagination exceeded the bound; audit is incomplete")
    evidence = {
        "checked_utc": datetime.now(UTC).isoformat(),
        "source": "https://google.github.io/osv.dev/post-v1-querybatch/",
        "lockfile_sha256": hashes,
        "unique_registry_versions": len(packages),
        "findings": findings,
        "scope": "Registry version advisories; not a reachability or code-security proof",
    }
    output = Path(__file__).with_name("cargo-advisories.json")
    output.write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"packages": len(packages), "findings": findings}, indent=2))


if __name__ == "__main__":
    main()
