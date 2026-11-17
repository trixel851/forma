#!/usr/bin/env python3
"""Uninstall only files recorded and verified by forma's per-user installer."""

import argparse
import os
import sys

from installer_common import (
    is_owned_file,
    managed_targets,
    paths_for,
    read_manifest,
    system_name,
)


def remove_file_if_owned(key, path, manifest):
    if not path.exists() and not path.is_symlink():
        return
    if key == "desktop" and manifest.get("system") == "Darwin":
        bundle = paths_for("Darwin")["root"]
        if path.is_symlink() and path.resolve() == bundle.resolve():
            path.unlink()
        return
    if is_owned_file(path, manifest, key):
        path.unlink()
    else:
        print("Preserving modified or unmanaged file: {}".format(path))


def remove_empty_directories(system, paths):
    if system == "Windows":
        directories = [paths["root"]]
    elif system == "Darwin":
        directories = [
            paths["root"] / "Contents" / "Resources",
            paths["root"] / "Contents" / "MacOS",
            paths["root"] / "Contents",
            paths["root"],
        ]
    else:
        directories = [
            paths["root"],
            paths["desktop_entry"].parent,
            paths["icon"].parent,
            paths["icon"].parent.parent,
            paths["icon"].parent.parent.parent,
        ]
    for directory in directories:
        try:
            directory.rmdir()
        except OSError:
            pass


def main():
    parser = argparse.ArgumentParser(description="Uninstall forma for the current user")
    parser.parse_args()
    system = system_name()
    paths = paths_for(system)
    manifest = read_manifest(paths["manifest"])
    if not manifest or manifest.get("system") != system:
        print("No verified Forma installation was found for this user.", file=sys.stderr)
        return 1
    if not is_owned_file(paths["binary"], manifest, "binary"):
        print(
            "The installed executable is missing or has changed; refusing to remove files.",
            file=sys.stderr,
        )
        return 1

    for key, path in managed_targets(system, paths).items():
        remove_file_if_owned(key, path, manifest)
    paths["manifest"].unlink()
    remove_empty_directories(system, paths)
    print("Uninstalled Forma for {}.".format(system))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except OSError as error:
        print("Uninstall failed: {}".format(error), file=sys.stderr)
        sys.exit(1)
