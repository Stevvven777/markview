# Changelog conventions

Add entries to [CHANGELOG.md](../../CHANGELOG.md) under `Unreleased` as you make
changes. CI checks the conventions with
[check_changelog.py](../../scripts/check_changelog.py).

## Entry format

`Unreleased` must appear exactly once and be the first release section. Group
entries under unique `###` category headings: `Added`, `Changed`, `Deprecated`,
`Removed`, `Fixed`, `Security`, or `Documentation`.

Keep each entry on one `- ` line. Do not wrap entries, put blank lines between
entries in the same category, or use Markdown line breaks or trailing whitespace.
Historical release bodies keep their existing format.

## Release headings

Use `## VERSION - YYYY-MM-DD`, with an unbracketed SemVer version and a valid
calendar date. Versions must be unique; prerelease versions and build metadata
are supported. These heading checks apply to every release.

## Run the check

From the repository root:

```sh
python3 scripts/check_changelog.py
```

Pass a file path to check another changelog. Failures print the file, line number
and violated rule, and exit with status 1.
