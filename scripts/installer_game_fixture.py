"""Seed and verify inert deployment records for the isolated NSIS fixture."""

import hashlib
import json
import os
from pathlib import Path
import sqlite3
import sys
import uuid


def main() -> None:
    mode, evidence_arg = sys.argv[1:]
    evidence = Path(evidence_arg).resolve(strict=True)
    allowed = Path(__file__).resolve().parents[1] / "test-results/0.6.0-packaging"
    if evidence.parent != allowed or not evidence.name.startswith("install-"):
        raise ValueError("Expected an isolated installer evidence directory")
    data = (
        Path(os.environ["LOCALAPPDATA"])
        / "io.github.mastervoliumpl.starframe.installer-test"
    )
    engine = evidence / "game/engine"
    owned = {"Starframe/fixture.txt": b"owned fixture", "winhttp.dll": b"inert loader"}
    preserved = {
        "Sanctuary_Data/save.fixture": b"game save",
        "BepInEx/config/fixture.cfg": b"game settings",
    }
    database = data / "sqlite/state.db"
    if mode == "seed":
        engine.mkdir(parents=True)
        for directory in ("Sanctuary_Data/Managed", "MonoBleedingEdge"):
            (engine / directory).mkdir(parents=True)
        # Match the existing Rust game fixture without copying an executable that can run.
        pe = bytearray(128)
        pe[:2] = b"MZ"
        pe[60:64] = (64).to_bytes(4, "little")
        pe[64:70] = b"PE\0\0\x64\x86"
        pe[88:90] = b"\x0b\x02"
        (engine / "Sanctuary.exe").write_bytes(pe)
        (engine / "UnityPlayer.dll").write_bytes(pe)
        (engine / "Sanctuary_Data/app.info").write_text(
            "Enhearten Media PTY\nSanctuary\n", encoding="utf-8"
        )
        (engine / "Sanctuary_Data/boot.config").write_text(
            "build-guid=0123456789abcdef0123456789abcdef\n", encoding="utf-8"
        )
        for relative, content in {**owned, **preserved}.items():
            path = engine / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(content)
        record = {
            "version": 1,
            "owned": {p: hashlib.sha256(b).hexdigest() for p, b in owned.items()},
            "pending": None,
        }
        with sqlite3.connect(database.as_uri() + "?mode=rw", uri=True) as connection:
            # Rust canonicalize stores the Windows extended-length path in the journal.
            connection.execute(
                "INSERT INTO deployments(root, record) VALUES (?, ?)",
                ("\\\\?\\" + str(engine), json.dumps(record)),
            )
            connection.executemany(
                "INSERT OR IGNORE INTO deployment_blobs(hash, bytes) VALUES (?, ?)",
                [(hashlib.sha256(b).hexdigest(), b) for b in owned.values()],
            )
    elif mode == "interrupt":
        # State after a committed uninstall journal and its first file deletion.
        with sqlite3.connect(database.as_uri() + "?mode=rw", uri=True) as connection:
            root, value = connection.execute(
                "SELECT root, record FROM deployments"
            ).fetchone()
            record = json.loads(value)
            assert record["pending"] is None and len(record["owned"]) == 2
            record["pending"] = {
                "id": str(uuid.uuid4()), "next": {}, "borrowed": {}
            }
            connection.execute(
                "UPDATE deployments SET record=? WHERE root=?",
                (json.dumps(record), root),
            )
        (engine / "Starframe/fixture.txt").unlink()
    elif mode in ("retained", "removed"):
        for relative, content in preserved.items():
            assert (engine / relative).read_bytes() == content
        for relative, content in owned.items():
            if mode == "retained":
                assert (engine / relative).read_bytes() == content
            else:
                assert not (engine / relative).exists()
        if database.exists():
            with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
                record = json.loads(
                    connection.execute("SELECT record FROM deployments").fetchone()[0]
                )
                assert record["pending"] is None
                assert bool(record["owned"]) == (mode == "retained")
    else:
        raise ValueError("Unknown installer fixture operation")


if __name__ == "__main__":
    main()
