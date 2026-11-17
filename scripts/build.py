#!/usr/bin/env python3
"""Build forma for the host operating system."""

import argparse
import platform
import shutil
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description="Build forma for this computer")
    parser.add_argument(
        "--no-gui",
        action="store_true",
        help="build only the CLI (skip native desktop GUI dependencies)",
    )
    parser.add_argument(
        "--debug",
        action="store_true",
        help="build a debug binary instead of a release binary",
    )
    args = parser.parse_args()

    system = platform.system()
    if system not in ("Windows", "Darwin", "Linux"):
        parser.error(
            "unsupported operating system {!r}; supported systems are Windows, macOS, and Linux".format(
                system
            )
        )

    cargo = shutil.which("cargo")
    if cargo is None:
        print("Error: Rust/Cargo is not installed or is not on PATH.", file=sys.stderr)
        return 1

    command = [cargo, "build"]
    if not args.debug:
        command.append("--release")
    if not args.no_gui:
        command.extend(["--features", "gui"])

    os_name = {"Darwin": "macOS"}.get(system, system)
    print("Detected {} ({}); building forma...".format(os_name, platform.machine()))
    subprocess.run(command, cwd=ROOT, check=True)

    executable = ROOT / "target" / ("debug" if args.debug else "release") / "forma"
    if system == "Windows":
        executable = executable.with_suffix(".exe")
    print("Build complete: {}".format(executable))
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except subprocess.CalledProcessError as error:
        sys.exit(error.returncode or 1)
