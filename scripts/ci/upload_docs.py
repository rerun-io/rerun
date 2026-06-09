#!/usr/bin/env python3

"""
Manage Rerun documentation and examples in GCS for the rerun.io website.

The website (rerun-io/landing) reads its docs and examples from GCS at
`gs://rerun-docs/prose/`, exposed publicly as `https://ref.rerun.io/prose/`.

Subcommands:
  upload   Build and upload a version (files + index.json), optionally
           promote to `latest`, and trigger revalidation.
  delete   Remove a version (files + entry in versions.json) and
           trigger revalidation.

Usable both from CI and locally. Run from anywhere; paths are resolved
relative to the script's monorepo location.

Install dependencies (already present in this repo's uv workspace):
    uv sync

Examples:
    uv run scripts/ci/upload_docs.py upload --version 0.21.0 --mark-latest --purge-token "$ISR_BYPASS_TOKEN"
    uv run scripts/ci/upload_docs.py upload --version pr-1234 --skip-purge
    uv run scripts/ci/upload_docs.py delete --version test-local --skip-purge
"""

from __future__ import annotations

import argparse
import gzip
import io
import json
import mimetypes
import re
import subprocess
import sys
from collections.abc import Iterable
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import requests
import requests.adapters
import yaml
from google.cloud import storage

SCHEMA_VERSION = 1

BUCKET_NAME = "rerun-docs"
GCS_PREFIX = "prose"
DEFAULT_SITE_URL = "https://rerun.io"

# Files that should be uploaded for each subtree. The website only reads
# these paths; uploading anything else just wastes space.
DOC_CONTENT_GLOBS = ("**/*.md",)
DOC_CONTENT_EXTRA_FILES = ("_redirects.yaml",)
SNIPPET_FILES = ("snippets.toml",)
SNIPPET_SOURCE_EXTS = (".py", ".rs", ".cpp")
EXAMPLE_FILES = ("manifest.toml",)
EXAMPLE_README_GLOB = "*/*/README.md"

PR_VERSION_RE = re.compile(r"^pr-\d+$")


# ---------------------------------------------------------------------------
# index.json construction
# ---------------------------------------------------------------------------


@dataclass
class FileEntry:
    """A single file to upload, plus its archive-relative path."""

    archive_path: str  # e.g. "docs/content/getting-started/quick-start.md"
    source: Path  # absolute path on disk
    is_doc_markdown: bool  # true for docs/content/**.md (parse frontmatter)


def collect_files(rerun_root: Path) -> list[FileEntry]:
    files: list[FileEntry] = []

    docs_content = rerun_root / "docs" / "content"
    for pattern in DOC_CONTENT_GLOBS:
        for md in docs_content.glob(pattern):
            if md.is_file():
                rel = md.relative_to(rerun_root).as_posix()
                files.append(FileEntry(rel, md, is_doc_markdown=True))
    for name in DOC_CONTENT_EXTRA_FILES:
        path = docs_content / name
        if path.is_file():
            rel = path.relative_to(rerun_root).as_posix()
            files.append(FileEntry(rel, path, is_doc_markdown=False))

    snippets_dir = rerun_root / "docs" / "snippets"
    for name in SNIPPET_FILES:
        path = snippets_dir / name
        if path.is_file():
            rel = path.relative_to(rerun_root).as_posix()
            files.append(FileEntry(rel, path, is_doc_markdown=False))
    snippets_all = snippets_dir / "all"
    if snippets_all.is_dir():
        for path in snippets_all.rglob("*"):
            if path.is_file() and path.suffix in SNIPPET_SOURCE_EXTS:
                rel = path.relative_to(rerun_root).as_posix()
                files.append(FileEntry(rel, path, is_doc_markdown=False))

    examples_dir = rerun_root / "examples"
    for name in EXAMPLE_FILES:
        path = examples_dir / name
        if path.is_file():
            rel = path.relative_to(rerun_root).as_posix()
            files.append(FileEntry(rel, path, is_doc_markdown=False))
    for readme in examples_dir.glob(EXAMPLE_README_GLOB):
        if readme.is_file():
            rel = readme.relative_to(rerun_root).as_posix()
            files.append(FileEntry(rel, readme, is_doc_markdown=False))

    return files


