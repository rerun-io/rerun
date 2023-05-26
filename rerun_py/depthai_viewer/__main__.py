"""See `python3 -m rerun --help`."""

import os
import sys
import subprocess
from depthai_viewer import bindings, unregister_shutdown  # type: ignore[attr-defined]
import site
from depthai_viewer import version as depthai_viewer_version
import shutil


def create_venv_and_install_dependencies() -> str:
    print("Creating venv and installing dependencies...")
    script_path = os.path.dirname(os.path.abspath(__file__))
    venv_dir = os.path.join(script_path, "venv-" + depthai_viewer_version())

    # Create venv if it doesn't exist
    if not os.path.exists(venv_dir):
        print("Creating venv...")
        subprocess.check_call([sys.executable, "-m", "venv", venv_dir])

        # Install dependencies
        pip_executable = (
            os.path.join(venv_dir, "Scripts", "pip")
            if sys.platform == "win32"
            else os.path.join(venv_dir, "bin", "pip")
        )
        py_executable = (
            os.path.join(venv_dir, "Scripts", "python")
            if sys.platform == "win32"
            else os.path.join(venv_dir, "bin", "python")
        )
        # Install depthai_sdk first, then override depthai version with the one from requirements.txt
        subprocess.check_call(
            [
                pip_executable,
                "install",
                "git+https://github.com/luxonis/depthai.git@2825e21e179d7f01001b98693805ea50a80a50e1#subdirectory=depthai_sdk",
            ],
            stdout=subprocess.PIPE,
        )
        subprocess.check_call(
            [pip_executable, "install", "-r", f"{script_path}/requirements.txt"], stdout=subprocess.PIPE
        )

        packages_dir = site.getsitepackages()[0]
        # Create symlink for depthai_viewer and depthai_viewer_bindings
        venv_packages_dir = subprocess.check_output(
            [py_executable, "-c", "import site; print(site.getsitepackages()[0], end='')"]
        ).decode()
        os.symlink(os.path.join(packages_dir, "depthai_viewer"), os.path.join(venv_packages_dir, "depthai_viewer"))
        os.symlink(
            os.path.join(packages_dir, "depthai_viewer_bindings"),
            os.path.join(venv_packages_dir, "depthai_viewer_bindings"),
        )

    # Delete old requirements
    for item in os.listdir(os.path.join(venv_dir, "..")):
        if not item.startswith("venv-"):
            continue
        if item == os.path.basename(venv_dir):
            continue
        print(f"Removing old venv: {item}")
        shutil.rmtree(os.path.join(venv_dir, "..", item))

    # Return Python executable within the venv
    return (
        os.path.join(venv_dir, "Scripts", "python")
        if sys.platform == "win32"
        else os.path.join(venv_dir, "bin", "python")
    )


def main() -> None:
    python_executable = create_venv_and_install_dependencies()

    # We don't need to call shutdown in this case. Rust should be handling everything
    unregister_shutdown()

    # Call the bindings.main using the Python executable in the venv
    subprocess.call(
        [
            python_executable,
            "-c",
            f"from depthai_viewer import bindings; import sys; sys.exit(bindings.main(sys.argv, '{python_executable}'))",
        ]
        + sys.argv[1:]
    )


if __name__ == "__main__":
    main()
