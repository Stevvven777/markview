#!/usr/bin/env python3
"""Require explicit visual fixtures for every MVSS drawing property."""

import re
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def fields(source, name):
    body = source.split(f"pub struct {name} {{", 1)[1].split("\n}", 1)[0]
    return set(re.findall(r"pub (\w+):", body))


def check():
    source = (ROOT / "crates/markview-core/src/style/types.rs").read_text()
    fixtures = list((ROOT / "crates/markview-render/tests/fixtures").glob("*.mvss.toml"))
    fixtures += list((ROOT / "tests/fixtures").rglob("*.mvss.toml"))
    covered = {name: set() for name in (
        "Rule", "Font", "PageStyle", "PageEdgeStyle", "MermaidStyle", "SvgStyle",
    )}
    media = set()
    for path in fixtures:
        sheet = tomllib.loads(path.read_text())
        for rule in sheet.get("rule", []):
            covered["Rule"].update(set(rule) - {"when", "media"})
            media.update(rule.get("media", []))
            for font in rule.get("font", []):
                covered["Font"].update(font)
        page = sheet.get("page", {})
        covered["PageStyle"].update(page)
        for edge in ("header", "footer"):
            covered["PageEdgeStyle"].update(page.get(edge, {}))
        covered["MermaidStyle"].update(sheet.get("mermaid", {}))
        covered["SvgStyle"].update(sheet.get("svg", {}))
    failures = []
    for name, configured in covered.items():
        missing = fields(source, name) - configured
        if missing:
            failures.append(f"{name}: missing visual fixtures for {sorted(missing)}")
    if failures:
        raise SystemExit("\n".join(failures))
    media_source = (ROOT / "crates/markview-core/src/style/media.rs").read_text()
    media_body = media_source.split("pub enum Media {", 1)[1].split("\n}", 1)[0]
    missing_media = {name.lower() for name in re.findall(r"\t(\w+),", media_body)} - media
    if missing_media:
        raise SystemExit(f"Media filters without visual fixtures: {sorted(missing_media)}")
    print(f"All {sum(len(fields(source, name)) for name in covered)} MVSS drawing properties have explicit fixtures")


if __name__ == "__main__":
    check()
