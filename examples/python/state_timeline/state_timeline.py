#!/usr/bin/env python3
"""
Demonstrates all features of the state timeline view.

Run:
```sh
./examples/python/state_timeline/state_timeline.py
```
"""

from __future__ import annotations

import argparse

import numpy as np
import pyarrow as pa

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.encodings import ComponentSourceKind, VisualizerComponentMapping

DESCRIPTION = """
# State timeline
This example simulates a robot work cell and demonstrates every feature of the state timeline view:
state changes, custom styling (labels, colors, per-state visibility), state resets, and columnar logging.

Pass `--cycles 20` to get a recording spanning more than a minute. Such a recording is not shown in
full: the view starts out showing a window around the time cursor, which keeps the cursor in the
middle of the view while playing. Panning or zooming leaves the cursor behind, and a double click
brings it back.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/state_timeline).
""".strip()

CYCLE_DURATION_SEC = 8.0
DEFAULT_NUM_CYCLES = 6

# The lanes that don't follow the work cycle repeat their pattern over this many seconds, so they
# cover the whole recording however long it is.
PATTERN_DURATION_SEC = 48.0


def repeat_pattern(transitions: list[tuple[float, str]], total_sec: float) -> list[tuple[float, str]]:
    """Repeat a pattern of `(time, state)` transitions until `total_sec` is covered."""
    repeated = []
    block_start = 0.0
    while block_start < total_sec:
        repeated += [(block_start + t, state) for t, state in transitions if block_start + t <= total_sec]
        block_start += PATTERN_DURATION_SEC
    return repeated


def log_task(num_cycles: int, total_sec: float) -> None:
    # A fully styled lane: `StateConfiguration` maps each raw state value to a display label
    # and a color. The configuration is time-independent, so it's logged as static.
    rr.log(
        "robot/task",
        rr.StateConfiguration(
            values=["idle", "pick", "place", "error"],
            labels=["Idle", "Picking", "Placing", "Error"],
            # Wrapped as `np.uint32` so that the list isn't mistaken for a single RGB color.
            colors=np.array([0x9E9E9EFF, 0x42A5F5FF, 0x66BB6AFF, 0xEF5350FF], dtype=np.uint32),
        ),
        static=True,
    )

    # A `StateChange` marks a transition into a new state; the state timeline view extends
    # each state until the next transition.
    for cycle in range(num_cycles):
        t = cycle * CYCLE_DURATION_SEC

        rr.set_time("time", duration=t)
        rr.log("robot/task", rr.StateChange(state="idle"))

        rr.set_time("time", duration=t + 2.0)
        rr.log("robot/task", rr.StateChange(state="pick"))

        if cycle % 4 == 3:
            # Something went wrong during this pick.
            rr.set_time("time", duration=t + 3.5)
            rr.log("robot/task", rr.StateChange(state="error"))
        else:
            rr.set_time("time", duration=t + 5.0)
            rr.log("robot/task", rr.StateChange(state="place"))

    rr.set_time("time", duration=total_sec)
    rr.log("robot/task", rr.StateChange(state="idle"))


def log_gripper(num_cycles: int) -> None:
    # This lane has no `StateConfiguration` at all: raw state values are used as labels, and
    # colors are assigned automatically from a built-in palette.
    rr.set_time("time", duration=0.0)
    rr.log("robot/gripper", rr.StateChange(state="open"))

    for cycle in range(num_cycles):
        t = cycle * CYCLE_DURATION_SEC

        rr.set_time("time", duration=t + 3.0)
        rr.log("robot/gripper", rr.StateChange(state="closed"))

        rr.set_time("time", duration=t + 6.0)
        rr.log("robot/gripper", rr.StateChange(state="open"))


def log_connection(total_sec: float) -> None:
    # `labels` is shorter than `values` here: states without a label fall back to showing
    # their raw value ("degraded").
    rr.log(
        "robot/connection",
        rr.StateConfiguration(
            values=["online", "degraded"],
            labels=["Online"],
            colors=np.array([0x66BB6AFF, 0xFFB300FF], dtype=np.uint32),
        ),
        static=True,
    )

    # An empty string resets the state: the state timeline view shows a gap until the next
    # state change.
    transitions = [
        (0.0, "online"),
        (18.0, ""),
        (22.0, "online"),
        (34.0, "degraded"),
        (42.0, "online"),
    ]
    for t, state in repeat_pattern(transitions, total_sec):
        rr.set_time("time", duration=t)
        rr.log("robot/connection", rr.StateChange(state=state))


