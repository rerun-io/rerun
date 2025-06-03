from __future__ import annotations

import subprocess


def generate_proto() -> None:
    proto_file = "CompressedVideo.proto"
    output_dir = "."

    # Create the command to generate Python code
    cmd = ["protoc", f"--python_out={output_dir}", proto_file]

    # Run the protoc command
    subprocess.run(cmd, check=True)
    print(f"Generated {proto_file.replace('.proto', '_pb2.py')}")


if __name__ == "__main__":
    generate_proto()
