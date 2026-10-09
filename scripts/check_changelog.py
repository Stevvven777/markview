#!/usr/bin/env python3
"""Check release headings and the dense, single-line Unreleased entries."""

import argparse
import re
import sys
from datetime import date
from pathlib import Path

CHANGELOG = Path(__file__).resolve().parents[1] / "CHANGELOG.md"
CATEGORIES = {
    "Added",
    "Changed",
    "Deprecated",
    "Removed",
    "Fixed",
    "Security",
    "Documentation",
}
NUMBER = r"(?:0|[1-9][0-9]*)"
IDENTIFIER = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
VERSION = (
    rf"{NUMBER}\.{NUMBER}\.{NUMBER}"
    rf"(?:-{IDENTIFIER}(?:\.{IDENTIFIER})*)?"
    r"(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?"
)
RELEASE = re.compile(rf"## ({VERSION}) - ([0-9]{{4}}-[0-9]{{2}}-[0-9]{{2}})")


def check(text):
    errors = []
    headings = []
    versions = set()
    lines = text.splitlines()
    for number, line in enumerate(lines, 1):
        if not re.match(r"^##(?:\s|$)", line):
            continue
        headings.append((number, line))
        if line == "## Unreleased":
            continue
        release = RELEASE.fullmatch(line)
        if not release:
            errors.append(
                (
                    number,
                    "expected '## VERSION - YYYY-MM-DD' with an unbracketed SemVer version",
                )
            )
            continue
        version, released = release.groups()
        if version in versions:
            errors.append((number, f"duplicate release version: {version}"))
        versions.add(version)
        try:
            date.fromisoformat(released)
        except ValueError:
            errors.append((number, f"invalid release date: {released}"))

    unreleased = [number for number, line in headings if line == "## Unreleased"]
    if len(unreleased) != 1:
        errors.append(
            (
                unreleased[-1] if unreleased else 1,
                "expected exactly one '## Unreleased' section",
            )
        )
    if headings and headings[0][1] != "## Unreleased":
        errors.append((headings[0][0], "Unreleased must be the first release section"))

    for start in unreleased:
        end = next((number for number, _ in headings if number > start), len(lines) + 1)
        categories = set()
        category = None
        previous_entry = None
        for number in range(start + 1, end):
            line = lines[number - 1]
            if line != line.rstrip():
                errors.append((number, "trailing whitespace is not allowed"))
            if not line.strip():
                continue
            if line.startswith("### "):
                category = line.removeprefix("### ")
                if category not in CATEGORIES:
                    errors.append((number, f"unknown category: {category}"))
                if category in categories:
                    errors.append((number, f"duplicate category: {category}"))
                categories.add(category)
                previous_entry = None
            elif line.startswith("- ") and line[2:].strip():
                if category is None:
                    errors.append((number, "entry must be under a category heading"))
                if previous_entry is not None and number != previous_entry + 1:
                    errors.append(
                        (
                            number,
                            "remove blank lines between entries to keep the list dense",
                        )
                    )
                if line.endswith("\\"):
                    errors.append(
                        (number, "Markdown line breaks are not allowed in entries")
                    )
                previous_entry = number
            else:
                errors.append(
                    (
                        number,
                        "expected '### CATEGORY' or a single-line '- ' entry; do not wrap entries",
                    )
                )
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("path", nargs="?", type=Path, default=CHANGELOG)
    path = parser.parse_args().path
    try:
        errors = check(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError) as error:
        print(f"{path}: {error}", file=sys.stderr)
        return 1
    for number, message in errors:
        print(f"{path}:{number}: {message}", file=sys.stderr)
    if errors:
        return 1
    print(f"{path}: changelog checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
