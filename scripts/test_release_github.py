import json
import unittest
from unittest.mock import patch

from release_github import draft_notes, remote_identity


class GitHubReleaseChecks(unittest.TestCase):
    @patch("release_github.gh")
    def test_never_replace_a_tag_or_existing_draft(self, gh):
        record = dict(commit="a" * 40, tag="v0.6.0-dev.1")
        head = json.dumps({"object": {"sha": record["commit"]}})
        tag = dict(ref="refs/tags/" + record["tag"], object=dict(type="commit", sha=record["commit"]))
        gh.side_effect = [head, "[]", "[[]]"]
        self.assertEqual(remote_identity(record, "main"), (False, False))
        gh.side_effect = [head, json.dumps([tag]), "[[]]"]
        self.assertEqual(remote_identity(record, "main"), (True, False))
        gh.side_effect = [head, json.dumps([tag | {"object": dict(type="commit", sha="b" * 40)}])]
        with self.assertRaises(ValueError):
            remote_identity(record, "main")
        gh.side_effect = [head, "[]", json.dumps([[dict(tag_name=record["tag"], draft=True)]])]
        with self.assertRaises(ValueError):
            remote_identity(record, "main")
        gh.side_effect = [json.dumps({"object": {"sha": "b" * 40}})]
        with self.assertRaises(ValueError):
            remote_identity(record, "main")
        gh.reset_mock()
        with self.assertRaises(ValueError):
            remote_identity(record, "untrusted")
        gh.assert_not_called()

    @patch("release_github.gh")
    def test_rehearsal_only_accepts_an_empty_exact_maintainer_scaffold(self, gh):
        record = dict(commit="a" * 40, tag="v0.6.0-dev.1", notes="Reviewed")
        head = json.dumps({"object": {"sha": record["commit"]}})
        tag = dict(ref="refs/tags/" + record["tag"], object=dict(type="commit", sha=record["commit"]))
        draft = dict(tag_name=record["tag"], draft=True, assets=[], author={"login": "Mastervoliumpl"}, body=draft_notes(record))
        gh.side_effect = [head, json.dumps([tag]), json.dumps([[draft]])]
        self.assertEqual(remote_identity(record, "codex/0.6.0-windows-alpha", True), (True, True))
        for change in ({"assets": [dict(name="existing.exe")]}, {"draft": False},
                       {"author": {"login": "other"}}, {"body": "different notes"}):
            gh.side_effect = [head, json.dumps([tag]), json.dumps([[draft | change]])]
            with self.subTest(change=change), self.assertRaises(ValueError):
                remote_identity(record, "codex/0.6.0-windows-alpha", True)


if __name__ == "__main__":
    unittest.main()
