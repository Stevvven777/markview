#!/usr/bin/env python3
"""Render the four source-backed document previews for each README."""

import argparse
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, ImageDraw
from readme_fonts import font

ROOT = Path(__file__).resolve().parents[1]
SOURCES = ROOT / "docs/screenshots/source/showcase"
OUTPUT = ROOT / "docs/screenshots/readme"
KINDS = ("prose", "technical", "math", "web")


def extract_web():
    """Run the production extractor and text validation against saved HTML."""
    source = (ROOT / "src/web_page.rs").read_text()
    source = source[source.index("fn extract(") : source.index("#[cfg(test)]")]
    paste = (ROOT / "src/paste.rs").read_text()
    paste = paste[
        paste.index("pub(crate) fn looks_like_markdown") : paste.index(
            "/// Finds an ATX heading"
        )
    ]
    limit = next(
        line
        for line in (ROOT / "src/file.rs").read_text().splitlines()
        if line.startswith("pub const MAX_FILE_BYTES")
    )
    with tempfile.TemporaryDirectory() as directory:
        project = Path(directory)
        (project / "src").mkdir()
        (project / "Cargo.toml").write_text("""[package]
name = "markview-readme-extractor"
version = "0.1.0"
edition = "2024"
[dependencies]
anyhow = "1"
dom_smoothie = "0.18.2"
dom_query = "0.28"
comrak = { version = "0.56", default-features = false }
""")
        main = "use anyhow::{Result, bail};\nuse dom_smoothie::{Config, Readability, TextMode};\n"
        main += f"mod file {{ {limit} }}\nmod paste {{ {paste} }}\n" + source
        main += """
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let html = std::fs::read_to_string(&args[1])?;
    print!("{}", extract(&html, &args[2])?);
    Ok(())
}
"""
        (project / "src/main.rs").write_text(main)
        for language in ("en", "zh"):
            url = (
                "https://zh.wikipedia.org/wiki/%E9%9F%8B%E4%BC%AF%E7%9A%84%E9%A6%96%E6%AC%A1%E6%B7%B1%E7%A9%BA"
                if language == "zh"
                else "https://en.wikipedia.org/wiki/Webb%27s_First_Deep_Field"
            )
            result = subprocess.run(
                [
                    "cargo",
                    "run",
                    "--quiet",
                    "--offline",
                    "--manifest-path",
                    str(project / "Cargo.toml"),
                    "--target-dir",
                    str(ROOT / "target/readme-web-extractor"),
                    "--",
                    str(SOURCES / language / "web.html"),
                    url,
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            (SOURCES / language / "web-extracted.md").write_text(result.stdout)


def render(binary, output):
    output.mkdir(parents=True, exist_ok=True)
    for language in ("en", "zh"):
        for kind in KINDS:
            target = output / f"{language}-showcase-{kind}.png"
            subprocess.run(
                [
                    str(binary),
                    "render",
                    str(SOURCES / language / f"{kind}.md"),
                    "--output",
                    str(target),
                    "--width",
                    "1440",
                    "--height",
                    "1600",
                    "--column",
                    "650",
                    "--font-size",
                    "14" if kind == "technical" else "18",
                    "--scale",
                    "2",
                    "--light",
                    "--offline",
                ],
                check=True,
            )
            compose_credit(target, language, kind)


def compose_credit(target, language, kind):
    with Image.open(target) as rendered:
        page = Image.new("RGB", (1440, 1664), "#F9FAFC")
        page.paste(rendered.crop((0, 0, 1440, 1600)), (0, 0))
        draw = ImageDraw.Draw(page)
        if kind == "math":
            credit = "Download for free at https://openstax.org/details/books/calculus-volume-3."
        elif language == "zh":
            credit = {
                "prose": "Aaron Swartz ·《游击队开放访问宣言》· 2008 年 7 月",
                "technical": "《Rust 程序设计语言》· Rust 中文社区译本",
                "web": "维基百科 ·《韦伯的首次深空》· 原文转写节选",
            }[kind]
        else:
            credit = {
                "prose": "Aaron Swartz · Guerilla Open Access Manifesto · July 2008",
                "technical": "The Rust Programming Language · Data Types",
                "web": "Wikipedia · Webb’s First Deep Field · Extracted article",
            }[kind]
        draw.text((70, 1616), credit, font=font(21, language), fill="#687587")
        page.save(target, optimize=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/markview")
    parser.add_argument("--output-directory", type=Path, default=OUTPUT)
    parser.add_argument(
        "--extract-web",
        action="store_true",
        help="refresh full web extraction from saved HTML using Cargo",
    )
    args = parser.parse_args()
    if args.extract_web:
        extract_web()
    render(args.binary, args.output_directory)
