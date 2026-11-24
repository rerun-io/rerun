"""Convert each episode of a LeRobot dataset to its own `.rrd` file."""

import tempfile
from pathlib import Path

import rerun as rr

dataset_path = (
    Path(__file__).resolve().parents[4]
    / "crates"
    / "data_flow"
    / "re_importer"
    / "tests"
    / "assets"
    / "lerobot"
    / "v30_apple_storage"
)
out_dir = Path(tempfile.mkdtemp()) / "rrd"

# region: convert
out_dir.mkdir(exist_ok=True)
profile = rr.chunk.OptimizationProfile.OBJECT_STORE

reader = rr.experimental.LeRobotReader(dataset_path)
for episode in reader.episodes():
    reader.stream(episode).collect(optimize=profile).write_rrd(
        out_dir / f"episode_{episode}.rrd",
        application_id="rerun_example_lerobot_to_rrd",
        recording_id=f"episode_{episode}",
    )
# endregion: convert

assert any(out_dir.glob("episode_*.rrd"))