def parse_doc_frontmatter(md_path: Path) -> dict[str, Any]:
    """Parse YAML frontmatter from a docs markdown file.

    Returns the metadata dict expected by the website: title, order,
    hidden, expand, redirect.
    """
    text = md_path.read_text(encoding="utf-8")
    if not text.startswith("---"):
        raise ValueError(f"{md_path}: missing YAML frontmatter")
    end = text.find("\n---", 3)
    if end == -1:
        raise ValueError(f"{md_path}: unterminated YAML frontmatter")
    raw = text[3:end].strip()
    fm = yaml.safe_load(raw) or {}
    if not isinstance(fm, dict):
        raise ValueError(f"{md_path}: frontmatter is not a mapping")

    if "title" not in fm:
        raise ValueError(f"{md_path}: frontmatter missing required 'title'")

    metadata: dict[str, Any] = {
        "title": fm["title"],
        "hidden": bool(fm.get("hidden", False)),
        "expand": bool(fm.get("expand", False)),
    }
    if "order" in fm and fm["order"] is not None:
        metadata["order"] = fm["order"]
    if "redirect" in fm and fm["redirect"] is not None:
        metadata["redirect"] = fm["redirect"]
    return metadata


def load_redirects(rerun_root: Path) -> dict[str, str]:
    path = rerun_root / "docs" / "content" / "_redirects.yaml"
    if not path.is_file():
        return {}
    data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    if not isinstance(data, dict):
        raise ValueError(f"{path}: top-level value must be a mapping")
    return {str(k): str(v) for k, v in data.items()}


def build_index(
    *,
    version: str,
    rerun_commit: str,
    files: list[FileEntry],
    redirects: dict[str, str],
) -> dict[str, Any]:
    entries: dict[str, dict[str, Any]] = {}
    dir_children: dict[str, set[str]] = {}

    def ensure_dir(dir_path: str) -> None:
        """Make sure `dir_path` (ending in '/') and all its parents exist
        in the entries map, and that each parent dir lists the child."""
        if dir_path in entries:
            return
        # Add this dir.
        entries[dir_path] = {"kind": "dir"}
        dir_children.setdefault(dir_path, set())
        # Recurse up to root and register this as a child of its parent.
        without_trailing = dir_path[:-1]
        if "/" in without_trailing:
            parent = without_trailing.rsplit("/", 1)[0] + "/"
            ensure_dir(parent)
            child_basename = without_trailing.rsplit("/", 1)[1] + "/"
            dir_children[parent].add(child_basename)
        # Top-level dirs have no parent in the index.

    for f in files:
        # Register all parent directories.
        if "/" in f.archive_path:
            parent = f.archive_path.rsplit("/", 1)[0] + "/"
            ensure_dir(parent)
            dir_children[parent].add(f.archive_path.rsplit("/", 1)[1])

        entry: dict[str, Any] = {"kind": "file"}
        if f.is_doc_markdown:
            entry["metadata"] = parse_doc_frontmatter(f.source)
        entries[f.archive_path] = entry

    # Flush children lists into the dir entries.
    for dir_path, children in dir_children.items():
        entries[dir_path]["children"] = sorted(children)

    return {
        "schema_version": SCHEMA_VERSION,
        "version": version,
        "last_update": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "rerun_commit": rerun_commit,
        "entries": entries,
        "redirects": redirects,
    }


# ---------------------------------------------------------------------------
# Upload
# ---------------------------------------------------------------------------


def content_type_for(path: str) -> str:
    if path.endswith(".md"):
        return "text/markdown; charset=utf-8"
    if path.endswith(".toml"):
        return "application/toml; charset=utf-8"
    if path.endswith((".yaml", ".yml")):
        return "application/yaml; charset=utf-8"
    if path.endswith(".json"):
        return "application/json; charset=utf-8"
    if path.endswith((".py", ".rs", ".cpp")):
        return "text/plain; charset=utf-8"
    guessed, _ = mimetypes.guess_type(path)
    return guessed or "application/octet-stream"


def gzip_bytes(data: bytes) -> bytes:
    buf = io.BytesIO()
    # mtime=0 so re-uploading the same content produces identical bytes.
    with gzip.GzipFile(fileobj=buf, mode="wb", mtime=0) as gz:
        gz.write(data)
    return buf.getvalue()


