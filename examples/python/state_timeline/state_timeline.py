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
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping

DESCRIPTION = """
# State timeline
This example simulates a robot work cell and demonstrates every feature of the state timeline view:
state changes, custom styling (labels, colors, per-state visibility), state resets, and columnar logging.

The full source code for this example is available
[on GitHub](https://github.com/rerun-io/rerun/blob/latest/examples/python/state_timeline).
""".strip()

CYCLE_DURATION_SEC = 8.0
NUM_CYCLES = 6
TOTAL_DURATION_SEC = CYCLE_DURATION_SEC * NUM_CYCLES


def log_task() -> None:
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
    for cycle in range(NUM_CYCLES):
        t = cycle * CYCLE_DURATION_SEC

        rr.set_time("time", duration=t)
        rr.log("robot/task", rr.StateChange(state="idle"))

        rr.set_time("time", duration=t + 2.0)
        rr.log("robot/task", rr.StateChange(state="pick"))

        if cycle == 3:
            # Something went wrong during this pick.
            rr.set_time("time", duration=t + 3.5)
            rr.log("robot/task", rr.StateChange(state="error"))
        else:
            rr.set_time("time", duration=t + 5.0)
            rr.log("robot/task", rr.StateChange(state="place"))

    rr.set_time("time", duration=TOTAL_DURATION_SEC)
    rr.log("robot/task", rr.StateChange(state="idle"))


def log_gripper() -> None:
    # This lane has no `StateConfiguration` at all: raw state values are used as labels, and
    # colors are assigned automatically from a built-in palette.
    rr.set_time("time", duration=0.0)
    rr.log("robot/gripper", rr.StateChange(state="open"))

    for cycle in range(NUM_CYCLES):
        t = cycle * CYCLE_DURATION_SEC

        rr.set_time("time", duration=t + 3.0)
        rr.log("robot/gripper", rr.StateChange(state="closed"))

        rr.set_time("time", duration=t + 6.0)
        rr.log("robot/gripper", rr.StateChange(state="open"))


def log_connection() -> None:
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

    rr.set_time("time", duration=0.0)
    rr.log("robot/connection", rr.StateChange(state="online"))

    # An empty string resets the state: the state timeline view shows a gap until the next
    # state change.
    rr.set_time("time", duration=18.0)
    rr.log("robot/connection", rr.StateChange(state=""))

    rr.set_time("time", duration=22.0)
    rr.log("robot/connection", rr.StateChange(state="online"))

    rr.set_time("time", duration=34.0)
    rr.log("robot/connection", rr.StateChange(state="degraded"))

    rr.set_time("time", duration=42.0)
    rr.log("robot/connection", rr.StateChange(state="online"))


def log_diagnostics() -> None:
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
    for t, state in transitions:
        rr.set_time("time", duration=t)
        rr.log("robot/diagnostics", rr.StateChange(state=state))


def log_conveyor() -> None:
    # State changes can also be logged in one batch using the columnar API. A `null` state
    # resets the state, just like an empty string: the conveyor sensor drops out twice, and
    # the state timeline view shows a gap until the next state. The states are wrapped in a
    # `pyarrow` array, since a plain Python list would stringify `None` entries.
    times = np.arange(0.0, TOTAL_DURATION_SEC, 6.0)
    states = pa.array(["running", "stopped", None, "jammed", "running", None, "stopped", "running"], type=pa.utf8())

    rr.send_columns(
        "conveyor",
        indexes=[rr.TimeColumn("time", duration=times)],
        columns=rr.StateChange.columns(state=states),
    )


def log_plc() -> None:
    # States don't have to be strings logged with `StateChange`: any string, integer, float,
    # or boolean component can be shown as a state lane, including custom components logged
    # with `DynamicArchetype`. The blueprint maps them onto the `StateChange:state` slot of
    # the state visualizer (see `main()`).
    times = np.arange(0.0, TOTAL_DURATION_SEC, 4.0)
    rr.send_columns(
        "plc",
        indexes=[rr.TimeColumn("time", duration=times)],
        columns=rr.DynamicArchetype.columns(
            archetype="plc",
            components={
                # An integer enum: 0 = auto, 1 = manual, 2 = maintenance.
                "mode": np.array([0, 0, 1, 1, 0, 1, 1, 2, 2, 2, 1, 0], dtype=np.int32),
                # A boolean flag; the emergency stop engages while the robot task errors out.
                "estop": np.array([False, False, False, False, False, False, True, True, False, False, False, False]),
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
    rr.script_add_args(parser)
    args = parser.parse_args()

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

    log_task()
    log_gripper()
    log_connection()
    log_diagnostics()
    log_conveyor()
    log_plc()

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
