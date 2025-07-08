from __future__ import annotations

import tempfile

import rerun as rr


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


def test_isolated_streams() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rec1_path = f"{tmpdir}/rec1.rrd"
        rec2_path = f"{tmpdir}/rec2.rrd"

        rec1 = rr.RecordingStream("rerun_example_multi_stream")
        rec1.log("/data1", rr.TextLog("Data1"))
        rec1.save(rec1_path)

        rec2 = rr.RecordingStream("rerun_example_multi_stream")
        rec2.log("/data2", rr.TextLog("Data2"))
        rec2.save(rec2_path)

        rec1_data = rr.dataframe.load_recording(rec1_path)
        rec2_data = rr.dataframe.load_recording(rec2_path)

        assert rec1_data.view(index="log_tick", contents="/data1").select().read_all().num_rows == 1
        assert rec2_data.view(index="log_tick", contents="/data2").select().read_all().num_rows == 1
