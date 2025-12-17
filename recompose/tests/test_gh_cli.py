"""Tests for GitHub CLI integration."""

from pathlib import Path
from unittest.mock import MagicMock, patch

from recompose import gh_cli


class TestFlowWorkflowMapping:
    """Tests for flow name <-> workflow name conversion."""

    def test_flow_to_workflow_name(self) -> None:
        """flow_to_workflow_name creates correct filename."""
        assert gh_cli.flow_to_workflow_name("ci") == "recompose_flow_ci.yml"
        assert gh_cli.flow_to_workflow_name("build_test") == "recompose_flow_build_test.yml"

    def test_workflow_to_flow_name(self) -> None:
        """workflow_to_flow_name extracts flow name correctly."""
        assert gh_cli.workflow_to_flow_name("recompose_flow_ci.yml") == "ci"
        assert gh_cli.workflow_to_flow_name("recompose_flow_build_test.yml") == "build_test"

    def test_workflow_to_flow_name_non_recompose(self) -> None:
        """workflow_to_flow_name returns None for non-recompose workflows."""
        assert gh_cli.workflow_to_flow_name("ci.yml") is None
        assert gh_cli.workflow_to_flow_name("build.yaml") is None


class TestWorkflowRun:
    """Tests for WorkflowRun dataclass."""

    def test_display_status_completed_success(self) -> None:
        """display_status shows conclusion for completed runs."""
        run = gh_cli.WorkflowRun(
            id=123,
            name="CI",
            status="completed",
            conclusion="success",
            head_branch="main",
            head_sha="abc123",
            url="https://gh_cli.com/example/repo/actions/runs/123",
            created_at="2025-01-01T00:00:00Z",
            updated_at="2025-01-01T00:00:00Z",
        )
        assert run.display_status == "success"

    def test_display_status_completed_failure(self) -> None:
        """display_status shows conclusion for failed runs."""
        run = gh_cli.WorkflowRun(
            id=123,
            name="CI",
            status="completed",
            conclusion="failure",
            head_branch="main",
            head_sha="abc123",
            url="https://gh_cli.com/example/repo/actions/runs/123",
            created_at="2025-01-01T00:00:00Z",
            updated_at="2025-01-01T00:00:00Z",
        )
        assert run.display_status == "failure"

    def test_display_status_in_progress(self) -> None:
        """display_status shows status for non-completed runs."""
        run = gh_cli.WorkflowRun(
            id=123,
            name="CI",
            status="in_progress",
            conclusion=None,
            head_branch="main",
            head_sha="abc123",
            url="https://gh_cli.com/example/repo/actions/runs/123",
            created_at="2025-01-01T00:00:00Z",
            updated_at="2025-01-01T00:00:00Z",
        )
        assert run.display_status == "in_progress"


class TestGitHelpers:
    """Tests for git helper functions."""

    def test_find_git_root_in_repo(self) -> None:
        """find_git_root returns path in a git repo."""
        # We're in a git repo, so this should work
        result = gh_cli.find_git_root()
        assert result is not None
        assert (result / ".git").exists()

    def test_get_current_branch(self) -> None:
        """get_current_branch returns current branch name."""
        result = gh_cli.get_current_branch()
        assert result.ok
        # We know we're on a branch
        assert len(result.value()) > 0


class TestGhCliWrapper:
    """Tests for _run_gh function (mocked)."""

    @patch("recompose.gh_cli.subprocess.run")
    def test_run_gh_success(self, mock_run: MagicMock) -> None:
        """_run_gh returns output on success."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout="success output\n",
            stderr="",
        )

        result = gh_cli._run_gh("test", "command")
        assert result.ok
        assert result.value() == "success output"

    @patch("recompose.gh_cli.subprocess.run")
    def test_run_gh_failure(self, mock_run: MagicMock) -> None:
        """_run_gh returns error on failure."""
        mock_run.return_value = MagicMock(
            returncode=1,
            stdout="",
            stderr="error message",
        )

        result = gh_cli._run_gh("test", "command")
        assert result.failed
        assert "error message" in str(result.error)

    @patch("recompose.gh_cli.subprocess.run")
    def test_run_gh_not_found(self, mock_run: MagicMock) -> None:
        """_run_gh returns helpful error when gh not found."""
        mock_run.side_effect = FileNotFoundError()

        result = gh_cli._run_gh("test", "command")
        assert result.failed
        assert "not found" in str(result.error).lower()

    @patch("recompose.gh_cli.subprocess.run")
    def test_run_gh_json_parsing(self, mock_run: MagicMock) -> None:
        """_run_gh can parse JSON output."""
        mock_run.return_value = MagicMock(
            returncode=0,
            stdout='{"key": "value"}',
            stderr="",
        )

        result = gh_cli._run_gh("test", capture_json=True)
        assert result.ok
        assert result.value() == {"key": "value"}


class TestValidateWorkflowSync:
    """Tests for workflow sync validation."""

    def test_validate_sync_local_missing(self, tmp_path: Path) -> None:
        """validate_workflow_sync fails if local file missing."""
        result = gh_cli.validate_workflow_sync(
            tmp_path / "nonexistent.yml",
            ".github/workflows/nonexistent.yml",
        )
        assert result.failed
        assert "not found" in str(result.error).lower()

    @patch("recompose.gh_cli.get_workflow_file_content")
    def test_validate_sync_remote_missing(self, mock_get: MagicMock, tmp_path: Path) -> None:
        """validate_workflow_sync fails if remote file missing."""
        # Create local file
        local_file = tmp_path / "test.yml"
        local_file.write_text("name: Test\n")

        # Mock remote not found
        mock_get.return_value = gh_cli.Err("Not found")

        result = gh_cli.validate_workflow_sync(local_file, ".github/workflows/test.yml")
        assert result.failed
        assert "not found" in str(result.error).lower()

    @patch("recompose.gh_cli.get_workflow_file_content")
    def test_validate_sync_files_match(self, mock_get: MagicMock, tmp_path: Path) -> None:
        """validate_workflow_sync succeeds when files match."""
        content = "name: Test\non: push\n"

        # Create local file
        local_file = tmp_path / "test.yml"
        local_file.write_text(content)

        # Mock remote with same content
        mock_get.return_value = gh_cli.Ok(content)

        result = gh_cli.validate_workflow_sync(local_file, ".github/workflows/test.yml")
        assert result.ok
        assert result.value() is True

    @patch("recompose.gh_cli.get_workflow_file_content")
    def test_validate_sync_files_differ(self, mock_get: MagicMock, tmp_path: Path) -> None:
        """validate_workflow_sync fails when files differ."""
        # Create local file
        local_file = tmp_path / "test.yml"
        local_file.write_text("name: Test\non: push\n")

        # Mock remote with different content
        mock_get.return_value = gh_cli.Ok("name: Test\non: pull_request\n")

        result = gh_cli.validate_workflow_sync(local_file, ".github/workflows/test.yml")
        assert result.failed
        assert "out of sync" in str(result.error).lower()
