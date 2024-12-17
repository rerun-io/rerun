from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """
# Blueprint backwards compatibility

Updating your Rerun version should never result in indecipherable Blueprint errors.

Even in the case of irrecoverable ABI changes, we should make sure that the end-user has
a pleasant experience (i.e., in the worst case, the blueprint is ignored, with a warning).

## Checks

#### Step 1: instantiate blueprints from previous release

* Install the latest official Rerun release in a virtual env: `pip install --force rerun-sdk`.
* Clear all your blueprint data: `rerun reset`.
* Start Rerun and check in Menu > About that you are indeed running the version you think
  you're running.
* Open all demos available in the welcome screen.
* Play around long enough for the blueprints to be saved to disk (a few seconds).
    * You can check whether that's the case by listing the contents of:
          - Linux: `/home/UserName/.local/share/rerun`
          - macOS: `/Users/UserName/Library/Application Support/rerun`
          - Windows: `C:\\Users\\UserName\\AppData\\Roaming\\rerun`
          - Web: local storage

#### Step 2: load blueprints from previous release into new one

* Install the about-to-released Rerun version in a virtual env: `pip install --force rerun-sdk=whatever`.
* Start Rerun and check in Menu > About that you are indeed running the version you think
  you're running.
* Open all demos available in the welcome screen.

#### Step 3: does it look okay?

There are two acceptable outcomes here:
- The blueprints worked as-is. 👍
- The blueprints completely or partially failed to load, but a clear warning explaining what's
  going on was shown to the user.

Anything else is a failure (e.g. deserialization failure spam).
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.send_blueprint(rrb.Grid(rrb.TextDocumentView(origin="readme")), make_active=True, make_default=True)

    log_readme()


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