def upload_blob(
    bucket: storage.Bucket,
    *,
    gcs_path: str,
    payload: bytes,
    content_type: str,
    cache_control: str = "no-cache",
) -> None:
    """Upload `payload` (already gzipped) under `gcs_path`."""
    blob = bucket.blob(gcs_path)
    blob.content_encoding = "gzip"
    blob.content_type = content_type
    blob.cache_control = cache_control
    blob.upload_from_string(payload, content_type=content_type)


def upload_files(
    bucket: storage.Bucket,
    version: str,
    files: list[FileEntry],
    *,
    dry_run: bool,
    concurrency: int = 128,
) -> None:
    prefix = f"{GCS_PREFIX}/{version}/"

    def upload_one(f: FileEntry) -> str:
        gcs_path = prefix + f.archive_path
        if dry_run:
            return f"DRY-RUN: {gcs_path}"
        payload = gzip_bytes(f.source.read_bytes())
        upload_blob(
            bucket,
            gcs_path=gcs_path,
            payload=payload,
            content_type=content_type_for(f.archive_path),
        )
        return gcs_path

    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [pool.submit(upload_one, f) for f in files]
        for i, fut in enumerate(as_completed(futures), 1):
            path = fut.result()
            if i % 50 == 0 or i == len(futures):
                print(f"  [{i}/{len(futures)}] {path}")


def upload_index(bucket: storage.Bucket, version: str, index: dict[str, Any], *, dry_run: bool) -> None:
    gcs_path = f"{GCS_PREFIX}/{version}/index.json"
    if dry_run:
        print(f"DRY-RUN: would write {gcs_path} ({len(index['entries'])} entries)")
        return
    payload = gzip_bytes(json.dumps(index, indent=2).encode("utf-8"))
    upload_blob(
        bucket,
        gcs_path=gcs_path,
        payload=payload,
        content_type="application/json; charset=utf-8",
    )
    print(f"wrote {gcs_path}")


def update_versions_manifest(
    bucket: storage.Bucket,
    *,
    version: str,
    rerun_commit: str,
    mark_latest: bool,
    dry_run: bool,
) -> None:
    gcs_path = f"{GCS_PREFIX}/versions.json"
    blob = bucket.blob(gcs_path)

    if blob.exists():
        # GCS auto-decompresses on download.
        raw = blob.download_as_bytes()
        manifest = json.loads(raw)
    else:
        manifest = {"schema_version": SCHEMA_VERSION, "latest": version, "versions": {}}

    manifest.setdefault("schema_version", SCHEMA_VERSION)
    manifest.setdefault("versions", {})
    manifest["versions"][version] = {"rerun_commit": rerun_commit}
    if mark_latest or "latest" not in manifest:
        manifest["latest"] = version

    if dry_run:
        print(f"DRY-RUN: would update {gcs_path}: {json.dumps(manifest, indent=2)}")
        return

    payload = gzip_bytes(json.dumps(manifest, indent=2).encode("utf-8"))
    upload_blob(
        bucket,
        gcs_path=gcs_path,
        payload=payload,
        content_type="application/json; charset=utf-8",
    )
    print(f"wrote {gcs_path} (latest={manifest['latest']})")


# ---------------------------------------------------------------------------
# Revalidation webhook
# ---------------------------------------------------------------------------


def trigger_revalidate(
    *,
    site_url: str,
    token: str,
    target: dict[str, Any],
    dry_run: bool,
) -> None:
    endpoint = site_url.rstrip("/") + "/api/revalidate"

    if dry_run:
        print(f"DRY-RUN: would POST {endpoint} body={target}")
        return

    resp = requests.post(
        endpoint,
        json=target,
        headers={"Authorization": f"Bearer {token}"},
        timeout=30,
    )
    resp.raise_for_status()
    print(f"revalidation OK: {resp.status_code}")


