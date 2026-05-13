"""Compact an existing MCAP recording in-process via the Chunk Processing API."""

from __future__ import annotations

from pathlib import Path

from rerun.experimental import McapReader, OptimizationProfile

mcap_path = Path(__file__).resolve().parents[4] / "tests" / "assets" / "mcap" / "trossen_transfer_cube.mcap"
output_path = Path("trossen_compacted.rrd")

# region: optimize
(
    McapReader(mcap_path)
    .stream()
    .collect(optimize=OptimizationProfile.OBJECT_STORE)
    .write_rrd(output_path, application_id="rerun_example_optimize", recording_id=mcap_path.stem)
)
# endregion: optimize
