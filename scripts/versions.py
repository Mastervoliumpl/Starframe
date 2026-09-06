"""Check or copy VERSION into the desktop manifests and root lockfile entries."""

import argparse
import json
import os
from pathlib import Path
import re
import tempfile
import tomllib


JSON_FIELDS = {
    "package.json": [("version",)],
    "package-lock.json": [("version",), ("packages", "", "version")],
    "src-tauri/tauri.conf.json": [("version",)],
}


def read_version(root: Path) -> str:
    version = (root / "VERSION").read_text(encoding="utf-8").strip()
    number = r"(?:0|[1-9][0-9]*)"
    identifier = rf"(?:{number}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
    semver = rf"{number}\.{number}\.{number}(?:-{identifier}(?:\.{identifier})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
    if not re.fullmatch(semver, version):
        raise ValueError(f"Invalid product version: {version!r}")
    return version


def update_cargo(text: str, version: str, lockfile: bool) -> str:
    # Parse TOML first; edit only the root package block to retain comments and dependencies.
    tomllib.loads(text)
    sections = re.split(r"(?m)(?=^\[)", text)
    header = "[[package]]" if lockfile else "[package]"
    matches = 0
    for index, section in enumerate(sections):
        if not section.startswith(header):
            continue
        package = tomllib.loads(section)["package"]
        if lockfile:
            package = package[0]
        if package.get("name") != "starframe" or "source" in package:
            continue
        if not isinstance(package.get("version"), str):
            raise ValueError("starframe package needs a string version")
        matches += 1
        if package["version"] == version:
            continue
        sections[index], count = re.subn(
            r"(?m)^([ \t]*version[ \t]*=[ \t]*)(?:\"[^\"\n]*\"|'[^'\n]*')",
            lambda match: match[1] + json.dumps(version),
            section,
        )
        if count != 1:
            raise ValueError("expected one literal version in the starframe package")
    if matches != 1:
        raise ValueError("expected exactly one local starframe package")
    return "".join(sections)


def version_updates(root: Path, version: str) -> dict[str, str]:
    updates = {}
    for name, fields in JSON_FIELDS.items():
        try:
            text = (root / name).read_text(encoding="utf-8")
            document = json.loads(text)
            changed = False
            for field in fields:
                parent = document
                for key in field[:-1]:
                    parent = parent[key]
                current = parent[field[-1]]
                if not isinstance(current, str):
                    raise ValueError(f"{'.'.join(field)} must be a string")
                if current != version:
                    parent[field[-1]] = version
                    changed = True
            if changed:
                updates[name] = json.dumps(document, indent=2, ensure_ascii=False) + "\n"
        except (KeyError, TypeError, ValueError) as exc:
            raise ValueError(f"{name}: {exc}") from exc

    for name in ("src-tauri/Cargo.toml", "src-tauri/Cargo.lock"):
        text = (root / name).read_text(encoding="utf-8")
        try:
            updated = update_cargo(text, version, name.endswith(".lock"))
        except ValueError as exc:
            raise ValueError(f"{name}: {exc}") from exc
        if updated != text:
            updates[name] = updated
    return updates


def synchronize(root: Path, write: bool = False) -> list[str]:
    version = read_version(root)
    updates = version_updates(root, version)
    if not write:
        return [f"{name}: product version differs from VERSION ({version})" for name in updates]
    # Preflight every file before writing. An interrupted run can be repeated from VERSION.
    for name, text in updates.items():
        path = root / name
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".version-", delete=False) as file:
            temporary = Path(file.name)
        try:
            temporary.write_text(text, encoding="utf-8", newline="\n")
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)
    return []


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="copy VERSION into all product version fields")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    try:
        errors = synchronize(root, args.write)
    except (OSError, ValueError) as exc:
        raise SystemExit(str(exc)) from exc
    if errors:
        raise SystemExit("\n".join(errors) + "\nRun python scripts/versions.py --write to synchronize.")
    print(f"Product versions {'synchronized' if args.write else 'match'}: {read_version(root)}")


if __name__ == "__main__":
    main()
