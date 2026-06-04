#!/usr/bin/env python3
"""Assemble a macOS `.app` bundle around the rerun-cli binary.

Produces `<output>/Rerun.app/` with:
    Contents/
        Info.plist            (with __VERSION__ substituted)
        MacOS/rerun           (the binary, executable bit set)
        Resources/Rerun.icns  (multi-resolution icns derived from the PNG)

The bundle is what gives macOS the right dock label ("Rerun" instead of "rerun"),
proper "About Rerun" menu, and a hook for future file associations.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

# Apple requires CFBundleShortVersionString to be three integers separated by periods.
# Map e.g. "0.33.0-alpha.1+dev" → "0.33.0".
_VERSION_PREFIX = re.compile(r"^(\d+\.\d+\.\d+)")

ICNS_SIZES = [
    (16, "icon_16x16.png"),
    (32, "icon_16x16@2x.png"),
    (32, "icon_32x32.png"),
    (64, "icon_32x32@2x.png"),
    (128, "icon_128x128.png"),
    (256, "icon_128x128@2x.png"),
    (256, "icon_256x256.png"),
    (512, "icon_256x256@2x.png"),
    (512, "icon_512x512.png"),
    (1024, "icon_512x512@2x.png"),
]


def run(args: list[str]) -> None:
    print(f"> {' '.join(args)}", flush=True)
    subprocess.run(args, check=True)


def build_icns(png: Path, out: Path) -> None:
    """Build a multi-resolution .icns from a square source PNG using macOS native tools."""
    iconset = out.parent / f"{out.stem}.iconset"
    if iconset.exists():
        shutil.rmtree(iconset)
    iconset.mkdir(parents=True)
    for size, name in ICNS_SIZES:
        run(["sips", "-z", str(size), str(size), str(png), "--out", str(iconset / name)])
    run(["iconutil", "--convert", "icns", "--output", str(out), str(iconset)])
    shutil.rmtree(iconset)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path, help="Path to the rerun-cli binary")
    parser.add_argument("--icon", required=True, type=Path, help="Path to the source square PNG icon")
    parser.add_argument("--info-plist", required=True, type=Path, help="Path to the Info.plist template")
    parser.add_argument("--version", required=True, help="Version string (e.g. 0.21.0)")
    parser.add_argument("--output-dir", required=True, type=Path, help="Directory to write Rerun.app into")
    args = parser.parse_args()

    if sys.platform != "darwin":
        print("error: bundle_macos_app.py must run on macOS (uses sips and iconutil)", file=sys.stderr)
        return 1

    for path in [args.binary, args.icon, args.info_plist]:
        if not path.exists():
            print(f"error: {path} does not exist", file=sys.stderr)
            return 1

    app = args.output_dir / "Rerun.app"
    if app.exists():
        shutil.rmtree(app)
    macos_dir = app / "Contents" / "MacOS"
    resources_dir = app / "Contents" / "Resources"
    macos_dir.mkdir(parents=True)
    resources_dir.mkdir(parents=True)

    # Binary — named with a capital R inside the bundle so macOS's
    # NSProcessInfo.processName resolves to "Rerun", which winit then uses to
    # build the app menu items ("About Rerun", "Hide Rerun", "Quit Rerun").
    binary_dst = macos_dir / "Rerun"
    shutil.copy2(args.binary, binary_dst)
    binary_dst.chmod(0o755)

    # Info.plist with version substituted (sanitized to Apple's x.y.z form)
    match = _VERSION_PREFIX.match(args.version)
    short_version = match.group(1) if match else "0.0.0"
    plist_text = args.info_plist.read_text(encoding="utf-8").replace("__VERSION__", short_version)
    (app / "Contents" / "Info.plist").write_text(plist_text, encoding="utf-8")

    # Icon
    build_icns(args.icon, resources_dir / "Rerun.icns")

    print(f"Wrote {app}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