def delete_version_files(bucket: storage.Bucket, version: str, *, dry_run: bool, concurrency: int = 128) -> int:
    prefix = f"{GCS_PREFIX}/{version}/"
    blobs = list(bucket.list_blobs(prefix=prefix))
    if not blobs:
        print(f"  no objects under {prefix}")
        return 0

    def delete_one(blob: storage.Blob) -> str:
        if dry_run:
            return f"DRY-RUN: {blob.name}"
        blob.delete()
        return str(blob.name)

    with ThreadPoolExecutor(max_workers=concurrency) as pool:
        futures = [pool.submit(delete_one, b) for b in blobs]
        for i, fut in enumerate(as_completed(futures), 1):
            name = fut.result()
            if i % 100 == 0 or i == len(futures):
                print(f"  [{i}/{len(futures)}] {name}")
    return len(blobs)


def remove_version_from_manifest(bucket: storage.Bucket, *, version: str, dry_run: bool) -> bool:
    """Remove `version` from versions.json. Returns True if `latest` was
    affected (caller should also purge the unversioned routes)."""
    gcs_path = f"{GCS_PREFIX}/versions.json"
    blob = bucket.blob(gcs_path)
    if not blob.exists():
        print(f"  {gcs_path} does not exist; nothing to update")
        return False

    manifest = json.loads(blob.download_as_bytes())
    versions = manifest.get("versions", {})
    if version not in versions:
        print(f"  {version} not in versions.json; nothing to update")
        return False

    if manifest.get("latest") == version:
        raise SystemExit(
            f"refusing to delete {version}: it is currently `latest`. "
            f"Promote another version with `upload --mark-latest` first."
        )

    del versions[version]

    if dry_run:
        print(f"DRY-RUN: would update {gcs_path}: {json.dumps(manifest, indent=2)}")
        return False

    payload = gzip_bytes(json.dumps(manifest, indent=2).encode("utf-8"))
    upload_blob(
        bucket,
        gcs_path=gcs_path,
        payload=payload,
        content_type="application/json; charset=utf-8",
    )
    print(f"wrote {gcs_path} (removed {version})")
    return False


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def make_storage_client(pool_size: int) -> storage.Client:
    """Storage client whose HTTP session has a connection pool big enough
    for `pool_size` concurrent in-flight requests. The default `requests`
    pool is 10, so without this the worker threads serialize behind it."""
    client = storage.Client()
    adapter = requests.adapters.HTTPAdapter(pool_connections=pool_size, pool_maxsize=pool_size, max_retries=3)
    client._http.mount("https://", adapter)
    client._http.mount("http://", adapter)
    return client


def detect_rerun_root(script_path: Path) -> Path:
    # scripts/ci/upload_docs.py -> ../../
    return script_path.resolve().parent.parent.parent


def detect_rerun_commit(rerun_root: Path) -> str:
    try:
        sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=rerun_root, text=True).strip()
        return sha
    except Exception as e:
        raise SystemExit(f"failed to detect rerun commit via git: {e}")


def add_common_args(p: argparse.ArgumentParser) -> None:
    p.add_argument("--version", required=True, help="version label, e.g. 0.21.0 / main / nightly / pr-1234")
    p.add_argument("--purge-token", help="bearer token for the website's /api/revalidate endpoint")
    p.add_argument("--skip-purge", action="store_true", help="skip the revalidation webhook (local testing)")
    p.add_argument("--site-url", default=DEFAULT_SITE_URL, help=f"website base URL (default: {DEFAULT_SITE_URL})")
    p.add_argument("--bucket", default=BUCKET_NAME, help=f"GCS bucket (default: {BUCKET_NAME})")
    p.add_argument("--concurrency", type=int, default=128, help="parallel GCS operations (default: 128)")
    p.add_argument("--dry-run", action="store_true", help="do not upload, delete, or call any webhook")


