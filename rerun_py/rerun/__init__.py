"""
A shim necessary to make maturin dev builds work properly.

Our maturin builds stick our package inside of a "rerun_sdk" folder
to avoid conflicting with the non-rerun "rerun" package. In released
builds, we include a rerun_sdk.pth file that makes things work properly,
but that doesn't work in dev builds where maturin generates its own
.pth file that points 1 level too high.

When we encounter this file on import, we instead redirect to the
real rerun module by adding it to the path and then, and then
replacing our own module content with it.

Additionally, we set the RERUN_CLI_PATH environment variable to point
to the rerun binary in the target directory so that we don't need to
inject it into the source tree.
"""

from __future__ import annotations

import os
import pathlib
import sys

real_path = pathlib.Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()

print(f"DEV ENVIRONMENT DETECTED! Re-importing rerun from: {real_path}", file=sys.stderr)

if "RERUN_CLI_PATH" not in os.environ:
    import rerun_bindings as bindings

    flavor = "debug" if bindings.is_dev_build() else "release"
    target_path = pathlib.Path(__file__).parent.parent.parent.joinpath(f"target/{flavor}/rerun").resolve()
    os.environ["RERUN_CLI_PATH"] = str(target_path)

sys.path.insert(0, str(real_path))

del sys.modules["rerun"]
sys.modules["rerun"] = __import__("rerun")
