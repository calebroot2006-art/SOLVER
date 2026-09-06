"""Read this repository's Actions evidence using the existing Git credential helper."""

import argparse
import json
import os
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REPOSITORY = "calebroot2006-art/SOLVER"
API = f"https://api.github.com/repos/{REPOSITORY}"


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    """Do not forward the GitHub credential to a signed artifact/log download host."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urllib.parse.urlsplit(newurl).scheme != "https":
            raise ValueError("Refusing a non-HTTPS redirect")
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if redirected is not None and (
            urllib.parse.urlsplit(req.full_url).netloc
            != urllib.parse.urlsplit(newurl).netloc
        ):
            redirected.remove_header("Authorization")
        return redirected


def credential():
    """Read credentials in memory; never print or persist the helper's response."""
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token:
        return token
    result = subprocess.run(
        ["git", "-c", "credential.interactive=false", "credential", "fill"],
        input="protocol=https\nhost=github.com\n\n",
        capture_output=True,
        text=True,
        timeout=20,
        check=False,
    )
    fields = dict(
        line.split("=", 1) for line in result.stdout.splitlines() if "=" in line
    )
    if result.returncode or not fields.get("password"):
        raise RuntimeError("Existing GitHub credential is unavailable")
    return fields["password"]


def fetch(path):
    request = urllib.request.Request(
        API + path,
        headers={
            "Authorization": "Bearer " + credential(),
            "Accept": "application/vnd.github+json",
            "User-Agent": "Astra-repository-validation",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.build_opener(SafeRedirect()).open(
        request, timeout=30
    ) as response:
        return response.read()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    runs = commands.add_parser("runs")
    runs.add_argument("--branch", default="solver/astra-takeover")
    jobs = commands.add_parser("jobs")
    jobs.add_argument("run", type=int)
    jobs.add_argument("--steps", action="store_true", help="Include every job step")
    logs = commands.add_parser("log")
    logs.add_argument("job", type=int)
    artifacts = commands.add_parser("artifacts")
    artifacts.add_argument("run", type=int)
    download = commands.add_parser("download")
    download.add_argument("artifact", type=int)
    download.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "runs":
        query = urllib.parse.urlencode({"branch": args.branch, "per_page": 5})
        data = json.loads(fetch(f"/actions/runs?{query}"))
        data = [
            {k: run[k] for k in ("id", "head_sha", "status", "conclusion", "html_url")}
            for run in data["workflow_runs"]
        ]
    elif args.command == "jobs":
        data = json.loads(fetch(f"/actions/runs/{args.run}/jobs?per_page=100"))
        data = [
            {
                k: job[k]
                for k in ("id", "name", "status", "conclusion", "html_url", "steps")
            }
            for job in data["jobs"]
        ]
        if not args.steps:
            for job in data:
                job["steps"] = [
                    step
                    for step in job["steps"]
                    if step["status"] == "in_progress"
                    or step["conclusion"] == "failure"
                ]
    elif args.command == "log":
        print(fetch(f"/actions/jobs/{args.job}/logs").decode("utf-8", errors="replace"))
        return
    elif args.command == "artifacts":
        data = json.loads(fetch(f"/actions/runs/{args.run}/artifacts"))
    else:
        args.output.write_bytes(fetch(f"/actions/artifacts/{args.artifact}/zip"))
        print(f"Saved artifact {args.artifact} to {args.output}")
        return
    print(json.dumps(data, indent=2))


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    main()
