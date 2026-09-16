"""Check populated schema-12 data across two installed application builds."""

import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys


def records(connection):
    tables = connection.execute(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).fetchall()
    return {
        name: sorted(
            [list(row) for row in connection.execute('SELECT * FROM "' + name.replace('"', '""') + '"')],
            key=repr,
        )
        for (name,) in tables
        if name != "catalog_security"
    }


def main():
    if sys.argv[1] == "binary":
        expected, installed = (Path(value).read_bytes() for value in sys.argv[2:])
        packaged = expected.replace(b"__TAURI_BUNDLE_TYPE_VAR_UNK", b"__TAURI_BUNDLE_TYPE_VAR_NSS")
        assert installed in (expected, packaged), "Installed executable differs beyond Tauri's NSIS marker"
        print("Installed executable matches its source build and bundle marker.")
        return
    mode, evidence_arg = sys.argv[1:]
    evidence = Path(evidence_arg).resolve(strict=True)
    allowed = Path(__file__).resolve().parents[1] / "test-results/0.6.0-packaging"
    if evidence.parent != allowed or not evidence.name.startswith("install-"):
        raise ValueError("Expected an isolated installer evidence directory")
    data = Path(os.environ["LOCALAPPDATA"]) / "io.github.mastervoliumpl.starframe.installer-test"
    database = data / "sqlite/state.db"
    snapshot = evidence / "upgrade-records.json"
    payload = b"inert cross-build upgrade payload"
    digest = hashlib.sha256(payload).hexdigest()
    artifact = data / "artifacts" / digest / "fixture.dll"
    source = evidence / "original-source.dll"
    active_id = "11111111-1111-4111-8111-111111111111"
    empty_id = "22222222-2222-4222-8222-222222222222"
    with sqlite3.connect(database.as_uri() + "?mode=rw", uri=True) as connection:
        schema = connection.execute("PRAGMA user_version").fetchone()[0]
        if mode == "seed":
            assert schema == 12, "The older installed executable must create schema 12"
            for mod_id, origin, release_id in (
                ("upgrade.catalog", "catalog", "upgrade-release"),
                ("upgrade.local", "local_import", None),
            ):
                connection.execute(
                    "INSERT INTO library VALUES (?, ?, ?, ?, ?, ?, ?)",
                    (mod_id, digest, mod_id, "Fixture author", "1.0.0", origin, release_id),
                )
            connection.executemany(
                "INSERT INTO collections VALUES (?, ?, ?)",
                [(active_id, "Upgrade collection", 3), (empty_id, "Empty collection", 1)],
            )
            connection.executemany(
                "INSERT INTO collection_entries VALUES (?, ?, ?, ?, ?, ?)",
                [
                    (active_id, 0, "upgrade.local", digest, "local_import", None),
                    (active_id, 1, "upgrade.catalog", digest, "catalog", "upgrade-release"),
                ],
            )
            connection.execute("UPDATE preferences SET active_collection=? WHERE id=1", (active_id,))
            connection.execute("UPDATE metadata SET revision=7 WHERE id=1")
            artifact.parent.mkdir(parents=True)
            artifact.write_bytes(payload)
            source.write_bytes(payload)
            snapshot.write_text(json.dumps(records(connection), sort_keys=True), encoding="utf-8")
        elif mode == "verify":
            assert schema == 14, "The new installed executable must migrate to schema 14"
            expected = json.loads(snapshot.read_text(encoding="utf-8"))
            assert records(connection) == expected, "Upgrade changed existing records"
            assert connection.execute("SELECT count(*) FROM catalog_security").fetchone()[0] == 0
            backups = list((data / "backups").glob("*/state.db"))
            assert backups, "Migration did not retain a database backup"
            for backup in backups:
                with sqlite3.connect(backup.as_uri() + "?mode=ro", uri=True) as previous:
                    if previous.execute("PRAGMA user_version").fetchone()[0] == 12:
                        assert records(previous) == expected
                        assert (backup.parent / "complete").is_file()
                        break
            else:
                raise AssertionError("No complete schema-12 backup")
            assert artifact.read_bytes() == source.read_bytes() == payload
        else:
            raise ValueError("Expected seed or verify")
    print(f"Cross-build populated-data {mode} passed.")


if __name__ == "__main__":
    main()
