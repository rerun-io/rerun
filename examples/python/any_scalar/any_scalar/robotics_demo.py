#!/usr/bin/env python3
"""
Robotics PID Controller Demo
Shows features for "Any Scalar" visualization:
- Multiple visualizers (Lines + Points) on the same telemetry field.
- Step interpolation for discrete control effort and state flags.
- Boolean scalar plotting.
"""

from __future__ import annotations

import numpy as np

import rerun as rr
import rerun.blueprint as rrb
from rerun.blueprint.datatypes import ComponentSourceKind, VisualizerComponentMapping


def simulate_robot_controller() -> None:
    """Simulate a 1-DOF joint with a PID controller tracking a trajectory."""
    print("Simulating robot PID controller…")

    # PID Constants
    Kp, Ki, Kd = 12.0, 1.5, 0.8
    dt = 0.05
    steps = 400

    # State
    position = 0.0
    velocity = 0.0
    integral = 0.0
    prev_error = 0.0

    telemetry_data = []

    for i in range(steps):
        t = i * dt

        # Trajectory
        setpoint = 2.0 * np.sin(1.0 * t) + 0.5 * np.cos(3.0 * t)

        # Controller
        error = setpoint - position
        integral += error * dt
        derivative = (error - prev_error) / dt
        effort = Kp * error + Ki * integral + Kd * derivative
        prev_error = error

        # Physics
        acceleration = effort - 0.5 * velocity
        velocity += acceleration * dt
        position += velocity * dt

        # Deeply nested telemetry: state, control, status
        # This showcases Rerun's powerful jq-style selectors for hierarchical data.
        telemetry_data.append([
            {
                "state": {
                    "setpoint": float(setpoint),
                    "position": float(position),
                },
                "control": {
                    "error": float(error),
                    "effort": float(effort),
                },
                "status": {"is_stable": bool(abs(error) < 0.1)},
            }
        ])

    # Log telemetry using send_columns
    rr.send_columns(
        "robot/joint_0",
        indexes=[rr.TimeColumn("robot_step", sequence=np.arange(steps))],
        columns=[*rr.DynamicArchetype.columns(archetype="ControllerTelemetry", components={"data": telemetry_data})],
    )


def run_robotics_simulation() -> None:
    """Run the robotics simulation and log data."""
    simulate_robot_controller()


def generate_blueprint() -> rrb.Blueprint:
    """Generate the blueprint for the robotics demo."""
    return rrb.Blueprint(
        rrb.Vertical(
            # View 1: Main Control Performance (Multiple Visualizers + Nested Selectors)
            rrb.TimeSeriesView(
                name="Control Performance",
                origin="/robot/joint_0",
                overrides={
                    "robot/joint_0": [
                        rr.SeriesLines(names="Setpoint", colors=[100, 100, 255]).visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".state.setpoint",
                                )
                            ]
                        ),
                        rr.SeriesLines(names="Position", colors=[255, 100, 0]).visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".state.position",
                                )
                            ]
                        ),
                        # Multiple visualizers on the SAME field (.control.error)
                        rr.SeriesLines(names="Error (Line)", colors=[255, 0, 0]).visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".control.error",
                                )
                            ]
                        ),
                        rr.SeriesPoints(names="Error (Dots)", colors=[255, 0, 0]).visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".control.error",
                                )
                            ]
                        ),
                    ]
                },
            ),
            # View 2: Internals (Step Interpolation & Nested Booleans)
            rrb.TimeSeriesView(
                name="Controller Internals",
                origin="/robot/joint_0",
                overrides={
                    "robot/joint_0": [
                        rr.SeriesLines(names="Effort", colors=[0, 200, 200], interpolation_mode="StepAfter").visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".control.effort",
                                )
                            ]
                        ),
                        rr.SeriesLines(
                            names="Stability Flag", colors=[0, 255, 0], interpolation_mode="StepAfter"
                        ).visualizer(
                            mappings=[
                                VisualizerComponentMapping(
                                    target="Scalars:scalars",
                                    source_kind=ComponentSourceKind.SourceComponent,
                                    source_component="ControllerTelemetry:data",
                                    selector=".status.is_stable",
                                )
                            ]
                        ),
                    ]
                },
            ),
            # View 3: Dataframe Inspector
            rrb.DataframeView(
                name="Telemetry Inspector",
                origin="/robot/joint_0",
                query=rrb.archetypes.DataframeQuery(
                    auto_scroll=True,
                ),
            ),
        )
    )


def main() -> None:
    rr.init("rerun_example_any_scalar_robotics", spawn=True)

    run_robotics_simulation()

    rr.send_blueprint(generate_blueprint())


if __name__ == "__main__":
    main()
