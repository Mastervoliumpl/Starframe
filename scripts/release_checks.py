"""Validate release identity before granting a job signing or upload access."""

import argparse
import json
from pathlib import Path
import re
import subprocess

from versions import read_version, synchronize

REPOSITORY = "Mastervoliumpl/Starframe"


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def release_notes(root, version):
    text = (root / "CHANGELOG.md").read_text(encoding="utf-8")
    match = re.search(rf"(?m)^## {re.escape(version)} - (\d{{4}}-\d{{2}}-\d{{2}})\s*$", text)
    if not match:
        raise ValueError("Finalize the matching dated changelog entry before preparing a release")
    notes = re.split(r"(?m)^## ", text[match.end():], maxsplit=1)[0].strip()
    if not notes or len(notes.encode("utf-8")) > 65_536:
        raise ValueError("Release notes must be nonempty and fit the updater limit")
    return notes


def validate_runs(runs, sha, branch):
    for name in ("Checks", "Dependency security"):
        matching = [run for run in runs if run["name"] == name
                    and run["head_sha"] == sha and run["head_branch"] == branch
                    and run["head_repository"]["full_name"] == REPOSITORY]
        # A newer failure or cancellation must not be hidden by an old success.
        latest = max(matching, key=lambda run: (run["run_number"], run["run_attempt"]), default=None)
        if latest is None or latest["status"] != "completed" or latest["conclusion"] != "success":
            raise ValueError(f"The selected commit needs a successful current {name} run")


def identity(root, sha):
    if not re.fullmatch(r"[0-9a-f]{40}", sha) or git(root, "rev-parse", "HEAD") != sha:
        raise ValueError("Release checkout must match the full selected commit SHA")
    if git(root, "status", "--porcelain", "--untracked-files=no"):
        raise ValueError("Release checkout has tracked changes")
    version = read_version(root)
    if "+" in version or synchronize(root):
        raise ValueError("Release versions must agree and cannot contain build metadata")
    notes = release_notes(root, version)
    return {"commit": sha, "version": version, "tag": "v" + version,
            "prerelease": "-" in version, "notes": notes}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--branch", default="main")
    parser.add_argument("--runs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    record = identity(root, args.commit)
    validate_runs(json.loads(args.runs.read_text(encoding="utf-8-sig")), args.commit, args.branch)
    args.output.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    print(f"Release identity verified: {record['tag']} at {args.commit}")


if __name__ == "__main__":
    main()
