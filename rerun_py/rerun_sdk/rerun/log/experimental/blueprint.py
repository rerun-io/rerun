# TODO(jleibs): docstrings
from typing import List, Optional

import rerun_bindings as bindings  # type: ignore[attr-defined]

from rerun.recording_stream import RecordingStream


def new_blueprint(
    application_id: str,
    blueprint_id: Optional[str] = None,
    make_default: bool = False,
    make_thread_default: bool = False,
    spawn: bool = False,
    add_to_app_default_blueprint: bool = False,
    default_enabled: bool = True,
) -> RecordingStream:
    """
    Creates a new blueprint with a user-chosen application id (name) to configure the appearance of Rerun.

    If you only need a single global blueprint, [`rerun.init`][] might be simpler.

    Parameters
    ----------
    application_id : str
        Your Rerun recordings will be categorized by this application id, so
        try to pick a unique one for each application that uses the Rerun SDK.

        For example, if you have one application doing object detection
        and another doing camera calibration, you could have
        `rerun.init("object_detector")` and `rerun.init("calibrator")`.
    blueprint_id : Optional[str]
        Set the blueprint ID that this process is logging to, as a UUIDv4.

        The default blueprint_id is based on `multiprocessing.current_process().authkey`
        which means that all processes spawned with `multiprocessing`
        will have the same default recording_id.

        If you are not using `multiprocessing` and still want several different Python
        processes to log to the same Rerun instance (and be part of the same blueprint),
        you will need to manually assign them all the same blueprint_id.
        Any random UUIDv4 will work, or copy the recording id for the parent process.
    make_default : bool
        If true (_not_ the default), the newly initialized blueprint will replace the current
        active one (if any) in the global scope.
    make_thread_default : bool
        If true (_not_ the default), the newly initialized blueprint will replace the current
        active one (if any) in the thread-local scope.
    spawn : bool
        Spawn a Rerun Viewer and stream logging data to it.
        Short for calling `spawn` separately.
        If you don't call this, log events will be buffered indefinitely until
        you call either `connect`, `show`, or `save`
    add_to_app_default_blueprint
        Should the blueprint append to the existing app-default blueprint instead instead of creating a new one.
    default_enabled
        Should Rerun logging be on by default?
        Can overridden with the RERUN env-var, e.g. `RERUN=on` or `RERUN=off`.

    Returns
    -------
    RecordingStream
        A handle to the [`rerun.RecordingStream`][]. Use it to log data to Rerun.

    """

    blueprint_id = application_id if add_to_app_default_blueprint else blueprint_id

    blueprint = RecordingStream(
        bindings.new_blueprint(
            application_id=application_id,
            blueprint_id=blueprint_id,
            make_default=make_default,
            make_thread_default=make_thread_default,
            default_enabled=default_enabled,
        )
    )

    if spawn:
        from rerun.sinks import spawn as _spawn

        _spawn(recording=blueprint)

    return blueprint


def add_space_view(
    name: str,
    space_path: str,
    entity_paths: List[str],
    blueprint: Optional[RecordingStream] = None,
) -> None:
    blueprint = RecordingStream.to_native(blueprint)
    bindings.add_space_view(name, space_path, entity_paths, blueprint)


# TODO(jleibs): docstrings
def set_panels(
    *,
    all_expanded: Optional[bool] = None,
    blueprint_view_expanded: Optional[bool] = None,
    selection_view_expanded: Optional[bool] = None,
    timeline_view_expanded: Optional[bool] = None,
    blueprint: Optional[RecordingStream] = None,
) -> None:
    blueprint = RecordingStream.to_native(blueprint)
    bindings.set_panels(
        blueprint_view_expanded=blueprint_view_expanded or all_expanded,
        selection_view_expanded=selection_view_expanded or all_expanded,
        timeline_view_expanded=timeline_view_expanded or all_expanded,
        blueprint=blueprint,
    )


# TODO(jleibs): docstrings
def set_auto_space_views(
    enabled: bool,
    blueprint: Optional[RecordingStream] = None,
) -> None:
    blueprint = RecordingStream.to_native(blueprint)
    bindings.set_auto_space_views(enabled, blueprint)
