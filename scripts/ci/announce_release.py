#!/usr/bin/env python3

"""
Announce a published Rerun release on Discord via an incoming webhook.

The webhook URL is read from the `DISCORD_WEBHOOK_URL` environment variable.

Usage:
    export RELEASE_BODY="$(gh release view 0.39.0 --repo rerun-io/rerun --json body -q .body)"
    python3 scripts/ci/announce_release.py --title 0.39.0 --url <release page>
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.request

# Discord rejects messages longer than 2000 characters.
DISCORD_SUMMARY_LIMIT = 1500


def summary(body: str) -> str:
    """
    The first paragraph or list of the release notes' highlights.

    Starts after the `Overview & highlights` heading if there is one, otherwise after the install header that
    precedes the first `---`. Skips headings, and image or video embeds.
    """
    lines = body.replace("\r\n", "\n").split("\n")
    highlights = next(
        (i for i, line in enumerate(lines) if line.startswith("#") and "highlights" in line.lower()), None
    )
    if highlights is not None:
        lines = lines[highlights + 1 :]
    elif "---" in lines:
        lines = lines[lines.index("---") + 1 :]
    block: list[str] = []
    for line in lines:
        stripped = line.strip()
        if not stripped:
            if block:
                break
            continue
        if stripped.startswith(("#", "<", "![")) or re.fullmatch(r"https?://\S+\.(?:mp4|webm|gif|png|jpe?g)", stripped):
            if block:
                break
            continue
        block.append(line.rstrip())
    return "\n".join(block)


def truncate(text: str, limit: int) -> str:
    if len(text) <= limit:
        return text
    cut = text.rfind("\n", 0, limit)
    return text[: cut if cut > 0 else limit].rstrip() + "\n…"


def post_webhook(webhook_url: str, payload: dict[str, object]) -> None:
    if not webhook_url.startswith("https://"):
        raise SystemExit("The webhook URL must be https")
    request = urllib.request.Request(  # noqa: S310
        webhook_url,
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json", "User-Agent": "rerun-release-bot"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=30) as response:  # noqa: S310
        print(f"Posted (HTTP {response.status})")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--title", required=True, help="The GitHub release title")
    parser.add_argument("--url", required=True, help="The GitHub release page")
    args = parser.parse_args()

    webhook_url = os.environ.get("DISCORD_WEBHOOK_URL", "")
    if not webhook_url:
        print("::error::DISCORD_WEBHOOK_URL is not set")
        return 1

    notes = truncate(summary(os.environ.get("RELEASE_BODY", "")), DISCORD_SUMMARY_LIMIT)
    content = f"**Rerun {args.title}** is out!\n{args.url}"
    if notes:
        content += f"\n\n{notes}"
    post_webhook(webhook_url, {"content": content, "allowed_mentions": {"parse": []}})
    return 0


if __name__ == "__main__":
    sys.exit(main())
