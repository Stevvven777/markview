"""Exercise the changelog checker through its command-line interface."""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).with_name("check_changelog.py")
VALID = """# Changelog

## Unreleased

### Added

- Add a feature.
- Add another feature.

### Fixed

- Fix a bug.

## 0.2.0-beta.1+build.2 - 2026-10-06

Historical prose and wrapped entries keep their format.

### Added

- An old entry that
  spans two lines.

- Another old entry.
""".replace("  spans two lines.", "  spans two lines.  ")


class ChangelogTest(unittest.TestCase):
    def run_checker(self, text):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "CHANGELOG.md"
            path.write_text(text, encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(path)],
                capture_output=True,
                text=True,
                check=False,
                cwd=directory,
            )
        return result

    def test_valid_changelog_and_historical_format(self):
        for text in (
            VALID,
            VALID.replace("- Add a feature.\n", "- 支持中文记录。\n"),
            "# Changelog\n\n## Unreleased\n",
        ):
            with self.subTest(text=text):
                result = self.run_checker(text)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("checks passed", result.stdout)
                self.assertEqual(result.stderr, "")

    def test_unreleased_format_errors_report_the_line(self):
        cases = [
            ("- Add a feature.\n  Wrapped prose.", "do not wrap entries", 8),
            ("- Add a feature.\n\n", "keep the list dense", 10),
            ("- Add a feature.  ", "trailing whitespace", 7),
            ("- Add a feature.\\", "line breaks are not allowed", 7),
            ("- ", "single-line '- ' entry", 7),
            ("* Add a feature.", "single-line '- ' entry", 7),
            ("#### Nested category", "do not wrap entries", 7),
        ]
        for replacement, message, number in cases:
            with self.subTest(replacement=replacement):
                result = self.run_checker(
                    VALID.replace("- Add a feature.", replacement)
                )
                self.assertEqual(result.returncode, 1)
                self.assertIn(f"CHANGELOG.md:{number}:", result.stderr)
                self.assertIn(message, result.stderr)
                self.assertEqual(result.stdout, "")

    def test_sections_and_release_headers(self):
        cases = [
            (VALID.replace("## Unreleased", "## 0.3.0 - 2026-10-09"), "exactly one"),
            (VALID + "\n## Unreleased\n", "exactly one"),
            ("## 0.3.0 - 2026-10-09\n\n" + VALID, "must be the first"),
            (VALID.replace("### Fixed", "### Added"), "duplicate category"),
            (VALID.replace("### Added", "### Features", 1), "unknown category"),
            (VALID.replace("### Added\n\n", "", 1), "under a category"),
            (VALID.replace("2026-10-06", "2026-02-30"), "invalid release date"),
            (VALID + "\n## 0.2.0-beta.1+build.2 - 2026-10-07\n", "duplicate release"),
        ]
        for version in ("[0.2.0]", "v0.2.0", "0.2", "00.2.0", "0.2.0-beta.01"):
            cases.append(
                (VALID.replace("0.2.0-beta.1+build.2", version), "unbracketed SemVer")
            )
        for text, message in cases:
            with self.subTest(message=message, text=text):
                result = self.run_checker(text)
                self.assertEqual(result.returncode, 1)
                self.assertIn(message, result.stderr)

    def test_repository_changelog_from_another_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(SCRIPT)],
                capture_output=True,
                text=True,
                check=False,
                cwd=directory,
            )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_file(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "missing.md"
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(path)],
                capture_output=True,
                text=True,
                check=False,
            )
        self.assertEqual(result.returncode, 1)
        self.assertIn(str(path), result.stderr)
        self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
