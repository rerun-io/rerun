from __future__ import annotations


def check_expected_wheels(wheels: list[str]) -> None:
    missing = {"windows", "macos_arm", "linux"}

    for wheel in wheels:
        if "win_amd64" in wheel:
            missing.remove("windows")
        if "macosx" in wheel and "arm64" in wheel:
            missing.remove("macos_arm")
        if "manylinux" in wheel and "x86_64" in wheel:
            missing.remove("linux")

    if len(missing) != 0:
        raise Exception(f"missing built wheels for the following platforms: {', '.join(missing)}")
