#!/usr/bin/env python3
"""Bump the app version everywhere it is declared.

    python3 scripts/bump-version.py          # 0.1.0 -> 0.1.1 (patch)
    python3 scripts/bump-version.py 0.2.0    # set an explicit version

The version lives once in the root Cargo.toml (`[workspace.package]`); the
workspace crates (`halo`, `halo-backend`) inherit it, and their entries in
Cargo.lock are updated to match. Edits are plain text replacements so each
file keeps its formatting. Prints the new version on stdout.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VERSION_RE = re.compile(r"^\d+\.\d+\.\d+$")
WS_RE = re.compile(r'(\[workspace\.package\][^\[]*?\nversion\s*=\s*")([^"]+)(")', re.S)


def current() -> str:
    m = WS_RE.search((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    if not m:
        sys.exit("version not found in [workspace.package] of Cargo.toml")
    return m.group(2)


def patch_bump(v: str) -> str:
    if not VERSION_RE.match(v):
        sys.exit(f'current version "{v}" is not major.minor.patch')
    major, minor, patch = v.split(".")
    return f"{major}.{minor}.{int(patch) + 1}"


def main() -> None:
    nxt = sys.argv[1] if len(sys.argv) > 1 else patch_bump(current())
    if not VERSION_RE.match(nxt):
        sys.exit(f'"{nxt}" is not major.minor.patch')

    cargo = ROOT / "Cargo.toml"
    text = cargo.read_text(encoding="utf-8")
    cargo.write_text(WS_RE.sub(lambda m: m.group(1) + nxt + m.group(3), text, count=1), encoding="utf-8")

    lock = ROOT / "Cargo.lock"
    text = lock.read_text(encoding="utf-8")
    for name in ("halo", "halo-backend"):
        pat = re.compile(r'(\[\[package\]\]\r?\nname = "' + re.escape(name) + r'"\r?\nversion = ")[^"]+(")')
        if not pat.search(text):
            sys.exit(f"{name} not found in Cargo.lock")
        text = pat.sub(lambda m: m.group(1) + nxt + m.group(2), text, count=1)
    lock.write_text(text, encoding="utf-8")

    print(nxt)


if __name__ == "__main__":
    main()
