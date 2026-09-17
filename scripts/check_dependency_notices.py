"""Reject stale, incomplete or changed dependency notices before packaging."""

import json
from pathlib import Path

from generate_dependency_notices import digest, render


def check(root: Path, integration: Path) -> None:
    for kind, inputs in (
        ("desktop", {"src-tauri/Cargo.lock", "src-tauri/Cargo.toml", "package-lock.json", "package.json"}),
        ("runtime", {"runtime/bootstrap.json", "runtime/Starframe.Runtime/packages.lock.json"}),
    ):
        path = root / f"docs/notices/{kind}-dependencies.json"
        inventory = json.loads(path.read_text("utf-8"))
        if inventory["schemaVersion"] != 1 or set(inventory["inputs"]) != inputs or not inventory["packages"]:
            raise ValueError(f"Incomplete {kind} notice inventory")
        for name, expected in inventory["inputs"].items():
            if digest((root / name).read_text("utf-8").encode()) != expected:
                raise ValueError(f"Refresh dependency notices after changing {name}")
        for key, value in inventory["texts"].items():
            if digest(value.encode("utf-8")) != key:
                raise ValueError("Dependency notice text changed")
        for package in inventory["packages"]:
            if not package["notices"]:
                raise ValueError(f'Missing notices: {package["name"]}')
            for notice in package["notices"]:
                if notice["text"] not in inventory["texts"]:
                    raise ValueError("Missing dependency notice text")
            for name, expected in package.get("files", {}).items():
                path = (integration / name).resolve()
                if not path.is_relative_to(integration.resolve()) or digest(path.read_bytes()) != expected:
                    raise ValueError(f"Runtime bytes differ from notice inventory: {name}")
        if (root / f"docs/notices/{kind}-dependencies.txt").read_text("utf-8") != render(inventory):
            raise ValueError(f"Rendered {kind} notices differ from the inventory")


if __name__ == "__main__":
    repository = Path(__file__).resolve().parents[1]
    check(repository, repository / "src-tauri/target/installer/integration")
    print("Dependency notice inputs, texts and runtime identities verified.")
