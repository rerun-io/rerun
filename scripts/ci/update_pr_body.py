#!/usr/bin/env python3

"""
Script to update the PR description template.

This is expected to be run by the `reusable_update_pr_body.yml` GitHub workflow.

Requires the following packages:
  pip install Jinja2 PyGithub
"""
from __future__ import annotations

import argparse
import logging
import urllib.parse

from github import Github
from jinja2 import DebugUndefined, select_autoescape
from jinja2.sandbox import SandboxedEnvironment

DOCS_PREVIEW_MARKER = "<!--DOCS-PREVIEW-->"
DOCS_PREVIEW_BARE_LINK = "- [Docs preview](https://rerun.io/preview/{{ pr.commit }}/docs) <!--DOCS-PREVIEW-->"
EXAMPLES_PREVIEW_MARKER = "<!--EXAMPLES-PREVIEW-->"
EXAMPLES_PREVIEW_BARE_LINK = (
    "- [Examples preview](https://rerun.io/preview/{{ pr.commit }}/examples) <!--EXAMPLES-PREVIEW-->"
)

# Need to protect code-blocks in the PR template.
# See https://github.com/rerun-io/rerun/issues/3972
#
# Could not get this to work with jinja-markdown extension so replicating the functionality from:
# https://github.com/digital-land/design-system/commit/14e678ccb3e1e62da18072ccf035f3d0e7467f3c

CODE_BLOCK_PLACEHOLDER = "{CODE BLOCK PLACEHOLDER}"


def index_code_block(lines: list[str]) -> tuple[bool, int, int]:
    # Truncate the lines so we can find things like "```rust".
    line_starts = [line[0:3] for line in lines]
    if "```" in lines:
        opening_line = line_starts.index("```")
        closing_line = line_starts.index("```", opening_line + 1)
        return True, opening_line, closing_line
    return False, 0, 0


# extracts code blocks from lines of text
# also replaces code_blocks with a placeholder string
def extract_code_blocks(lines: list[str]) -> list[list[str]]:
    code_blocks = []
    has_code_blocks = True
    while has_code_blocks:
        # Truncate the lines so we can find things like "```rust".
        line_starts = [line[0:3] for line in lines]
        if "```" in line_starts:
            block, op, cl = index_code_block(lines)
            if block:
                code_blocks.append(lines[op : cl + 1])
                del lines[op + 1 : cl + 1]
                lines[op] = CODE_BLOCK_PLACEHOLDER
            else:
                has_code_blocks = False
        else:
            has_code_blocks = False
    return code_blocks


def insert_lines(lines: list[str], lines_to_insert: list[str], start_from: int) -> None:
    insert_pt = start_from
    for line in lines_to_insert:
        lines.insert(insert_pt, line)
        insert_pt = insert_pt + 1


def insert_code_blocks(lines: list[str], code_blocks: list[list[str]]) -> list[str]:
    has_placeholders = True
    while has_placeholders:
        if CODE_BLOCK_PLACEHOLDER in lines:
            idx = lines.index(CODE_BLOCK_PLACEHOLDER)
            try:
                block = code_blocks.pop(0)
                insert_lines(lines, block, idx + 1)
                del lines[idx]
            except IndexError:
                lines[idx] = "{Error: Couldn't re-insert code-block}"
        else:
            has_placeholders = False
    return lines


def encode_uri_component(value: str) -> str:
    return urllib.parse.quote(value, safe="")


def main() -> None:
    logging.getLogger().setLevel(-1)
    parser = argparse.ArgumentParser(description="Generate a PR summary page")
    parser.add_argument("--github-token", required=True, help="GitHub token")
    parser.add_argument("--github-repository", required=True, help="GitHub repository")
    parser.add_argument("--pr-number", required=True, type=int, help="PR number")
    args = parser.parse_args()

    gh = Github(args.github_token)
    repo = gh.get_repo(args.github_repository)
    pr = repo.get_pull(args.pr_number)
    if not pr.body:
        # body is empty
        return

    latest_commit = pr.get_commits().reversed[0]

    print(f"Latest commit: {latest_commit.sha}")

    env = SandboxedEnvironment(
        autoescape=select_autoescape(),
        undefined=DebugUndefined,
    )
    env.filters["encode_uri_component"] = encode_uri_component

    new_body = pr.body

    docs_preview_link_end = new_body.find(DOCS_PREVIEW_MARKER)
    if docs_preview_link_end != -1:
        docs_preview_link_end += len(DOCS_PREVIEW_MARKER)
        docs_preview_link_start = new_body.rfind("\n", 0, docs_preview_link_end) + 1
        new_body = new_body[:docs_preview_link_start] + DOCS_PREVIEW_BARE_LINK + new_body[docs_preview_link_end:]

    examples_preview_link_end = new_body.find(EXAMPLES_PREVIEW_MARKER)
    if examples_preview_link_end != -1:
        examples_preview_link_end += len(EXAMPLES_PREVIEW_MARKER)
        examples_preview_link_start = new_body.rfind("\n", 0, examples_preview_link_end) + 1
        new_body = (
            new_body[:examples_preview_link_start] + EXAMPLES_PREVIEW_BARE_LINK + new_body[examples_preview_link_end:]
        )

    lines = new_body.splitlines()
    codeblocks = extract_code_blocks(lines)
    text = "\n".join(lines)

    new_body = env.from_string(text).render(
        pr={
            "number": args.pr_number,
            "branch": pr.head.ref,
            "commit": latest_commit.sha,
        },
    )

    lines = new_body.split("\n")
    new_body = "\n".join(insert_code_blocks(lines, codeblocks))

    if new_body != pr.body:
        print("updated pr body")
        pr.edit(body=new_body)


if __name__ == "__main__":
    main()
