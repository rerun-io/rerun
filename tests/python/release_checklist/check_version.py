from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr

README = """\
# Version check

- Verify the version number is correct given the release.
"""


def log_readme() -> None:
    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)


def log_the_versions() -> None:
    rr.log(
        "/version",
        rr.TextDocument(
            f"""```
__version__ = {rr.__version__}
___version_info___ = {rr.__version_info__}
rr.version() = {rr.version()}
```
""",
            media_type=rr.MediaType.MARKDOWN,
        ),
    )


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    log_readme()
    log_the_versions()

    rr.send_blueprint(rr.blueprint.Blueprint(auto_layout=True, auto_views=True), make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)
