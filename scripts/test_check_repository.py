from pathlib import Path
import tempfile
import unittest

from check_repository import check


class RepositoryChecks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="starframe-checks-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.tracked = ["VERSION", "README.md", "diagram.svg"]
        (self.root / "VERSION").write_text("0.1.0-dev.1", encoding="utf-8")
        (self.root / "README.md").write_text(
            "[Version](VERSION)\n```text\n[Example](missing)\n```\n",
            encoding="utf-8",
        )
        (self.root / "diagram.svg").write_text(
            '<svg xmlns="http://www.w3.org/2000/svg"/>', encoding="utf-8"
        )

    def test_valid_repository_and_code_examples(self):
        self.assertEqual(check(self.root, self.tracked), [])

    def test_version_contract(self):
        for version, valid in [
            ("0.0.0", True),
            ("0.2.0-rc.1+build.01", True),
            ("0.2.0-01", False),
            ("01.2.0", False),
            ("0.2", False),
        ]:
            with self.subTest(version=version):
                (self.root / "VERSION").write_text(version, encoding="utf-8")
                self.assertEqual(not check(self.root, self.tracked), valid)

    def test_missing_links_and_unclosed_fences_fail(self):
        (self.root / "README.md").write_text(
            "[Broken](missing.md)\n```text\n", encoding="utf-8"
        )
        errors = check(self.root, self.tracked)
        self.assertTrue(any("missing/outside local link" in error for error in errors))
        self.assertTrue(any("unclosed fenced" in error for error in errors))

    def test_malformed_svg_fails(self):
        (self.root / "diagram.svg").write_text("<svg>", encoding="utf-8")
        self.assertTrue(any("invalid SVG XML" in error for error in check(self.root, self.tracked)))

    def test_local_instructions_cannot_be_tracked(self):
        (self.root / "AGENTS.md").write_text("local instructions", encoding="utf-8")
        errors = check(self.root, self.tracked + ["AGENTS.md"])
        self.assertTrue(any("must not be tracked" in error for error in errors))

    def test_personal_home_paths_fail_in_text_and_code_examples(self):
        paths = [
            "/".join(["C:", "Users", "example-person", "project"]),
            "\\".join(["C:", "Users", "example-person", "project"]),
            "\\\\".join(["C:", "Users", "example-person", "project"]),
            "/".join(["", "home", "example-person", "project"]),
            "/".join(["", "Users", "example-person", "project"]),
            "/".join(["file:", "", "", "home", "example-person", "project"]),
            "%2F".join(["C%3A", "Users", "example-person", "project"]),
        ]
        for name in ["README.md", "fixture.json", "script.ps1"]:
            for value in paths:
                with self.subTest(name=name, value=value):
                    (self.root / name).write_text(f"```text\n{value}\n```\n", encoding="utf-8")
                    errors = check(self.root, ["VERSION", name])
                    self.assertTrue(any("personal home-directory path" in error for error in errors))
                    self.assertTrue(all("example-person" not in error for error in errors))

    def test_portable_paths_and_binary_assets_are_allowed(self):
        (self.root / "README.md").write_text(
            "Use $env:USERPROFILE, %LOCALAPPDATA%, ~/project or path/to/project.\n"
            "https://api.github.com/users/example-person\n",
            encoding="utf-8",
        )
        (self.root / "asset.png").write_bytes(b"\x89PNG\r\n\x1a\n")
        self.assertEqual(check(self.root, self.tracked + ["asset.png"]), [])

    def test_invalid_markdown_encoding_fails(self):
        (self.root / "README.md").write_bytes(b"\xff")
        self.assertTrue(any("UTF-8" in error for error in check(self.root, self.tracked)))


if __name__ == "__main__":
    unittest.main()
