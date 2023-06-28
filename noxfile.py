"""Nox sessions.

This file is used by `nox` to run tests and examples against multiple Python versions.

See: http://nox.thea.codes
"""

from __future__ import annotations

import nox  # type: ignore


@nox.session(python=["3.8", "3.9", "3.10", "3.11"])
def tests(session: nox.Session) -> None:
    session.install("-r", "rerun_py/requirements-build.txt")
    session.install("./rerun_py")
    session.run("just", "py-test", external=True)


@nox.session(python=["3.8", "3.9", "3.10", "3.11"])
def examples(session: nox.Session) -> None:
    session.install("-r", "examples/python/requirements.txt")
    session.install("./rerun_py")
    session.run("python", "scripts/run_all.py", "--save")
