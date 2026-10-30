"""
The folders under `crates/`, and how they layer.

`check_crate_layers.py` enforces this layering, and `generate_crate_graph.py` draws it in `ARCHITECTURE.md`.
"""

from __future__ import annotations

from typing import NamedTuple

# Top to bottom. Each layer holds one or more columns, and each column stacks one or more folders, top to bottom.
#
# A folder may only depend on itself, the folders under it in its column, and the later layers.
# The columns of a layer are siblings: neither may depend on the other, in either direction.
LAYERS: list[list[list[str]]] = [
    [["crates/tests"]],
    [["crates/top"]],
    [["crates/views"], ["crates/panels"]],  # A view is not a panel, and neither uses the other.
    [["crates/viewer_support"]],
    [
        ["crates/store_app", "crates/data_flow", "crates/store"],
        ["crates/ui_tools"],  # UI tools know nothing about the data, and the data nothing about UI.
    ],
    [["crates/build"]],
    [["crates/utils"]],
]

# Every folder in `LAYERS`, top to bottom, ignoring which of them are siblings.
FOLDERS = [folder for layer in LAYERS for column in layer for folder in column]


class Description(NamedTuple):
    """How `ARCHITECTURE.md` presents a folder."""

    heading: str
    """The label on its band in the diagram, and the heading of its crate table."""

    blurb: str
    """Shown under the heading, after the link to the folder."""


DESCRIPTIONS: dict[str, Description] = {
    "crates/tests": Description(
        "Test support",
        "only tests depend on these, so they sit at the top of the diagram.",
    ),
    "crates/top": Description(
        "SDK / CLI / Wasm",
        "the entry points for our users, and the viewer app itself.",
    ),
    "crates/views": Description(
        "Views",
        "the visualizations a user can put in the viewport.\n"
        "A sibling of `crates/panels`: neither depends on the other.",
    ),
    "crates/panels": Description(
        "Panels",
        "the panels the app is assembled from, and the widgets they are built out of.\n"
        "A sibling of `crates/views`: neither depends on the other.",
    ),
    "crates/viewer_support": Description(
        "Viewer support",
        "the UI and rendering machinery the views are built on.",
    ),
    "crates/store_app": Description(
        "Application-level store",
        "the queryable state a viewer or a server works with.",
    ),
    "crates/data_flow": Description(
        "Data flow",
        "getting data in and out: clients, servers, and file importers.",
    ),
    "crates/store": Description(
        "Data model & chunk store",
        "the data model, and the in-memory store that holds it.",
    ),
    "crates/ui_tools": Description(
        "UI tools",
        "generic widgets and UI helpers, built on egui.\nNothing Rerun-specific in here.",
    ),
    "crates/build": Description(
        "Build support",
        "crates that run at build time.",
    ),
    "crates/utils": Description(
        "Utilities",
        "small crates that depend on nothing outside `crates/utils`.",
    ),
}

assert set(DESCRIPTIONS) == set(FOLDERS), "every folder in LAYERS needs a description, and vice versa"
