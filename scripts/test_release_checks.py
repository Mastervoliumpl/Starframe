import tempfile
from pathlib import Path
import unittest

from release_checks import REPOSITORY, release_notes, validate_runs


class ReleaseChecks(unittest.TestCase):
    def test_matching_changelog_is_required_and_notes_are_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            changelog = root / "CHANGELOG.md"
            changelog.write_text("## Unreleased\n\nPending\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                release_notes(root, "0.6.0-dev.1")
            changelog.write_text("## Unreleased\n\nPending\n\n## 0.6.0-dev.1 - 2026-09-16\n\nVerified change.\n\n## 0.5.0 - 2026-09-08\nOld.\n", encoding="utf-8")
            self.assertEqual(release_notes(root, "0.6.0-dev.1"), "Verified change.")
            changelog.write_text("## 0.6.0-dev.1 - 2026-09-16\n" + "x" * 65_537, encoding="utf-8")
            with self.assertRaises(ValueError):
                release_notes(root, "0.6.0-dev.1")

    def test_exact_commit_repository_branch_and_latest_success_are_required(self):
        sha = "a" * 40
        runs = [dict(name=name, head_sha=sha, head_branch="main",
                     head_repository={"full_name": REPOSITORY}, run_number=3,
                     run_attempt=1, status="completed", conclusion="success")
                for name in ("Checks", "Dependency security")]
        validate_runs(runs, sha, "main")
        for field, value in (("head_sha", "b" * 40), ("head_branch", "untrusted"),
                             ("head_repository", {"full_name": "other/repo"}),
                             ("status", "in_progress"), ("conclusion", "failure")):
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_runs([runs[0] | {field: value}, runs[1]], sha, "main")
        with self.assertRaises(ValueError):
            validate_runs(runs + [runs[0] | {"run_attempt": 2, "conclusion": "cancelled"}], sha, "main")
        with self.assertRaises(ValueError):
            validate_runs(runs[:1], sha, "main")


if __name__ == "__main__":
    unittest.main()
