from __future__ import annotations

from pathlib import Path

import rerun as rr
import semver
import tomli


def test_version() -> None:
    cargo_toml_path = Path(__file__).parent.parent.parent.parent / "Cargo.toml"
    # ensure Cargo.toml file is loaded as UTF-8 (this can fail on Windows otherwise)
    cargo_toml = tomli.loads(cargo_toml_path.read_text(encoding="utf-8"))
    assert rr.__version__ == cargo_toml["workspace"]["package"]["version"]

    ver = semver.VersionInfo.parse(rr.__version__)

    assert len(rr.__version_info__) == 4

    assert ver.major == rr.__version_info__[0]
    assert ver.minor == rr.__version_info__[1]
    assert ver.patch == rr.__version_info__[2]

    if ver.prerelease:
        assert ver.prerelease == rr.__version_info__[3]
    else:
        # The last field is `None` if there is no prerelease.
        assert rr.__version_info__[3] is None

    assert rr.__version__ in rr.version()


if __name__ == "__main__":
    test_version()
