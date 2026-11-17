import hashlib
import json
import os
import platform
from pathlib import Path


APP_ID = "forma-user-install-v1"


def system_name():
    system = platform.system()
    if system not in ("Windows", "Darwin", "Linux"):
        raise RuntimeError(
            "unsupported operating system {!r}; supported systems are Windows, macOS, and Linux".format(
                system
            )
        )
    return system


def paths_for(system=None):
    system = system or system_name()
    home = Path.home()
    if system == "Windows":
        local = Path(os.environ.get("LOCALAPPDATA", home / "AppData/Local"))
        root = local / "Programs" / "Forma"
        return {
            "root": root,
            "binary": root / "forma.exe",
            "manifest": root / ".forma-install.json",
            "start_menu": local / "Programs" / "Forma" / "Forma.lnk",
            "desktop": home / "Desktop" / "Forma.lnk",
        }
    if system == "Darwin":
        bundle = home / "Applications" / "Forma.app"
        return {
            "root": bundle,
            "binary": bundle / "Contents" / "MacOS" / "forma",
            "launcher": bundle / "Contents" / "MacOS" / "forma-launcher",
            "info": bundle / "Contents" / "Info.plist",
            "manifest": bundle / "Contents" / "Resources" / ".forma-install.json",
            "desktop": home / "Desktop" / "Forma.app",
        }

    data_home = Path(os.environ.get("XDG_DATA_HOME", home / ".local/share"))
    local_bin = home / ".local/bin"
    return {
        "root": data_home / "forma",
        "binary": local_bin / "forma",
        "manifest": data_home / "forma" / "install.json",
        "desktop_entry": data_home / "applications" / "forma.desktop",
        "desktop_icon": home / "Desktop" / "forma.desktop",
        "icon": data_home / "icons" / "hicolor" / "scalable" / "apps" / "forma.svg",
    }


def managed_targets(system, paths):
    keys = {
        "Windows": ("binary", "start_menu", "desktop"),
        "Darwin": ("binary", "launcher", "info", "desktop"),
        "Linux": ("binary", "desktop_entry", "desktop_icon", "icon"),
    }[system]
    return {key: paths[key] for key in keys}


def sha256_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_manifest(path):
    try:
        manifest = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if (
        not isinstance(manifest, dict)
        or manifest.get("app_id") != APP_ID
        or not isinstance(manifest.get("files"), dict)
    ):
        return None
    return manifest


def is_owned_file(path, manifest, key):
    if Path(path).is_symlink():
        return False
    expected = manifest.get("files", {}).get(key) if manifest else None
    try:
        return bool(expected) and sha256_file(path) == expected
    except OSError:
        return False


def write_manifest(path, system, files):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(
            {"app_id": APP_ID, "system": system, "files": files},
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
