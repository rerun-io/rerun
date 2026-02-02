from __future__ import annotations

import platform
import tempfile
from typing import TYPE_CHECKING

import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path


def test_init_twice() -> None:
    """Regression test for #9948: creating the same recording twice caused hangs in the past (should instead warn)."""
    # Always set strict mode to false in case it leaked from another test
    rr.set_strict_mode(False)

    # Using default recording id
    rr.init("rerun_example_test_app_id")
    recording_id = rr.get_recording_id()

    rr.init("rerun_example_test_app_id")
    assert recording_id == rr.get_recording_id()

    # Using a custom recording id
    recording_id = "test_recording_id"
    rr.init("rerun_example_test_app_id", recording_id=recording_id)
    rr.init("rerun_example_test_app_id", recording_id=recording_id)
    assert recording_id == rr.get_recording_id()


def test_recording_stream_twice() -> None:
    """For `RecordingStream`, the default should be to have new, unique recording ids."""

    with rr.RecordingStream("rerun_example_bug") as rec:
        id1 = rec.get_recording_id()

    with rr.RecordingStream("rerun_example_bug") as rec:
        id2 = rec.get_recording_id()

    assert id1 != id2


def test_isolated_streams(tmp_path: Path) -> None:
    rec1_path = f"{tmp_path}/rec1.rrd"
    rec2_path = f"{tmp_path}/rec2.rrd"

    rec1 = rr.RecordingStream("rerun_example_multi_stream", recording_id="rec1")
    rec1.log("/data1", rr.TextLog("Data1"))
    rec1.save(rec1_path)

    rec2 = rr.RecordingStream("rerun_example_multi_stream", recording_id="rec2")
    rec2.log("/data2", rr.TextLog("Data2"))
    rec2.save(rec2_path)

    server = rr.server.Server(datasets={"test_dataset": tmp_path})
    ds = server.client().get_dataset("test_dataset")
    assert ds.filter_segments("rec1").filter_contents("/data1").reader(index="log_tick").count() == 1
    assert ds.filter_segments("rec2").filter_contents("/data2").reader(index="log_tick").count() == 1
    assert (
        ds.filter_segments(["rec1", "rec2"]).filter_contents(["/data1", "/data2"]).reader(index="log_tick").count() == 2
    )


def test_cleanup_reinit() -> None:
    system = platform.system()
    if system == "Linux":
        import os

        def is_file_open(file_path: str) -> bool:
            file_path = os.path.realpath(file_path)

            fd_dir = "/proc/self/fd"
            if not os.path.isdir(fd_dir):
                return False  # Only works on platforms with /proc

            try:
                for fd in os.listdir(fd_dir):
                    try:
                        target = os.readlink(os.path.join(fd_dir, fd))
                        if os.path.samefile(file_path, target):
                            return True
                    except (FileNotFoundError, PermissionError, OSError):
                        continue
            except Exception:
                pass

            return False

        with tempfile.TemporaryDirectory() as tmpdir:
            rec_path = f"{tmpdir}/my_rec.rrd"
            rr.init("rerun_example_reinit")
            rr.save(rec_path)
            rr.log("/data1", rr.TextLog("Data1"))

            assert is_file_open(rec_path), "Recording file should be open after saving"

            rr.init("rerun_example_reinit")

            assert not is_file_open(rec_path), "Recording file should be closed after calling rr.init"
