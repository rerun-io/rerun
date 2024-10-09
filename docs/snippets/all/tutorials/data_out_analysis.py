from __future__ import annotations

import numpy as np
import numpy.typing as npt
import pandas as pd
import rerun as rr
import matplotlib.pyplot as plt

SIGNAL_COLUMN = "/blendshapes/0/jawOpen:Scalar"


def export_to_pandas(recording: rr.dataframe.Recording) -> pd.DataFrame:
    return (
        recording.view(index="frame_nr", contents="/blendshapes/0/jawOpen")
        .select(
            rr.dataframe.IndexColumnSelector("frame_nr"),
            rr.dataframe.ComponentColumnSelector(
                "/blendshapes/0/jawOpen",
                "rerun.components.Scalar",
            ),
        )
        .read_pandas()
    )


def load_mouth_open_data(recording: rr.dataframe.Recording) -> pd.DataFrame:
    df = export_to_pandas(recording)

    # TODO: how can we do that better?
    df[SIGNAL_COLUMN] = df[SIGNAL_COLUMN].values.flatten()
    # df[SIGNAL_COLUMN] = df[SIGNAL_COLUMN].apply(lambda x: x[0] if x is not None else x)  # alternative to handle null (slower?)

    return df


def analyze_data(df: pd.DataFrame) -> (npt.NDArray, npt.NDArray):
    # compute the mouth open state
    df["mouth_open"] = (df[SIGNAL_COLUMN] > 0.15).astype(int)

    # find the state transitions
    diff = np.diff(df["mouth_open"], prepend=df["mouth_open"].iloc[0])
    open_mouth_frames = df["frame_nr"][diff == 1].values
    closed_mouth_frames = df["frame_nr"][diff == -1].values

    # add the initial state
    if df["mouth_open"].iloc[0] == 1:
        open_mouth_frames = np.concatenate([[0], open_mouth_frames])
    else:
        closed_mouth_frames = np.concatenate([[0], closed_mouth_frames])

    return open_mouth_frames, closed_mouth_frames


def plot_analysis(df: pd.DataFrame, open_mouth_frames: npt.NDArray, closed_mouth_frames: npt.NDArray) -> None:
    # TODO: bonus points for splitting the series on `frame_nr` discontinuities

    plt.plot(df["frame_nr"], df[SIGNAL_COLUMN], df["frame_nr"], df["mouth_open"])
    plt.plot(open_mouth_frames, np.ones_like(open_mouth_frames), "ro", label="start smiling")
    plt.plot(closed_mouth_frames, np.zeros_like(closed_mouth_frames), "go", label="stop smiling")
    plt.show()


def log_analysis(df: pd.DataFrame, open_mouth_frames: npt.NDArray, closed_mouth_frames: npt.NDArray) -> None:
    # log state transitions as a red dot showing on top the video feed
    for frame_nr in open_mouth_frames:
        rr.set_time_sequence("frame_nr", frame_nr)
        rr.log("/mouth_open/indicator", rr.Points2D([100, 100], radii=20, colors=[255, 0, 0]))
    for frame_nr in closed_mouth_frames:
        rr.set_time_sequence("frame_nr", frame_nr)
        rr.log("/mouth_open/indicator", rr.Clear(recursive=False))

    # log state transitions to a TextLog view
    for frame_nr in open_mouth_frames:
        rr.set_time_sequence("frame_nr", frame_nr)
        rr.log("/mouth_open/state", rr.TextLog(f"mouth opened"))
    for frame_nr in closed_mouth_frames:
        rr.set_time_sequence("frame_nr", frame_nr)
        rr.log("/mouth_open/state", rr.TextLog(f"mouth closed"))

    # log the mouth open signal as a scalar
    rr.send_columns(
        "/mouth_open/values",
        times=[rr.TimeSequenceColumn("frame_nr", df["frame_nr"])],
        components=[
            rr.components.ScalarBatch(df["mouth_open"].values),
        ],
    )


def main():
    # Load the recording
    # TODO: loading directly from the viewer would be sweet
    recording = rr.dataframe.load_recording("face_tracking.rrd")

    # Extract the data
    df = load_mouth_open_data(recording)

    # Process the data
    open_mouth_frames, closed_mouth_frames = analyze_data(df)

    # Log the analysis results
    rr.init(recording.application_id(), recording_id=recording.recording_id())
    rr.connect()
    log_analysis(df, open_mouth_frames, closed_mouth_frames)

    # Plot the data
    plot_analysis(df, open_mouth_frames, closed_mouth_frames)


if __name__ == "__main__":
    main()
