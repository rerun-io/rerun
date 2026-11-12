from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import zombie_todos

REPOSITORY = "rerun-io/reality"


def todo(reference: str) -> str:
    return "TODO" + f"({reference})"


class ClosingIssuesTests(unittest.TestCase):
    def test_keywords(self) -> None:
        for keyword in (
            "close",
            "closes",
            "closed",
            "fix",
            "fixes",
            "fixed",
            "resolve",
            "resolves",
            "resolved",
            "completes",
        ):
            with self.subTest(keyword=keyword):
                self.assertEqual(
                    zombie_todos.closing_issues(f"{keyword.upper()}: #42", REPOSITORY), {f"{REPOSITORY}#42"}
                )

    def test_links_and_lists(self) -> None:
        text = (
            "Fixes #1, Other/Repo#2 and https://github.com/Other/Repo/issues/3.\n"
            "Closes [RR-4](https://linear.app/rerun/issue/RR-4/title), RR-5\n"
            "Resolves <https://github.com/rerun-io/rerun/issues/6>\n"
            "Fixes https://linear.app/rerun/issue/rr-7/title, RR-8"
        )
        self.assertEqual(
            zombie_todos.closing_issues(text, REPOSITORY),
            {f"{REPOSITORY}#1", "other/repo#2", "other/repo#3", "RR-4", "RR-5", "rerun-io/rerun#6", "RR-7", "RR-8"},
        )

    def test_decorated_issue_links(self) -> None:
        for url, issue in (
            ("https://linear.app/rerun/issue/RR-2/title?focus=true", "RR-2"),
            ("https://github.com/rerun-io/rerun/issues/42#issuecomment-123", "rerun-io/rerun#42"),
            ("https://github.com/rerun-io/rerun/issues/42?view=compact#details", "rerun-io/rerun#42"),
        ):
            for reference in (url, f"<{url}>", f"[issue]({url})"):
                with self.subTest(reference=reference):
                    self.assertEqual(
                        zombie_todos.closing_issues(f"Fixes {reference}, RR-9", REPOSITORY), {issue, "RR-9"}
                    )

    def test_non_closing_references(self) -> None:
        text = (
            "Related to #1. Fixes a bug mentioned in #2. Encloses #3.\n"
            "<!-- Closes #4 -->\n"
            "Fixes #5. Related to #6. Fixes #12abc."
        )
        self.assertEqual(zombie_todos.closing_issues(text, REPOSITORY), {f"{REPOSITORY}#5"})

    def test_multiple_todos_and_issue_numbers(self) -> None:
        line = " ".join(todo(ref) for ref in ("#1, #2, #123", "RR-4", "Other/Repo#5", "alice"))
        self.assertEqual(
            zombie_todos.todo_issues(line, REPOSITORY),
            {f"{REPOSITORY}#1", f"{REPOSITORY}#2", f"{REPOSITORY}#123", "RR-4", "other/repo#5"},
        )


class CheckoutTests(unittest.TestCase):
    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        subprocess.run(["git", "init", "-q", directory.name], check=True)
        (self.root / "rerun").mkdir()
        (self.root / "rerun/code.rs").write_text(f"{todo('#42')}\n{todo('RR-2')}\n", encoding="utf-8")
        (self.root / "code.rs").write_text(todo("#42"), encoding="utf-8")
        (self.root / "binary").write_bytes(b"\0" + todo("RR-2").encode())
        (self.root / "link").symlink_to(self.root / "code.rs")
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        (self.root / "untracked.rs").write_text(todo("RR-2"), encoding="utf-8")

    def run_pr(self, body: str | None) -> subprocess.CompletedProcess[str]:
        pr = self.root / "pr.json"
        pr.write_text(json.dumps({"base": {"repo": {"full_name": REPOSITORY}}, "body": body}), encoding="utf-8")
        return subprocess.run(
            [sys.executable, "-S", str(Path(zombie_todos.__file__).resolve()), "--pr", str(pr)],
            cwd=self.root,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_scans_tracked_text_and_resolves_subtree_references(self) -> None:
        self.assertEqual(
            zombie_todos.find_zombies(self.root, {"rerun-io/rerun#42", f"{REPOSITORY}#42", "RR-2"}, REPOSITORY),
            [
                ("code.rs", 1, f"{REPOSITORY}#42"),
                ("rerun/code.rs", 1, "rerun-io/rerun#42"),
                ("rerun/code.rs", 2, "RR-2"),
            ],
        )

    def test_fixture_paths_are_excluded(self) -> None:
        for prefix in ("", "rerun/"):
            for excluded in zombie_todos.EXCLUDED_PATHS:
                path = self.root / prefix / excluded
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(todo("RR-1234"), encoding="utf-8")
        subprocess.run(["git", "add", "."], cwd=self.root, check=True)
        self.assertEqual(zombie_todos.find_zombies(self.root, {"RR-1234"}, REPOSITORY), [])
        self.assertTrue(zombie_todos.excluded_path("./scripts/zombie_todos.py"))
        self.assertFalse(zombie_todos.excluded_path("other/scripts/zombie_todos.py"))

    def test_example_opt_out_keeps_surrounding_notes(self) -> None:
        path = self.root / "examples.md"
        path.write_text(
            f"{todo('RR-2')} <!-- NOLINT -->\n"
            "<!-- NOLINT_START -->\n"
            f"{todo('RR-2')}\n"
            "<!-- NOLINT_END -->\n"
            f"{todo('RR-2')}\n",
            encoding="utf-8",
        )
        self.assertEqual(list(zombie_todos.file_todos(path, REPOSITORY)), [(5, "RR-2", todo("RR-2"))])
        with (
            patch.object(zombie_todos, "get_linear_line_output", return_value=(True, "remaining")) as report,
        ):
            self.assertEqual(zombie_todos.check_file(str(path)), (False, "remaining"))
            report.assert_called_once_with("RR-2", str(path), 4, todo("RR-2"))

    def test_pr_fails_with_file_and_line(self) -> None:
        result = self.run_pr("Fixes RR-2")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("rerun/code.rs:2: This PR closes RR-2", result.stdout)

    def test_pr_passes_after_todo_removal(self) -> None:
        (self.root / "rerun/code.rs").write_text(todo("#42"), encoding="utf-8")
        result = self.run_pr("Fixes RR-2")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("No zombie " + "TODO" + "s found.", result.stdout)

    def test_pr_without_closing_references(self) -> None:
        for body in (None, "", "Related to #42"):
            with self.subTest(body=body):
                result = self.run_pr(body)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, "No closing issue references found.\n")


class DiscoveryTests(unittest.TestCase):
    def test_closed_issues_fail_and_open_issues_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "code.rs"
            path.write_text(f"{todo('#1, #2')}\n{todo('RR-3')}\n", encoding="utf-8")
            for closed in (False, True):
                with (
                    self.subTest(closed=closed),
                    patch.object(zombie_todos, "args", argparse.Namespace(markdown=False), create=True),
                    patch.object(zombie_todos, "get_line_blame_info", return_value=None),
                    patch.object(zombie_todos, "check_issue_closed", return_value=(closed, "author")) as github,
                    patch.object(zombie_todos, "check_linear_issue_closed", return_value=(closed, "author")) as linear,
                ):
                    ok, output = zombie_todos.check_file(str(path))
                    self.assertEqual(ok, not closed)
                    self.assertEqual(bool(output), closed)
                    self.assertEqual(github.call_count, 2)
                    linear.assert_called_once_with("RR-3")


if __name__ == "__main__":
    unittest.main()
