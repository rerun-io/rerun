from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Deselect on Escape

This checks that the deselect-on-escape works as expected.

### Actions

* Base behavior: select the 3D view and hit ESC => the view is no longer selected.

In all the following cases, the 3D view should *remain selected*.

* Select the 3D view, open the "Background Kind" dropdown in the selection panel, and hit ESC.
* Select the 3D view, open the "Add view or container" modal, and hit ESC.
* Select the 3D view, right-click on it in the blueprint tree, and hit ESC.
* Select the 3D view, open the Rerun menu, and hit ESC (KNOWN FAIL).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_some_data() -> None:
    rr.log("data", rr.Points3D([0, 0, 0]))


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_some_data()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
