"""GitHub CLI integration for workflow dispatch and status.

This module provides a Python interface to the `gh` CLI for:
- Triggering workflow_dispatch events
- Checking workflow run status
- Validating workflow file sync between local and remote

Requires the `gh` CLI to be installed and authenticated.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .result import Err, Ok, Result


class GitHubError(Exception):
    """Error interacting with GitHub."""


@dataclass
class WorkflowRun:
    """Information about a GitHub Actions workflow run."""

    id: int
    name: str
    status: str  # queued, in_progress, completed
    conclusion: str | None  # success, failure, cancelled, skipped, etc.
    head_branch: str
    head_sha: str
    url: str
    created_at: str
    updated_at: str

    @property
    def display_status(self) -> str:
        """Get a human-readable status string."""
        if self.status == "completed":
            return self.conclusion or "completed"
        return self.status


def _run_gh(*args: str, capture_json: bool = False) -> Result[str | dict[str, Any] | list[Any]]:
    """
    Run a gh CLI command.

    Args:
        args: Command arguments (e.g., "workflow", "list")
        capture_json: If True, parse output as JSON

    Returns:
        Result containing stdout (or parsed JSON) on success, error message on failure

    """
    cmd = ["gh", *args]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=60,
        )
    except FileNotFoundError:
        return Err("GitHub CLI (gh) not found. Install from https://cli.github.com/ and run 'gh auth login'")
    except subprocess.TimeoutExpired:
        return Err(f"Command timed out: gh {' '.join(args)}")

    if result.returncode != 0:
        error_msg = (
            result.stderr.strip() or result.stdout.strip() or f"Command failed with exit code {result.returncode}"
        )
        return Err(error_msg)

    output = result.stdout.strip()

    if capture_json:
        try:
            return Ok(json.loads(output))
        except json.JSONDecodeError as e:
            return Err(f"Failed to parse JSON output: {e}")

    return Ok(output)


GH_NOT_FOUND_ERROR = (
    "GitHub CLI (gh) not found.\n\n"
    "The --remote and --status flags require the GitHub CLI.\n"
    "Install from: https://cli.github.com/\n"
    "Then run: gh auth login"
)


def is_gh_installed() -> bool:
    """Check if the gh CLI is installed (doesn't check authentication)."""
    import shutil

    return shutil.which("gh") is not None


def check_gh_available() -> Result[str]:
    """
    Check if gh CLI is available and authenticated.

    Returns:
        Result containing the authenticated user on success

    """
    if not is_gh_installed():
        return Err(GH_NOT_FOUND_ERROR)

    result = _run_gh("auth", "status", "--show-token")
    if result.failed:
        # Provide more context for auth errors
        error = str(result.error)
        if "not logged in" in error.lower() or "auth" in error.lower():
            return Err(f"GitHub CLI not authenticated.\n\nRun: gh auth login\n\nDetails: {error}")
        return Err(error)
    return Ok(str(result.value()))


def get_repo_info() -> Result[tuple[str, str]]:
    """
    Get the owner and repo name for the current directory.

    Returns:
        Result containing (owner, repo) tuple

    """
    result = _run_gh("repo", "view", "--json", "owner,name", capture_json=True)
    if result.failed:
        return Err(f"Not in a GitHub repository or not authenticated: {result.error}")

    data = result.value()
    if isinstance(data, dict):
        owner = data.get("owner", {}).get("login", "")
        name = data.get("name", "")
        if owner and name:
            return Ok((owner, name))

    return Err("Could not determine repository owner/name")


def list_workflow_runs(
    workflow_name: str | None = None,
    limit: int = 10,
    branch: str | None = None,
) -> Result[list[WorkflowRun]]:
    """
    List recent workflow runs.

    Args:
        workflow_name: Filter by workflow filename (e.g., "recompose_flow_ci.yml")
        limit: Maximum number of runs to return
        branch: Filter by branch name

    Returns:
        Result containing list of WorkflowRun objects

    """
    args = ["run", "list", "--json", "databaseId,name,status,conclusion,headBranch,headSha,url,createdAt,updatedAt"]
    args.extend(["--limit", str(limit)])

    if workflow_name:
        args.extend(["--workflow", workflow_name])
    if branch:
        args.extend(["--branch", branch])

    result = _run_gh(*args, capture_json=True)
    if result.failed:
        return Err(str(result.error))

    data = result.value()
    if not isinstance(data, list):
        return Err(f"Unexpected response format: {type(data)}")

    runs = []
    for item in data:
        runs.append(
            WorkflowRun(
                id=item["databaseId"],
                name=item["name"],
                status=item["status"],
                conclusion=item.get("conclusion"),
                head_branch=item["headBranch"],
                head_sha=item["headSha"],
                url=item["url"],
                created_at=item["createdAt"],
                updated_at=item["updatedAt"],
            )
        )

    return Ok(runs)


def get_workflow_run(run_id: int) -> Result[WorkflowRun]:
    """
    Get details of a specific workflow run.

    Args:
        run_id: The workflow run ID

    Returns:
        Result containing WorkflowRun object

    """
    json_fields = "databaseId,name,status,conclusion,headBranch,headSha,url,createdAt,updatedAt"
    args = ["run", "view", str(run_id), "--json", json_fields]

    result = _run_gh(*args, capture_json=True)
    if result.failed:
        return Err(str(result.error))

    item = result.value()
    if not isinstance(item, dict):
        return Err(f"Unexpected response format: {type(item)}")

    return Ok(
        WorkflowRun(
            id=item["databaseId"],
            name=item["name"],
            status=item["status"],
            conclusion=item.get("conclusion"),
            head_branch=item["headBranch"],
            head_sha=item["headSha"],
            url=item["url"],
            created_at=item["createdAt"],
            updated_at=item["updatedAt"],
        )
    )


def trigger_workflow(
    workflow_name: str,
    ref: str | None = None,
    inputs: dict[str, str] | None = None,
) -> Result[str]:
    """
    Trigger a workflow_dispatch event.

    Args:
        workflow_name: Workflow filename (e.g., "recompose_flow_ci.yml")
        ref: Git ref to run against (branch/tag). Defaults to default branch.
        inputs: Input parameters for the workflow

    Returns:
        Result containing success message or error

    """
    args = ["workflow", "run", workflow_name]

    if ref:
        args.extend(["--ref", ref])

    if inputs:
        for key, value in inputs.items():
            args.extend(["-f", f"{key}={value}"])

    result = _run_gh(*args)
    if result.failed:
        return Err(str(result.error))

    return Ok(f"Triggered workflow {workflow_name}")


def get_workflow_file_content(workflow_path: str) -> Result[str]:
    """
    Get the content of a workflow file from the remote repository.

    Args:
        workflow_path: Path to workflow file (e.g., ".github/workflows/ci.yml")

    Returns:
        Result containing the file content as string

    """
    # Use gh api to get file content
    args = ["api", f"repos/{{owner}}/{{repo}}/contents/{workflow_path}", "--jq", ".content"]

    result = _run_gh(*args)
    if result.failed:
        return Err(str(result.error))

    # Content is base64 encoded
    import base64

    content_b64 = str(result.value()).strip()
    try:
        content = base64.b64decode(content_b64).decode("utf-8")
        return Ok(content)
    except Exception as e:
        return Err(f"Failed to decode file content: {e}")


def get_current_branch() -> Result[str]:
    """Get the current git branch name."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            return Err(result.stderr.strip() or "Failed to get current branch")
        return Ok(result.stdout.strip())
    except FileNotFoundError:
        return Err("git not found")
    except subprocess.TimeoutExpired:
        return Err("git command timed out")


def find_git_root() -> Path | None:
    """Find the git repository root directory."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode == 0:
            return Path(result.stdout.strip())
        return None
    except (FileNotFoundError, subprocess.TimeoutExpired):
        return None


def get_default_branch() -> Result[str]:
    """Get the default branch of the repository."""
    result = _run_gh("repo", "view", "--json", "defaultBranchRef", "--jq", ".defaultBranchRef.name")
    if result.failed:
        return Err(str(result.error))
    return Ok(str(result.value()).strip())


def validate_workflow_sync(local_path: Path, remote_path: str) -> Result[bool]:
    """
    Check if local workflow file matches the remote version.

    Args:
        local_path: Path to local workflow file
        remote_path: Path in repository (e.g., ".github/workflows/ci.yml")

    Returns:
        Result containing True if in sync, or Err with details if not

    """
    # Read local file
    if not local_path.exists():
        return Err(f"Local workflow file not found: {local_path}")

    local_content = local_path.read_text()

    # Get remote file
    remote_result = get_workflow_file_content(remote_path)
    if remote_result.failed:
        # File doesn't exist on remote - that's a sync issue
        return Err(f"Remote workflow file not found: {remote_path}")

    remote_content = remote_result.value()

    # Compare (normalize line endings)
    local_normalized = local_content.replace("\r\n", "\n").strip()
    remote_normalized = str(remote_content).replace("\r\n", "\n").strip()

    if local_normalized == remote_normalized:
        return Ok(True)
    else:
        return Err(
            f"Workflow out of sync: local '{local_path}' differs from remote '{remote_path}'. "
            "Commit and push your changes, or use --force to skip validation."
        )


def flow_to_workflow_name(flow_name: str) -> str:
    """
    Convert a flow name to the corresponding workflow filename.

    Args:
        flow_name: Name of the flow (e.g., "ci")

    Returns:
        Workflow filename (e.g., "recompose_flow_ci.yml")

    """
    return f"recompose_flow_{flow_name}.yml"


def workflow_to_flow_name(workflow_name: str) -> str | None:
    """
    Extract flow name from a workflow filename.

    Args:
        workflow_name: Workflow filename (e.g., "recompose_flow_ci.yml")

    Returns:
        Flow name (e.g., "ci") or None if not a recompose workflow

    """
    if workflow_name.startswith("recompose_flow_") and workflow_name.endswith(".yml"):
        return workflow_name[len("recompose_flow_") : -len(".yml")]
    return None
