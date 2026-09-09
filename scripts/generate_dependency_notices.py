"""Collect exact notices for the Windows Rust graph and bundled JavaScript."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import tomllib


def digest(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def rust_packages(metadata: dict) -> list[dict]:
    nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    seen = set()
    while pending:
        package = pending.pop()
        if package in seen:
            continue
        seen.add(package)
        pending.extend(
            dep["pkg"] for dep in nodes[package]["deps"]
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"])
        )
    return sorted(
        (p for p in metadata["packages"] if p["id"] in seen and p["source"]),
        key=lambda p: (p["name"], p["version"]),
    )


def notice_files(directory: Path) -> list[Path]:
    pattern = re.compile(r"^(licen[cs]e|copying|copyright|notice|unlicense)([-._]|$)", re.I)
    return sorted(
        p for p in directory.rglob("*") if p.is_file()
        and (pattern.match(p.name) or "LICENSES" in p.relative_to(directory).parts)
    )


def collect(root: Path, metadata: dict, frontend_map: dict) -> dict:
    fallbacks = json.loads((root / "docs/notices/upstream-sources.json").read_text("utf-8"))
    lock = tomllib.loads((root / "src-tauri/Cargo.lock").read_text("utf-8"))
    checksums = {(p["name"], p["version"]): p["checksum"] for p in lock["package"] if "checksum" in p}
    texts = {}
    packages = []

    def retain(content: str) -> str:
        key = digest(content.encode("utf-8"))
        texts[key] = content
        return key

    def local_notices(directory: Path, source: str) -> list[dict]:
        return [dict(
            file=p.relative_to(directory).as_posix(), source=source,
            text=retain(p.read_text("utf-8-sig")),
        ) for p in notice_files(directory)]

    for package in rust_packages(metadata):
        name, version = package["name"], package["version"]
        source = f"https://static.crates.io/crates/{name}/{name}-{version}.crate"
        notices = local_notices(Path(package["manifest_path"]).parent, source)
        if not notices:
            for entry in fallbacks.get(f"{name}@{version}", []):
                if digest(entry["content"].encode("utf-8")) != entry["sha256"]:
                    raise ValueError(f"Changed upstream notice: {name}")
                notices.append(dict(file=entry["file"], source=entry["source"], text=retain(entry["content"])))
        if not notices:
            raise ValueError(f"No license/notice text for {name} {version}")
        packages.append(dict(
            ecosystem="cargo", name=name, version=version, license=package["license"],
            source=source, sourceSha256=checksums[(name, version)], notices=notices,
        ))

    bundled = set()
    for source in frontend_map["sources"]:
        if "node_modules/" in source:
            parts = source.split("node_modules/")[-1].split("/")
            bundled.add("/".join(parts[:2]) if parts[0].startswith("@") else parts[0])
    npm_lock = json.loads((root / "package-lock.json").read_text("utf-8"))["packages"]
    for name in sorted(bundled):
        package = npm_lock[f"node_modules/{name}"]
        source = package["resolved"]
        notices = local_notices(root / "node_modules" / name, source)
        if not notices:
            raise ValueError(f"No notice for bundled JavaScript: {name}")
        packages.append(dict(
            ecosystem="npm", name=name, version=package["version"],
            license=package["license"], source=source,
            sourceIntegrity=package["integrity"], notices=notices,
        ))
    return dict(
        schemaVersion=1,
        scope="Windows x64 Rust dependencies, including build dependencies; JavaScript identified in the production source map. Runtime, bootstrap, toolchain and assets have separate notices.",
        inputs={name: digest((root / name).read_text("utf-8").encode()) for name in ("src-tauri/Cargo.lock", "src-tauri/Cargo.toml", "package-lock.json", "package.json")},
        packages=packages, texts=dict(sorted(texts.items())),
    )


def render(inventory: dict) -> str:
    lines = [inventory.get("title", "Starframe desktop dependency notices"), "", inventory["scope"], "",
             "Upstream source archives are linked per package. MPL-2.0 components remain under MPL-2.0; obtain their corresponding source from those exact archives. Starframe does not restrict rights granted by the component licenses.", ""]
    for package in inventory["packages"]:
        lines.extend([f'{package["ecosystem"]}: {package["name"]} {package["version"]}',
                      f'License: {package["license"]}', f'Source: {package["source"]}'])
        for notice in package["notices"]:
            lines.append(f'Notice: {notice["file"]} [text {notice["text"]}]')
        lines.append("")
    for key, content in inventory["texts"].items():
        lines.extend([f"License/notice text {key}", "", content.rstrip(), ""])
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", required=True, type=Path)
    parser.add_argument("--frontend-map", required=True, type=Path)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[1]
    inventory = collect(repository, json.loads(args.metadata.read_text("utf-8-sig")),
                        json.loads(args.frontend_map.read_text("utf-8")))
    (repository / "docs/notices/desktop-dependencies.json").write_text(
        json.dumps(inventory, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    (repository / "docs/notices/desktop-dependencies.txt").write_text(render(inventory), encoding="utf-8")
    print(f'Collected {len(inventory["packages"])} packages and {len(inventory["texts"])} distinct notice texts.')
