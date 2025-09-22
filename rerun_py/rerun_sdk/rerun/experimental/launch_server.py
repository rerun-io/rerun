"""
Robot generated server wrapper.

This is bad. Don't merge it but it is useful for experimentation.
"""

from __future__ import annotations

import atexit
import os
import signal
import subprocess
import sys
import time
from typing import Any

# Global variable to track the server process
server_process = None


def cleanup_server() -> None:
    """Clean up the server process."""
    global server_process
    if server_process and server_process.poll() is None:
        print(f"Terminating rerun server (PID: {server_process.pid})…")

        try:
            # Kill the entire process group to ensure all child processes are terminated
            os.killpg(os.getpgid(server_process.pid), signal.SIGTERM)
            print("Sent SIGTERM to process group.")

            # Wait for graceful termination
            try:
                server_process.wait(timeout=3)
                print("Rerun server terminated gracefully.")
                return
            except subprocess.TimeoutExpired:
                print("Server didn't terminate gracefully, force killing…")

            # Force kill the process group
            os.killpg(os.getpgid(server_process.pid), signal.SIGKILL)
            server_process.wait(timeout=2)
            print("Rerun server force killed.")

        except (ProcessLookupError, OSError) as e:
            print(f"Process group cleanup failed: {e}")

            # Fallback to individual process termination
            try:
                server_process.terminate()
                server_process.wait(timeout=3)
                print("Server terminated using fallback method.")
            except subprocess.TimeoutExpired:
                server_process.kill()
                server_process.wait()
                print("Server force killed using fallback method.")

        # Additional cleanup: try to kill any remaining processes
        try:
            # Use pkill to find and kill any remaining rerun server processes
            result = os.system("pkill -f 'rerun server' 2>/dev/null")
            if result == 0:
                print("Killed remaining rerun server processes using pkill.")
        except Exception as e:
            print(f"Could not kill remaining processes: {e}")

    elif server_process:
        print("Rerun server process already terminated.")
    else:
        print("No rerun server process to terminate.")


def signal_handler(signum: int, frame: Any) -> None:
    """Handle signals to ensure cleanup."""
    cleanup_server()
    sys.exit(0)


def start_rerun_server(dataset_path: str) -> None:
    """Start the rerun server in the background."""
    global server_process

    cmd = ["rerun", "server", "-d", dataset_path]
    print(f"Starting rerun server: {' '.join(cmd)}")

    try:
        # Start server in a new process group to make cleanup easier
        server_process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            preexec_fn=os.setsid,  # Create new process group on Unix
        )

        # Register cleanup functions
        atexit.register(cleanup_server)
        signal.signal(signal.SIGINT, signal_handler)
        signal.signal(signal.SIGTERM, signal_handler)

        # Give the server a moment to start
        time.sleep(1)

        # Check if the server started successfully
        if server_process.poll() is None:
            print(f"Rerun server started successfully (PID: {server_process.pid})")
        else:
            stdout, stderr = server_process.communicate()
            print(f"Failed to start rerun server. Exit code: {server_process.returncode}")
            if stdout:
                print(f"STDOUT: {stdout}")
            if stderr:
                print(f"STDERR: {stderr}")
            server_process = None

    except FileNotFoundError as e:
        raise ValueError("Error: 'rerun' command not found. Make sure rerun is installed and in your PATH.") from e
    except Exception as e:
        raise ValueError(f"Error starting rerun server: {e}") from e
