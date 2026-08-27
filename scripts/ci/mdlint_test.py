from __future__ import annotations

import unittest

from mdlint import Error, check_upcoming_asset_links  # type: ignore[import-not-found]

UPCOMING_PATH = "docs/content/changelog/upcoming/feature.md"
GITHUB_ATTACHMENT = "https://github.com/user-attachments/assets/01234567-89ab-cdef-0123-456789abcdef"


class UpcomingAssetLinksTest(unittest.TestCase):
    def check(self, content: str, path: str = UPCOMING_PATH) -> list[Error]:
        errors: list[Error] = []
        check_upcoming_asset_links(path, content, errors)
        return errors

    def assert_rejected(self, content: str) -> None:
        errors = self.check(content)

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0].code, "E006")
        self.assertIn("upcoming/feature.md:1", errors[0].render(UPCOMING_PATH, content))

    def test_rejects_standalone_urls(self) -> None:
        lines = [
            "https://static.rerun.io/demo.mp4",
            "> https://static.rerun.io/demo.mp4",
            "- https://static.rerun.io/demo.mp4",
        ]
        for line in lines:
            with self.subTest(line=line):
                self.assert_rejected(line)

    def test_rejects_standalone_github_attachment(self) -> None:
        self.assert_rejected(f"{GITHUB_ATTACHMENT}\n")

    def test_rejects_embedded_github_attachment(self) -> None:
        self.assert_rejected(f"![Demo]({GITHUB_ATTACHMENT})\n")

    def test_rejects_github_asset_url_forms(self) -> None:
        urls = [
            "https://github.com/rerun-io/rerun/assets/49431240/1c75b816-7e3e-4882-9ee6-ba124c00d73c",
            "https://user-images.githubusercontent.com/123/456/demo.mp4",
            "https://private-user-images.githubusercontent.com/123/456/demo.mp4?token=secret",
        ]
        for url in urls:
            with self.subTest(url=url):
                self.assert_rejected(f'<video src="{url}"></video>\n')

    def test_accepts_named_markdown_link(self) -> None:
        self.assertEqual(self.check("[Documentation](https://rerun.io/docs)\n"), [])

    def test_accepts_embedded_permanent_asset(self) -> None:
        content = '<video controls src="https://static.rerun.io/demo.mp4"></video>\n'
        self.assertEqual(self.check(content), [])

    def test_ignores_code_fences_and_html_comments(self) -> None:
        content = f"""```md
{GITHUB_ATTACHMENT}
```

<!--
{GITHUB_ATTACHMENT}
-->
"""
        self.assertEqual(self.check(content), [])

    def test_checks_visible_content_next_to_html_comment(self) -> None:
        self.assert_rejected(f"{GITHUB_ATTACHMENT} <!-- explanation -->\n")

    def test_only_checks_upcoming_changelog_entries(self) -> None:
        path = "docs/content/changelog/changeset-0-35.md"
        self.assertEqual(self.check(f"{GITHUB_ATTACHMENT}\n", path), [])


if __name__ == "__main__":
    unittest.main()
