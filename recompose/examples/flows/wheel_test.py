"""
Wheel build and test flow for the recompose project.

This flow handles building wheels and testing them in isolated environments,
with optional full test suite execution.
"""

import recompose

from ..tasks import build_wheel, create_test_venv, install_wheel, smoke_test, test_installed


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
