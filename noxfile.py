"""Nox sessions.

This file is used by `nox` to run tests and examples against multiple Python versions.

See: http://nox.thea.codes
"""

from __future__ import annotations

import nox  # type: ignore


@nox.session(python=["3.8", "3.9", "3.10", "3.11"])
def tests(session: nox.Session) -> None:
    """Run the Python test suite"""
    session.install("-r", "rerun_py/requirements-build.txt")
    session.install("./rerun_py")
    session.run("just", "py-test", external=True)


@nox.session(python=["3.8", "3.9", "3.10", "3.11"])
def run_all(session: nox.Session) -> None:
    """Run all examples through the run_all.py script (pass args with: "-- <args>")"""
    # Note: the run_all.py scripts installs all dependencies itself. In particular, we can install from
    # examples/python/requirements.txt because it includes pyrealsense2, which is not available for mac.
    session.run("python", "scripts/run_all.py", "--install-requirements", *session.posargs)
