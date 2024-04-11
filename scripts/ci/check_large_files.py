from __future__ import annotations
import os
import subprocess

#!/usr/bin/env python3

# Check for files that are too large to be checked into the repository.
# Whenever we want to make an exception, we add it to `check_large_files_allow_list.txt`

# Maximum file size, unless found in `check_large_files_allow_list.txt`
maximum_size = 100 * 1024

result = 0
script_path = os.path.dirname(os.path.realpath(__file__))
os.chdir(os.path.join(script_path, "../.."))

# Get the list of tracked files using git ls-files command
tracked_files = subprocess.check_output(["git", "ls-files"]).decode().splitlines()

for file_path in tracked_files:
    actual_size = os.path.getsize(file_path)

    if actual_size >= maximum_size:
        allow_list_path = os.path.join(script_path, "check_large_files_allow_list.txt")

        with open(allow_list_path) as allow_list_file:
            allow_list = allow_list_file.read().splitlines()

        if file_path not in allow_list:
            print(f"{file_path} is {actual_size} bytes (max allowed is {maximum_size} bytes)")
            result = 1

print("checked {} files".format(len(tracked_files)))
exit(result)
