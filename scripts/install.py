#!/usr/bin/env python3
"""Install forma for the current user and create native launch entries."""

import argparse
import os
import plistlib
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from installer_common import (
    is_owned_file,
    managed_targets,
    paths_for,
    read_manifest,
    sha256_file,
    system_name,
    write_manifest,
)


ROOT = Path(__file__).resolve().parents[1]
ICON = ROOT / "assets" / "forma.svg"


def install_binary(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=destination.name + ".", dir=destination.parent
    )
    os.close(descriptor)
    temporary = Path(temporary_name)
    try:
        shutil.copy2(source, temporary)
        if os.name != "nt":
            temporary.chmod(temporary.stat().st_mode | 0o111)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def create_windows_shortcut(target, shortcut):
    shortcut.parent.mkdir(parents=True, exist_ok=True)
    if shortcut.is_symlink():
        shortcut.unlink()
    env = os.environ.copy()
    env["FORMA_SHORTCUT"] = str(shortcut)
    env["FORMA_TARGET"] = str(target)
    command = (
        "$w=New-Object -ComObject WScript.Shell; "
        "$s=$w.CreateShortcut($env:FORMA_SHORTCUT); "
        "$s.TargetPath=$env:FORMA_TARGET; "
        "$s.Arguments='gui'; "
        "$s.WorkingDirectory=[IO.Path]::GetDirectoryName($env:FORMA_TARGET); "
        "$s.IconLocation=$env:FORMA_TARGET; "
        "$s.Save()"
    )
    subprocess.run(
        ["powershell", "-NoProfile", "-NonInteractive", "-Command", command],
        check=True,
        env=env,
    )


def desktop_entry(binary, icon):
    executable = str(binary).replace("\\", "\\\\").replace('"', '\\"')
    return "\n".join(
        [
            "[Desktop Entry]",
            "Version=1.0",
            "Type=Application",
            "Name=Forma",
            "Comment=Freedom-first photo and video editor",
            'Exec="{}" gui'.format(executable),
            "Icon={}".format(icon),
            "Terminal=false",
            "Categories=Graphics;Video;Photography;",
            "StartupNotify=true",
        ]
    ) + "\n"


def create_macos_bundle(paths, source):
    bundle = paths["root"]
    macos = bundle / "Contents" / "MacOS"
    resources = bundle / "Contents" / "Resources"
    macos.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    install_binary(source, paths["binary"])
    for path in (paths["launcher"], paths["info"]):
        if path.is_symlink():
            path.unlink()
    paths["launcher"].write_text(
        '#!/bin/sh\nexec "$(dirname "$0")/forma" gui "$@"\n',
        encoding="utf-8",
    )
    paths["launcher"].chmod(0o755)
    with paths["info"].open("wb") as stream:
        plistlib.dump(
            {
                "CFBundleName": "Forma",
                "CFBundleDisplayName": "Forma",
                "CFBundleIdentifier": "org.forma.editor",
                "CFBundleExecutable": "forma-launcher",
                "CFBundlePackageType": "APPL",
                "CFBundleVersion": "0.1.0",
            },
            stream,
        )


def main():
    parser = argparse.ArgumentParser(description="Install forma for the current user")
    parser.add_argument("--binary", type=Path, help="install an existing built forma binary")
    parser.add_argument(
        "--force",
        action="store_true",
        help="replace conflicting files at forma's dedicated install paths",
    )
    args = parser.parse_args()

    system = system_name()
    paths = paths_for(system)
    if paths["root"].is_symlink():
        raise RuntimeError(
            "refusing to install through a symlink at {}".format(paths["root"])
        )
    source = args.binary or (
        ROOT / "target" / "release" / ("forma.exe" if system == "Windows" else "forma")
    )
    if args.binary and not source.is_file():
        raise RuntimeError("specified forma executable was not found: {}".format(source))
    if not source.is_file():
        print("Release binary not found; building forma first.")
        subprocess.run([sys.executable, str(ROOT / "scripts" / "build.py")], check=True)
    if not source.is_file():
        raise RuntimeError("build completed but the forma executable was not found: {}".format(source))

    manifest = read_manifest(paths["manifest"])
    targets = managed_targets(system, paths)
    binary_key = "binary"
    if paths["binary"].exists() and not (
        manifest and is_owned_file(paths["binary"], manifest, binary_key)
    ) and not args.force:
        raise RuntimeError(
            "{} already exists and is not a verified forma install; use --force to replace it".format(
                paths["binary"]
            )
        )

    for key, path in targets.items():
        if key == "binary" or key == "desktop":
            continue
        if path.exists() and not (manifest and is_owned_file(path, manifest, key)) and not args.force:
            raise RuntimeError(
                "{} already exists and is not managed by forma; use --force to replace it".format(path)
            )

    files = {}
    if system == "Windows":
        install_binary(source, paths["binary"])
        for key in ("start_menu", "desktop"):
            create_windows_shortcut(paths["binary"], paths[key])
            files[key] = sha256_file(paths[key])
    elif system == "Darwin":
        desktop = paths["desktop"]
        owns_desktop = (
            manifest
            and desktop.is_symlink()
            and desktop.resolve() == paths["root"].resolve()
        )
        if desktop.parent.is_dir() and (desktop.exists() or desktop.is_symlink()):
            if not owns_desktop and not args.force:
                raise RuntimeError(
                    "{} already exists and is not forma's desktop link; use --force to replace it".format(
                        desktop
                    )
                )
            if not desktop.is_symlink():
                raise RuntimeError(
                    "refusing to replace non-symlink desktop path {}".format(desktop)
                )
        create_macos_bundle(paths, source)
        files["launcher"] = sha256_file(paths["launcher"])
        files["info"] = sha256_file(paths["info"])
        if desktop.parent.is_dir():
            if desktop.exists() or desktop.is_symlink():
                if desktop.is_symlink() or desktop.is_file():
                    desktop.unlink()
            desktop.symlink_to(paths["root"], target_is_directory=True)
    else:
        install_binary(source, paths["binary"])
        desktop_data = desktop_entry(paths["binary"], paths["icon"])
        for key in ("desktop_entry", "desktop_icon"):
            destination = paths[key]
            if key == "desktop_icon" and not destination.parent.is_dir():
                continue
            destination.parent.mkdir(parents=True, exist_ok=True)
            if destination.is_symlink():
                destination.unlink()
            destination.write_text(desktop_data, encoding="utf-8")
            destination.chmod(0o755)
            files[key] = sha256_file(destination)
        paths["icon"].parent.mkdir(parents=True, exist_ok=True)
        if paths["icon"].is_symlink():
            paths["icon"].unlink()
        shutil.copyfile(ICON, paths["icon"])
        files["icon"] = sha256_file(paths["icon"])

    files["binary"] = sha256_file(paths["binary"])
    write_manifest(paths["manifest"], system, files)
    print("Installed Forma for {} at {}".format(system, paths["root"]))
    print("Launch it from the desktop/app menu, or run: forma gui")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print("Install failed: {}".format(error), file=sys.stderr)
        sys.exit(1)
