# Legacy Turso fixtures

Generated on Windows with Turso 0.7.2, default features disabled, using the fixture worker at commit cd851af. The executable was built from that commit plus documentation-only changes. No user or game data is included.

Each directory contains schema 1, 2 or 3, with either empty or populated records. The transaction fixture has committed WAL records followed by an unfinished transaction. The backup subdirectory holds a completed pre-WAL legacy backup. Expected JSON records were read by Turso before termination. Artifact payloads are inert test text. Paths under C:/Windows/Temp/starframe-legacy-011 are synthetic game selections, never inspected by the tests. SHA256.json records the retained bytes.

To regenerate, build the tests at cd851af, then run the ignored storage::sqlite_proof::legacy_worker test in a child process. Set STARFRAME_PROOF_SOURCE to a fresh fixture directory, STARFRAME_PROOF_SCHEMA to 1, 2 or 3, and STARFRAME_PROOF_MODE to empty, populated or transaction. Wait for its ready file, terminate the child, and retain its files except ready. The historical worker is the generator; Turso is not a dependency of the current application or its tests.
