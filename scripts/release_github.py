"""Create a draft without replacing a tag, release or uploaded asset."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess

from release_artifacts import verify
from release_checks import REPOSITORY, git


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def remote_identity(release, branch):
    if branch not in ("main", "codex/0.6.0-windows-alpha"):
        raise ValueError("Release branch is not authorized")
    current = json.loads(gh("api", f"repos/{REPOSITORY}/git/ref/heads/{branch}"))["object"]["sha"]
    if current != release["commit"]:
        raise ValueError("The reviewed branch changed; check the new revision before releasing")
    refs = json.loads(gh("api", f"repos/{REPOSITORY}/git/matching-refs/tags/{release['tag']}"))
    exact = [ref for ref in refs if ref["ref"] == "refs/tags/" + release["tag"]]
    if exact:
        obj = exact[0]["object"]
        while obj["type"] == "tag":
            obj = json.loads(gh("api", f"repos/{REPOSITORY}/git/tags/{obj['sha']}"))["object"]
        if obj["type"] != "commit" or obj["sha"] != release["commit"]:
            raise ValueError("The version tag already identifies different content")
    pages = json.loads(gh("api", "--paginate", "--slurp", f"repos/{REPOSITORY}/releases?per_page=100"))
    if any(item["tag_name"] == release["tag"] for page in pages for item in page):
        raise ValueError("This version already has a release or draft; review it instead of replacing it")
    return bool(exact)


def upload(root, release, branch, output, verifier):
    key = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["plugins"]["updater"]["pubkey"]
    verify(output, key, verifier, release)
    tagged = remote_identity(release, branch)
    if not tagged:
        gh("api", f"repos/{REPOSITORY}/git/refs", "--method", "POST", "-f",
           "ref=refs/tags/" + release["tag"], "-f", "sha=" + release["commit"])
    notes = output.parent / "draft-notes.md"
    notes.write_text(f"Draft for maintainer review. Do not distribute before final acceptance.\n\n"
                     f"Source commit: `{release['commit']}`\n\n{release['notes']}\n\n"
                     "The installer has a Starframe update signature. Windows publisher signing is not configured. "
                     "Verify SHA256SUMS and its signature with the independently trusted Starframe update public key. "
                     "Matching project and component sources are attached.\n", encoding="utf-8")
    args = ["release", "create", release["tag"], "--repo", REPOSITORY, "--draft", "--verify-tag",
            "--title", "Starframe " + release["version"], "--notes-file", str(notes)]
    if release["prerelease"]:
        args.append("--prerelease")
    gh(*args)
    gh("release", "upload", release["tag"], "--repo", REPOSITORY,
       *[str(path) for path in sorted(output.iterdir())])
    downloaded = output.parent / "downloaded-draft"
    downloaded.mkdir(exist_ok=False)
    gh("release", "download", release["tag"], "--repo", REPOSITORY, "--dir", str(downloaded))
    verify(downloaded, key, verifier, release)
    result = json.loads(gh("release", "view", release["tag"], "--repo", REPOSITORY, "--json", "isDraft,url"))
    if not result["isDraft"]:
        raise ValueError("The release was published externally during verification")
    print("Uploaded draft files downloaded and verified: " + result["url"])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("preflight", "upload"))
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--branch", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--verifier", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    record = json.loads(args.release.read_text(encoding="utf-8"))
    if not re.fullmatch(r"v[0-9A-Za-z.-]+", record["tag"]) or git(root, "rev-parse", "HEAD") != record["commit"]:
        raise ValueError("Release checkout and identity must agree")
    if args.mode == "preflight":
        remote_identity(record, args.branch)
        print("Remote release identity is available.")
    else:
        if os.environ.get("GITHUB_EVENT_NAME") != "workflow_dispatch":
            raise ValueError("Draft upload requires an explicitly dispatched hosted workflow")
        upload(root, record, args.branch, args.output, args.verifier)
