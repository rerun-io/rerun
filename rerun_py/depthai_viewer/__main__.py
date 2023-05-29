"""See `python3 -m depthai-viewer --help`."""

import os
import shutil
import sysconfig
import subprocess
import sys
import traceback

from depthai_viewer import bindings, unregister_shutdown
from depthai_viewer import version as depthai_viewer_version  # type: ignore[attr-defined]


def create_venv_and_install_dependencies() -> str:
    script_path = os.path.dirname(os.path.abspath(__file__))
    venv_dir = os.path.join(script_path, "venv-" + depthai_viewer_version())
    try:
        # Create venv if it doesn't exist
        if not os.path.exists(venv_dir):
            print("Creating virtual environment...")
            subprocess.run([sys.executable, "-m", "venv", venv_dir], check=True)

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
            subprocess.run(
                [
                    pip_executable,
                    "install",
                    "git+https://github.com/luxonis/depthai.git@ac5b94eee908f5ea00c942e185fa135af771f82e#subdirectory=depthai_sdk",
                ],
                check=True,
            )
            subprocess.run(
                [pip_executable, "install", "-r", f"{script_path}/requirements.txt"],
                check=True,
            )

            packages_dir = sysconfig.get_paths()["purelib"]
            # Create symlink for depthai_viewer and depthai_viewer_bindings
            venv_packages_dir = subprocess.run(
                [py_executable, "-c", "import sysconfig; print(sysconfig.get_paths()['purelib'], end='')"],
                capture_output=True,
                text=True,
                check=True,
            ).stdout.strip()
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
        return os.path.normpath(
            os.path.join(venv_dir, "Scripts", "python")
            if sys.platform == "win32"
            else os.path.join(venv_dir, "bin", "python")
        )

    except Exception as e:
        print(f"Error occurred during the creation of the virtual environment or installation of dependencies: {e}")
        print(traceback.format_exc())
        # Attempt to delete the partially created venv
        try:
            if os.path.exists(venv_dir):
                print(f"Deleting partially created virtual environment: {venv_dir}")
                shutil.rmtree(venv_dir)
        except Exception as e:
            print(f"Error occurred while attempting to delete the virtual environment: {e}")
            print(traceback.format_exc())
        exit(1)


def main() -> None:
    python_exe = create_venv_and_install_dependencies()
    # Call the bindings.main using the Python executable in the venv
    unregister_shutdown()
    subprocess.call(
        [
            python_exe,
            "-c",
            f"from depthai_viewer import bindings, unregister_shutdown; import sys; unregister_shutdown(); sys.exit(bindings.main(sys.argv, r'{python_exe}'))",
        ]
        + sys.argv[1:]
    )


if __name__ == "__main__":
    main()
