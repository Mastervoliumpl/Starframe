"""Verify an installed executable differs only by Tauri's NSIS bundle marker."""

from pathlib import Path
import sys


def main():
    if len(sys.argv) != 4 or sys.argv[1] != "binary":
        raise ValueError("Expected binary, source executable and installed executable")
    expected, installed = (Path(value).read_bytes() for value in sys.argv[2:])
    packaged = expected.replace(b"__TAURI_BUNDLE_TYPE_VAR_UNK", b"__TAURI_BUNDLE_TYPE_VAR_NSS")
    assert installed in (expected, packaged), "Installed executable differs beyond Tauri's NSIS marker"
    print("Installed executable matches its source build and bundle marker.")


if __name__ == "__main__":
    main()
