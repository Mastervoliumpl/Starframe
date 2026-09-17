import copy
import json
from pathlib import Path
import tempfile
import unittest

from check_dependency_notices import check
from generate_dependency_notices import digest, render, rust_packages


class DependencyNotices(unittest.TestCase):
    def test_graph_excludes_dev_dependencies_and_handles_shared_nodes(self):
        def dependency(name, kind=None):
            return dict(pkg=name, dep_kinds=[dict(kind=kind)])

        metadata = dict(
            packages=[dict(id=n, name=n, version="1", source="registry") for n in ("root", "runtime", "build", "dev")],
            resolve=dict(root="root", nodes=[
                dict(id="root", deps=[dependency("runtime"), dependency("build", "build"), dependency("dev", "dev")]),
                dict(id="runtime", deps=[dependency("build")]),
                dict(id="build", deps=[]), dict(id="dev", deps=[]),
            ]),
        )
        metadata["packages"][0]["source"] = None
        self.assertEqual([p["name"] for p in rust_packages(metadata)], ["build", "runtime"])

    def test_packaging_rejects_stale_notices_changed_text_and_runtime_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            integration = root / "integration"
            integration.mkdir()
            (integration / "runtime.dll").write_bytes(b"verified DLL")
            directory = root / "docs/notices"
            directory.mkdir(parents=True)
            originals = {}
            for kind, names in (
                ("desktop", ("src-tauri/Cargo.lock", "src-tauri/Cargo.toml", "package-lock.json", "package.json")),
                ("runtime", ("runtime/bootstrap.json", "runtime/Starframe.Runtime/packages.lock.json")),
            ):
                for name in names:
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_text("locked input\n", encoding="utf-8")
                key = digest(b"License text")
                originals[kind] = dict(
                    schemaVersion=1, scope="fixture", inputs={n: digest(b"locked input\n") for n in names},
                    texts={key: "License text"}, packages=[dict(
                        ecosystem=kind, name="fixture", version="1", license="MIT", source="https://example.invalid/source",
                        files={"runtime.dll": digest(b"verified DLL")} if kind == "runtime" else {},
                        notices=[dict(file="LICENSE", text=key)],
                    )],
                )

            def write(kind, inventory):
                (directory / f"{kind}-dependencies.json").write_text(json.dumps(inventory), encoding="utf-8")
                (directory / f"{kind}-dependencies.txt").write_text(render(inventory), encoding="utf-8")

            for kind, inventory in originals.items():
                write(kind, inventory)
            check(root, integration)
            (root / "src-tauri/Cargo.lock").write_text("changed", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "Refresh dependency notices"):
                check(root, integration)
            (root / "src-tauri/Cargo.lock").write_text("locked input\n", encoding="utf-8")
            changed = copy.deepcopy(originals["desktop"])
            changed["texts"][key] = "altered license"
            write("desktop", changed)
            with self.assertRaisesRegex(ValueError, "text changed"):
                check(root, integration)
            write("desktop", originals["desktop"])
            (integration / "runtime.dll").write_bytes(b"replacement DLL")
            with self.assertRaisesRegex(ValueError, "Runtime bytes differ"):
                check(root, integration)


if __name__ == "__main__":
    unittest.main()
