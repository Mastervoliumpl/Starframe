"""Create a draft without replacing a tag, release or uploaded asset."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess

from release_artifacts import verify
from release_checks import REPOSITORY, identity


def gh(*args):
    return subprocess.check_output(["gh", *args], text=True)


def draft_notes(release):
    return (f"<!-- starframe-draft:{release['commit']} -->\n"
            "Draft for maintainer review. Do not distribute before final acceptance.\n\n"
            f"Source commit: `{release['commit']}`\n\n{release['notes']}\n\n"
            "Verify the attached installer and SHA256SUMS signatures with the independently trusted "
            "Starframe update public key. Windows publisher signing is not configured. "
            "Matching project and component sources must accompany the installer.\n")


def remote_identity(release, branch, rehearsal=False):
    if branch != "main" and not (rehearsal and branch == "codex/0.6.0-windows-alpha"):
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
    existing = [item for page in pages for item in page if item["tag_name"] == release["tag"]]
    if existing:
        draft = existing[0]
        permitted = (rehearsal and exact and draft["draft"] and not draft["assets"]
                     and draft["author"]["login"] == "Mastervoliumpl"
                     and draft["body"].replace("\r\n", "\n") == draft_notes(release))
        if not permitted:
            raise ValueError("This version already has a release or draft; review it instead of replacing it")
    return bool(exact), bool(existing)


def create_draft(release, tagged, notes):
    if not tagged:
        gh("api", f"repos/{REPOSITORY}/git/refs", "--method", "POST", "-f",
           "ref=refs/tags/" + release["tag"], "-f", "sha=" + release["commit"])
    notes.write_text(draft_notes(release), encoding="utf-8", newline="\n")
    args = ["release", "create", release["tag"], "--repo", REPOSITORY, "--draft", "--verify-tag",
            "--title", "Starframe " + release["version"], "--notes-file", str(notes)]
    if release["prerelease"]:
        args.append("--prerelease")
    gh(*args)


def upload(root, release, branch, output, verifier, rehearsal=False):
    key = json.loads((root / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))["plugins"]["updater"]["pubkey"]
    verify(output, key, verifier, release)
    tagged, existing = remote_identity(release, branch, rehearsal)
    if rehearsal and not existing:
        raise ValueError("The maintainer must prepare the empty rehearsal draft before approval")
    if not existing:
        create_draft(release, tagged, output.parent / "draft-notes.md")
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
    parser.add_argument("mode", choices=("preflight", "upload", "scaffold"))
    parser.add_argument("--release", type=Path, required=True)
    parser.add_argument("--branch", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--verifier", type=Path)
    parser.add_argument("--rehearsal", action="store_true")
    parser.add_argument("--require-scaffold", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    record = json.loads(args.release.read_text(encoding="utf-8"))
    if not re.fullmatch(r"v[0-9A-Za-z.-]+", record["tag"]) or identity(root, record["commit"]) != record:
        raise ValueError("Release checkout and identity must agree")
    if args.mode == "preflight":
        _, existing = remote_identity(record, args.branch, args.rehearsal)
        if args.require_scaffold and not existing:
            raise ValueError("Prepare the empty maintainer-owned rehearsal draft before signing")
        print("Remote release identity is available.")
    elif args.mode == "scaffold":
        if not args.rehearsal or os.environ.get("GITHUB_ACTIONS") or json.loads(gh("api", "user"))["login"] != "Mastervoliumpl":
            raise ValueError("Only the local maintainer CLI may prepare the rehearsal scaffold")
        tagged, existing = remote_identity(record, args.branch, True)
        if not existing:
            create_draft(record, tagged, args.release.parent / "draft-notes.md")
        print("Empty maintainer-owned rehearsal draft is ready; no assets were published.")
    else:
        if os.environ.get("GITHUB_EVENT_NAME") != "workflow_dispatch":
            raise ValueError("Draft upload requires an explicitly dispatched hosted workflow")
        upload(root, record, args.branch, args.output, args.verifier, args.rehearsal)
