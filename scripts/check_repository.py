"""Check the documentation repository without application dependencies."""

from pathlib import Path
import re
import subprocess
from urllib.parse import unquote, urlsplit
import xml.etree.ElementTree as ET

from versions import read_version


def check(root: Path, tracked: list[str]) -> list[str]:
    errors = []
    if any(Path(name).name.lower() == "agents.md" for name in tracked):
        errors.append("Machine-local AGENTS.md must not be tracked")

    try:
        read_version(root)
    except (OSError, ValueError) as exc:
        errors.append(str(exc))

    for name in tracked:
        path = root / name
        if path.suffix == ".svg":
            try:
                ET.parse(path)
            except ET.ParseError as exc:
                errors.append(f"{name}: invalid SVG XML: {exc}")
        if path.suffix != ".md":
            continue
        fence = None
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            marker = re.match(r"^\s{0,3}(`{3,}|~{3,})(.*)$", line)
            if marker:
                run, tail = marker.groups()
                if fence is None:
                    fence = run
                elif run[0] == fence[0] and len(run) >= len(fence) and not tail.strip():
                    fence = None
                continue
            if fence:
                continue
            # Only inline local links are checked; network availability and anchors are separate checks.
            for raw in re.findall(r"\]\((<[^>]+>|[^\s)]+)(?:\s+\"[^\"]*\")?\)", line):
                target = urlsplit(raw.strip("<>"))
                if target.scheme or target.netloc or not target.path:
                    continue
                destination = (path.parent / unquote(target.path)).resolve()
                if not destination.is_relative_to(root) or not destination.exists():
                    errors.append(f"{name}:{line_number}: missing/outside local link: {raw}")
        if fence:
            errors.append(f"{name}: unclosed fenced code block")
    return errors


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    tracked = subprocess.check_output(
        ["git", "ls-files", "-z"], cwd=root
    ).decode("utf-8").rstrip("\0").split("\0")
    errors = check(root, tracked)
    if errors:
        raise SystemExit("\n".join(errors))
    print("Repository checks passed: version, local links, fences, SVG XML and local-instruction exclusion.")
