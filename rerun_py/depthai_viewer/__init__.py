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
"""
import pathlib
import sys

real_path = pathlib.Path(__file__).parent.parent.joinpath("rerun_sdk").resolve()

print(f"DEV ENVIRONMENT DETECTED! Re-importing rerun from: {real_path}", file=sys.stderr)

sys.path.insert(0, str(real_path))

del sys.modules["depthai_viewer"]
sys.modules["depthai_viewer"] = __import__("depthai_viewer")