def log_diagnostics(total_sec: float) -> None:
    # Per-state visibility: "chatter" is a noisy diagnostic state that would clutter the
    # timeline; setting its `visible` entry to `False` hides those segments.
    rr.log(
        "robot/diagnostics",
        rr.StateConfiguration(
            values=["ok", "chatter", "fault"],
            colors=np.array([0x66BB6AFF, 0x9E9E9EFF, 0xEF5350FF], dtype=np.uint32),
            visible=[True, False, True],
        ),
        static=True,
    )

    transitions = [
        (0.0, "ok"),
        (10.0, "chatter"),
        (11.0, "ok"),
        (20.0, "chatter"),
        (21.0, "ok"),
        (26.0, "fault"),
        (29.0, "ok"),
        (40.0, "chatter"),
        (41.0, "ok"),
    ]
    for t, state in repeat_pattern(transitions, total_sec):
        rr.set_time("time", duration=t)
        rr.log("robot/diagnostics", rr.StateChange(state=state))


def log_conveyor(total_sec: float) -> None:
    # State changes can also be logged in one batch using the columnar API. A `null` state
    # resets the state, just like an empty string: the conveyor sensor drops out twice, and
    # the state timeline view shows a gap until the next state. The states are wrapped in a
    # `pyarrow` array, since a plain Python list would stringify `None` entries.
    times = np.arange(0.0, total_sec, 6.0)
    pattern = ["running", "stopped", None, "jammed", "running", None, "stopped", "running"]
    states = pa.array([pattern[i % len(pattern)] for i in range(len(times))], type=pa.utf8())

    rr.send_columns(
        "conveyor",
        indexes=[rr.TimeColumn("time", duration=times)],
        columns=rr.StateChange.columns(state=states),
    )


def log_plc(total_sec: float) -> None:
    # States don't have to be strings logged with `StateChange`: any string, integer, float,
    # or boolean component can be shown as a state lane, including custom components logged
    # with `DynamicArchetype`. The blueprint maps them onto the `StateChange:state` slot of
    # the state visualizer (see `main()`).
    times = np.arange(0.0, total_sec, 4.0)
    # An integer enum: 0 = auto, 1 = manual, 2 = maintenance.
    mode = np.array([0, 0, 1, 1, 0, 1, 1, 2, 2, 2, 1, 0], dtype=np.int32)
    # A boolean flag; the emergency stop engages while the robot task errors out.
    estop = np.array([False, False, False, False, False, False, True, True, False, False, False, False])

    rr.send_columns(
        "plc",
        indexes=[rr.TimeColumn("time", duration=times)],
        columns=rr.DynamicArchetype.columns(
            archetype="plc",
            components={
                # `np.resize` repeats the pattern to cover however long the recording is.
                "mode": np.resize(mode, len(times)),
                "estop": np.resize(estop, len(times)),
            },
        ),
    )

    # `StateConfiguration` works for non-string states too: values are matched against the
    # displayed form of the state, so the integer enum is keyed by "0", "1", "2".
    rr.log(
        "plc",
        rr.StateConfiguration(
            values=["0", "1", "2"],
            labels=["Auto", "Manual", "Maintenance"],
        ),
        static=True,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Demonstrates all features of the state timeline view")
    parser.add_argument(
        "--cycles",
        type=int,
        default=DEFAULT_NUM_CYCLES,
        help="Number of work cycles to simulate, 8 seconds each. Past a minute (8 cycles) the views "
        "no longer show the whole recording, and start out centered on the time cursor instead.",
    )
    rr.script_add_args(parser)
    args = parser.parse_args()

    num_cycles = max(1, args.cycles)
    total_sec = num_cycles * CYCLE_DURATION_SEC

    def map_to_state(source_component: str) -> rrb.Visualizer:
        # Install a state visualizer that sources its state from a custom component.
        return rr.StateChange().visualizer(
            mappings=[
                VisualizerComponentMapping(
                    target="StateChange:state",
                    source_kind=ComponentSourceKind.SourceComponent,
                    source_component=source_component,
                ),
            ],
        )

    blueprint = rrb.Blueprint(
        rrb.Horizontal(
            rrb.Vertical(
                rrb.StateTimelineView(
                    name="All states",
                    origin="/",
                    overrides={
                        # The custom `plc` components are not picked up automatically; each
                        # one gets its own state lane by explicitly mapping it onto the
                        # `StateChange:state` slot of a state visualizer.
                        "plc": [
                            map_to_state("plc:mode"),
                            map_to_state("plc:estop"),
                        ],
                    },
                ),
                # A view can be scoped to a subtree with `origin`, and its contents can be
                # further filtered with entity path expressions.
                rrb.StateTimelineView(
                    name="Robot (without diagnostics)",
                    origin="/robot",
                    contents=["$origin/**", "- $origin/diagnostics"],
                ),
            ),
            rrb.TextDocumentView(name="Description", origin="/description"),
            column_shares=[3, 1],
        ),
        rrb.SelectionPanel(state="collapsed"),
    )

    rr.script_setup(args, "rerun_example_state_timeline", default_blueprint=blueprint)

    rr.log("description", rr.TextDocument(DESCRIPTION, media_type=rr.MediaType.MARKDOWN), static=True)

    log_task(num_cycles, total_sec)
    log_gripper(num_cycles)
    log_connection(total_sec)
    log_diagnostics(total_sec)
    log_conveyor(total_sec)
    log_plc(total_sec)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