def cmd_upload(args: argparse.Namespace) -> int:
    is_pr_preview = bool(PR_VERSION_RE.match(args.version))
    if is_pr_preview and args.mark_latest:
        raise SystemExit("--mark-latest is incompatible with a pr-* version")

    rerun_root = Path(args.rerun_root).resolve() if args.rerun_root else detect_rerun_root(Path(__file__))
    if not (rerun_root / "docs" / "content").is_dir():
        raise SystemExit(f"could not locate docs/content under {rerun_root}")
    rerun_commit = args.rerun_commit or detect_rerun_commit(rerun_root)

    print(f"rerun root:    {rerun_root}")
    print(f"version:       {args.version}{'  (PR preview)' if is_pr_preview else ''}")
    print(f"rerun commit:  {rerun_commit}")
    print(f"bucket:        gs://{args.bucket}/{GCS_PREFIX}/{args.version}/")
    print(f"site:          {args.site_url}")
    print(f"dry-run:       {args.dry_run}")
    print()

    print("scanning files…")
    files = collect_files(rerun_root)
    print(f"  found {len(files)} files")

    print("building index.json…")
    redirects = load_redirects(rerun_root)
    index = build_index(
        version=args.version,
        rerun_commit=rerun_commit,
        files=files,
        redirects=redirects,
    )
    print(f"  {len(index['entries'])} entries, {len(redirects)} redirects")

    client = make_storage_client(args.concurrency)
    bucket = client.bucket(args.bucket)

    print("uploading files…")
    upload_files(bucket, args.version, files, dry_run=args.dry_run, concurrency=args.concurrency)

    print("uploading index.json…")
    upload_index(bucket, args.version, index, dry_run=args.dry_run)

    if is_pr_preview:
        print("PR preview: skipping versions.json update")
    else:
        print("updating versions.json…")
        update_versions_manifest(
            bucket,
            version=args.version,
            rerun_commit=rerun_commit,
            mark_latest=args.mark_latest,
            dry_run=args.dry_run,
        )

    if args.skip_purge:
        print("skipping revalidation webhook (--skip-purge)")
    else:
        target: dict[str, Any] = (
            {"target": "latest"} if args.mark_latest else {"target": "version", "version": args.version}
        )
        print("triggering revalidation…")
        trigger_revalidate(site_url=args.site_url, token=args.purge_token, target=target, dry_run=args.dry_run)

    print("done.")
    return 0


def cmd_delete(args: argparse.Namespace) -> int:
    is_pr_preview = bool(PR_VERSION_RE.match(args.version))

    print(f"version:       {args.version}{'  (PR preview)' if is_pr_preview else ''}")
    print(f"bucket:        gs://{args.bucket}/{GCS_PREFIX}/{args.version}/")
    print(f"site:          {args.site_url}")
    print(f"dry-run:       {args.dry_run}")
    print()

    client = make_storage_client(args.concurrency)
    bucket = client.bucket(args.bucket)

    # Update manifest first so concurrent readers stop discovering the
    # version before we start tearing down its files. (PR previews aren't
    # in versions.json, so this is a no-op for them.)
    if not is_pr_preview:
        print("updating versions.json…")
        remove_version_from_manifest(bucket, version=args.version, dry_run=args.dry_run)

    print("deleting files…")
    deleted = delete_version_files(bucket, args.version, dry_run=args.dry_run, concurrency=args.concurrency)
    print(f"  deleted {deleted} objects")

    if args.skip_purge:
        print("skipping revalidation webhook (--skip-purge)")
    else:
        print("triggering revalidation…")
        trigger_revalidate(
            site_url=args.site_url,
            token=args.purge_token,
            target={"target": "version", "version": args.version},
            dry_run=args.dry_run,
        )

    print("done.")
    return 0


def main(argv: Iterable[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Manage Rerun docs/examples in GCS for the rerun.io website.")
    sub = p.add_subparsers(dest="command", required=True)

    pu = sub.add_parser("upload", help="build & upload a version")
    add_common_args(pu)
    pu.add_argument("--mark-latest", action="store_true", help="set this version as `latest` in versions.json")
    pu.add_argument("--rerun-commit", help="override the rerun commit SHA (default: git HEAD)")
    pu.add_argument(
        "--rerun-root", help="path to a rerun checkout to read docs/examples from (default: this monorepo's rerun/)"
    )

    pd = sub.add_parser("delete", help="delete a version")
    add_common_args(pd)

    args = p.parse_args(list(argv) if argv is not None else None)

    if not args.skip_purge and not args.purge_token:
        raise SystemExit("either --purge-token or --skip-purge is required")

    if args.command == "upload":
        return cmd_upload(args)
    if args.command == "delete":
        return cmd_delete(args)
    raise SystemExit(f"unknown command: {args.command}")


if __name__ == "__main__":
    sys.exit(main())
