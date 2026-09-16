import json
import unittest
from unittest.mock import patch

from release_github import remote_identity


class GitHubReleaseChecks(unittest.TestCase):
    @patch("release_github.gh")
    def test_never_replace_a_tag_or_existing_draft(self, gh):
        record = dict(commit="a" * 40, tag="v0.6.0-dev.1")
        head = json.dumps({"object": {"sha": record["commit"]}})
        tag = dict(ref="refs/tags/" + record["tag"], object=dict(type="commit", sha=record["commit"]))
        gh.side_effect = [head, "[]", "[[]]"]
        self.assertFalse(remote_identity(record, "main"))
        gh.side_effect = [head, json.dumps([tag]), "[[]]"]
        self.assertTrue(remote_identity(record, "main"))
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


if __name__ == "__main__":
    unittest.main()
