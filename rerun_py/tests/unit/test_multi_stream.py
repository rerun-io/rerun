from __future__ import annotations

import tempfile

import rerun as rr


def test_isolated_streams() -> None:
    with tempfile.TemporaryDirectory() as tmpdir:
        rec1_path = f"{tmpdir}/rec1.rrd"
        rec2_path = f"{tmpdir}/rec2.rrd"

        rec1 = rr.RecordingStream("rerun_example")
        rec1.log("/data1", rr.TextLog("Data1"))
        rec1.save(rec1_path)

        rec2 = rr.RecordingStream("rerun_example")
        rec2.log("/data2", rr.TextLog("Data2"))
        rec2.save(rec2_path)

        rec1_data = rr.dataframe.load_recording(rec1_path)
        rec2_data = rr.dataframe.load_recording(rec2_path)

        assert rec1_data.view(index="log_tick", contents="/data1").select().read_all().num_rows == 1
        assert rec2_data.view(index="log_tick", contents="/data2").select().read_all().num_rows == 1
