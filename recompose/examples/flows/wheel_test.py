"""
Wheel build and test flow for the recompose project.

This flow handles building wheels and testing them in isolated environments,
with optional full test suite execution.
"""

import recompose

from ..tasks import Venv, build_wheel, smoke_test_venv, test_installed_venv

# Keep old imports for backward compatibility flow
from ..tasks import create_test_venv, install_wheel, smoke_test, test_installed


@recompose.flow
def wheel_test(*, full_tests: bool = False) -> None:
    """
    Build a wheel, install it in a fresh venv, and run tests.

    This flow validates that the package can be:
    1. Built into a wheel
    2. Installed into a clean environment
    3. Imported and used correctly (smoke test)
    4. Optionally: pass the complete test suite

    Args:
        full_tests: If True, run the full pytest suite after smoke tests.
                   Default is False (smoke tests only).

    Examples:
        # Quick validation (smoke tests only):
        ./run wheel_test

        # Full validation (includes full test suite):
        ./run wheel_test --full_tests

    """
    # Build the wheel
    wheel = build_wheel()

    # Create a fresh test environment
    venv = create_test_venv()

    # Install the wheel
    install_wheel(wheel=wheel.value(), venv=venv.value())

    # Always run smoke tests
    smoke_test(venv=venv.value())

    # Optionally run the full test suite
    with recompose.run_if(full_tests):
        test_installed(venv=venv.value())


@recompose.flow
def wheel_test_v2(*, full_tests: bool = False) -> None:
    """
    Build a wheel, install it in a fresh venv, and run tests.

    This version uses the Venv TaskClass instead of separate tasks,
    demonstrating the cleaner API where:
    - Venv instantiation creates the venv (step 1)
    - venv.install_wheel() installs the wheel (step 2)
    - smoke_test_venv(venv=venv) receives the Venv instance and runs tests

    Args:
        full_tests: If True, run the full pytest suite after smoke tests.
                   Default is False (smoke tests only).

    Examples:
        # Quick validation (smoke tests only):
        ./run wheel_test_v2

        # Full validation (includes full test suite):
        ./run wheel_test_v2 --full_tests

    """
    # Build the wheel
    wheel = build_wheel()

    # Create a fresh test environment (TaskClass instantiation becomes a step)
    venv = Venv()

    # Install the wheel (method call becomes a step)
    venv.install_wheel(wheel=wheel.value())

    # Always run smoke tests (pass Venv directly, no .value() needed)
    smoke_test_venv(venv=venv)

    # Optionally run the full test suite
    with recompose.run_if(full_tests):
        test_installed_venv(venv=venv)
