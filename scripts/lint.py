#!/usr/bin/env python
"""
Runs custom linting on our code
"""

import argparse
import glob
import os
import re
import sys


def lint_file(filepath, args):
    with open(filepath) as f:
        lines_in = f.readlines()

    last_line_was_empty = True

    num_errors = 0

    todo_pattern = re.compile(r'TODO:')  # NOLINT

    for line_nr, line in enumerate(lines_in):
        if 'NOLINT' in line:
            continue

        line_nr = line_nr+1

        if todo_pattern.search(line):
            num_errors = + 1
            print(
                f'{filepath}:{line_nr}: TODO:s should contain the name of who wrote them, i.e. TODO(name):')  # NOLINT

    return num_errors


if __name__ == '__main__':
    parser = argparse.ArgumentParser(
        description='Lint code with custom linter.')
    parser.add_argument('files', metavar='file', type=str, nargs='*',
                        help='File paths. Empty = all files, recursively.')

    args = parser.parse_args()

    num_errors = 0

    if args.files:
        for filepath in args.files:
            num_errors += lint_file(filepath, args)
    else:
        script_dirpath = os.path.dirname(os.path.realpath(__file__))
        root_dirpath = os.path.abspath(f'{script_dirpath}/..')
        os.chdir(root_dirpath)

        exclude = set(['env', 'target'])

        for root, dirs, files in os.walk('.', topdown=True):
            dirs[:] = [d for d in dirs if d not in exclude]
            for filename in files:
                extension = filename.split('.')[-1]
                if extension in ['html', 'js', 'py', 'rs']:
                    filepath = os.path.join(root, filename)
                    num_errors += lint_file(filepath, args)

    if num_errors == 0:
        print('lint.py finished without error')
        sys.exit(0)
    else:
        sys.exit(1)
