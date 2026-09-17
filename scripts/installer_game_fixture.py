"""Seed and verify inert deployment records for the isolated NSIS fixture."""

import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import time


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
    elif mode in ("kill-game", "kill-data"):
        directory = (
            engine / "Starframe/interruption"
            if mode == "kill-game"
            else data / "backups/interruption"
        )
        directory.mkdir()
        content = b"process interruption fixture"
        digest = hashlib.sha256(content).hexdigest()
        files = [directory / f"{index:04}.txt" for index in range(8192)]
        for path in files:
            path.write_bytes(content)
        if mode == "kill-game":
            with sqlite3.connect(database.as_uri() + "?mode=rw", uri=True) as connection:
                root, value = connection.execute(
                    "SELECT root, record FROM deployments"
                ).fetchone()
                record = json.loads(value)
                assert record["pending"] is None and len(record["owned"]) == 2
                record["owned"].update(
                    {p.relative_to(engine).as_posix(): digest for p in files}
                )
                connection.execute(
                    "UPDATE deployments SET record=? WHERE root=?",
                    (json.dumps(record), root),
                )
                connection.execute(
                    "INSERT OR IGNORE INTO deployment_blobs(hash, bytes) VALUES (?, ?)",
                    (digest, content),
                )
        application = evidence / "application"
        command = [str(application / "uninstall.exe"), "/S"]
        if mode == "kill-game":
            command.append("/KEEPDATA")
        command.append(f"_?={application}")
        process = subprocess.Popen(command, creationflags=subprocess.CREATE_NO_WINDOW)
        try:
            deadline = time.monotonic() + 30
            while (
                files[0].exists()
                and process.poll() is None
                and time.monotonic() < deadline
            ):
                time.sleep(0.005)
            if files[0].exists() or not files[-1].exists() or process.poll() is not None:
                raise AssertionError("Installer did not reach observable partial removal")
            subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                check=True, capture_output=True,
                creationflags=subprocess.CREATE_NO_WINDOW,
            )
            process.wait(timeout=10)
            remaining = sum(path.exists() for path in files)
            assert 0 < remaining < len(files), "Termination missed partial removal"
            assert (application / "starframe.exe").is_file() and database.is_file()
            with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
                record = json.loads(
                    connection.execute("SELECT record FROM deployments").fetchone()[0]
                )
                assert (record["pending"] is not None) == (mode == "kill-game")
            for relative, content in preserved.items():
                assert (engine / relative).read_bytes() == content
            print(
                f"Terminated isolated NSIS process tree during {mode}: "
                f"{len(files) - remaining} files removed, {remaining} retained; "
                "app/database preserved."
            )
        finally:
            if process.poll() is None:
                subprocess.run(
                    ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                    capture_output=True, creationflags=subprocess.CREATE_NO_WINDOW,
                )
                process.wait(timeout=10)
    elif mode in ("retained", "removed"):
        if mode == "removed":
            assert not any((engine / "Starframe/interruption").glob("*.txt"))
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
