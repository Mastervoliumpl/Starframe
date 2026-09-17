"""Run bounded mutation campaigns separately from ordinary checks."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def run(root: Path, seconds: int, seed: int) -> None:
    evidence = root / "test-results/fuzz" / f"{time.time_ns()}-{seed}"
    evidence.mkdir(parents=True)
    (evidence / "run.json").write_text(json.dumps(dict(
        seed=seed, requestedSeconds=seconds,
        commit=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
        workingTreeDirty=bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip()),
    ), indent=2) + "\n", encoding="utf-8")
    build = subprocess.run([
        "cargo", "test", "--manifest-path", "src-tauri/Cargo.toml", "--locked",
        "--lib", "--no-run", "--message-format=json",
    ], cwd=root, capture_output=True, text=True, timeout=900)
    (evidence / "build.log").write_text(build.stderr, encoding="utf-8")
    if build.returncode:
        raise RuntimeError("Campaign build failed; inspect the local build log")
    executable = None
    for line in build.stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "compiler-artifact" and event["target"]["kind"] == ["lib"] and event.get("executable"):
            executable = event["executable"]
    if not executable:
        raise RuntimeError("No Rust library test executable was produced")
    environment = dict(os.environ, STARFRAME_FUZZ_SECONDS=str(seconds), STARFRAME_FUZZ_SEED=str(seed), STARFRAME_FUZZ_OUTPUT=str(evidence))
    tests = ["bounded_campaign"]
    if os.name == "nt":
        tests.append("directory_replacement_race")
    for test in tests:
        with (evidence / f"{test}.log").open("w", encoding="utf-8") as log:
            result = subprocess.run([
                executable, f"packages::tests::fuzz::{test}", "--exact", "--ignored", "--nocapture",
            ], cwd=root, env=environment, stdout=log, stderr=subprocess.STDOUT,
                timeout=seconds + 30 if test == "bounded_campaign" else 30)
        if result.returncode:
            raise RuntimeError(f"{test} failed; retain the local input and log before retrying")
    report = json.loads((evidence / "campaign.json").read_text("utf-8"))
    report["windowsPathRace"] = os.name == "nt"
    report["commit"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    report["workingTreeDirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=root, text=True).strip())
    (evidence / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=int, default=60, choices=range(1, 901), metavar="1..900")
    parser.add_argument("--seed", type=int, default=47)
    args = parser.parse_args()
    if not 0 <= args.seed <= 2**64 - 1:
        parser.error("seed must fit an unsigned 64-bit integer")
    try:
        run(Path(__file__).resolve().parents[1], args.seconds, args.seed)
    except (RuntimeError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
