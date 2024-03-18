"""
Nox sessions.

This file is used by `nox` to run tests and examples against multiple Python versions.

See: http://nox.thea.codes
"""

from __future__ import annotations

import nox  # type: ignore

PYTHON_VERSIONS = ["3.8", "3.9", "3.10", "3.11", "3.12"]


@nox.session(python=PYTHON_VERSIONS)
def tests(session: nox.Session) -> None:
    """Run the Python test suite."""
    session.install("-r", "rerun_py/requirements-build.txt")

    # TODO(#4704): clean that up when torch is 3.12 compatible
    if session.python == "3.12":
        session.run(
            "pip", "install", "torch", "torchvision", "--pre", "--index-url", "https://download.pytorch.org/whl/nightly"
        )

    session.install("./rerun_py")
    session.run("just", "py-test", external=True)


@nox.session(python=PYTHON_VERSIONS)
def run_all(session: nox.Session) -> None:
    """Run all examples through the run_all.py script (pass args with: "-- <args>")."""

    # TODO(#4704): clean that up when torch is 3.12 compatible
    if session.python == "3.12":
        session.run(
            "pip", "install", "torch", "torchvision", "--pre", "--index-url", "https://download.pytorch.org/whl/nightly"
        )

    # Note: the run_all.py scripts installs all dependencies itself. In particular, we can install from
    # examples/python/requirements.txt because it includes pyrealsense2, which is not available for mac.
    session.run("python", "scripts/run_all.py", "--install-requirements", *session.posargs)


roundtrip_cpp_built = False


@nox.session(python=PYTHON_VERSIONS)
def roundtrips(session: nox.Session) -> None:
    """Run all roundtrip tests (C++ will be built only once / skip with: "-- --no-cpp-build")."""

    global roundtrip_cpp_built

    session.install("-r", "rerun_py/requirements-build.txt")
    session.install("opencv-python")

    # TODO(#4704): clean that up when torch is 3.12 compatible
    if session.python == "3.12":
        session.run(
            "pip", "install", "torch", "torchvision", "--pre", "--index-url", "https://download.pytorch.org/whl/nightly"
        )
    session.install("./rerun_py")

    extra_args = []
    if roundtrip_cpp_built and "--no-cpp-build" not in session.posargs:
        extra_args.append("--no-cpp-build")
    extra_args.extend(session.posargs)

    session.run("python", "tests/roundtrips.py", "--no-py-build", *extra_args)
    session.run("python", "docs/snippets/compare_snippet_output.py", "--no-py-build", *extra_args)

    roundtrip_cpp_built = True
