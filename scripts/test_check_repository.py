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


if __name__ == "__main__":
    unittest.main()
