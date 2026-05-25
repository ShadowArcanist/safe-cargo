#!/usr/bin/env python3
"""Safely extract a crate tarball into a destination directory."""

import os
import pathlib
import sys
import tarfile


def is_within_directory(base: pathlib.Path, target: pathlib.Path) -> bool:
    try:
        target.resolve().relative_to(base.resolve())
        return True
    except ValueError:
        return False


def validate_member(member: tarfile.TarInfo, dest: pathlib.Path) -> None:
    member_path = pathlib.PurePosixPath(member.name)
    if member_path.is_absolute() or ".." in member_path.parts:
        raise ValueError(f"unsafe archive path: {member.name}")

    if member.issym() or member.islnk():
        raise ValueError(f"archive links are not allowed: {member.name}")

    target = dest / pathlib.Path(*member_path.parts)
    if not is_within_directory(dest, target):
        raise ValueError(f"archive member escapes destination: {member.name}")


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: safe_extract.py <archive.tar.gz> <dest>", file=sys.stderr)
        return 2

    archive = pathlib.Path(sys.argv[1])
    dest = pathlib.Path(sys.argv[2])
    dest.mkdir(parents=True, exist_ok=True)

    try:
        with tarfile.open(archive, "r:gz") as tar:
            members = tar.getmembers()
            for member in members:
                validate_member(member, dest)
            tar.extractall(dest, members=members, filter="data")
    except (tarfile.TarError, ValueError) as exc:
        print(f"safe extraction failed: {exc}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
