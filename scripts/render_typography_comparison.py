#!/usr/bin/env python3
"""Render the WebView-versus-Markview typography figure used by the READMEs.

Both panels set the same plain text to the same measure, type size and font, and
the left panel uses conventional left alignment. It is a headless Chromium
with a conventional Markdown-preview stylesheet; the right one is Markview's
default light stylesheet. The figure is written to docs/screenshots/.

Requirements: chromium, Pillow and numpy, plus a release Markview build
that can reach a GPU. The result depends on the host's fonts and browser build,
so the figure published in the README is the one this host produces.

Usage: scripts/render_typography_comparison.py [--source text.txt] [--column 400] [--scale 2]
"""

import argparse
import html
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile

import numpy as np
from PIL import Image, ImageDraw
from readme_fonts import font

ROOT = pathlib.Path(__file__).resolve().parents[1]
SOURCE = ROOT / "docs/screenshots/source/typography.txt"
OUTPUT = ROOT / "docs/screenshots/en-comparison.png"

# The render viewport is physical pixels and the reading column is inset by
# 16 logical pixels on each side (`src/app/launch.rs`).
INSET_X = 16
INSET_Y = 24
FOOT = 48
FONT_SIZE = 18
MARGIN = 36
GAP = 48
HEADER = 164
FOOTER = 70
LABEL_FONT = "/usr/share/fonts/TTF/DejaVuSans.ttf"
INK = (40, 51, 67)
RULE = (226, 226, 222)

# A conventional Markdown preview: the same font, size, measure and paragraph
# spacing as the light stylesheet, a ragged right edge as every browser-based
# preview ships it, and `hyphens: auto` so the engine is asked for hyphenation
# rather than silently denied it.
PAGE = """<!doctype html><meta charset="utf-8"><style>
@font-face {{ font-family: Comparison; src: url("{font_uri}"); }}
html, body {{ margin: 0; padding: 0; background: #F9FAFC }}
body {{
  width: {width}px;
  padding: {top}px {left}px;
  box-sizing: border-box;
  font: {size}px/1.65 Comparison, serif;
  color: #283343;
  text-align: left;
  hyphens: auto;
  -webkit-hyphens: auto;
  hyphenate-limit-chars: 6 2 2;
}}
p {{ margin: 0 0 0.8em }}
</style><body>
{body}
"""


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, text=True, **kwargs)


def markview_panel(binary, work, width, column, scale):
    """Render the reading view and return the image plus its content height."""
    target = work / "markview.png"
    fonts = work / "fonts"
    fonts.mkdir()
    font_path = run("fc-match", "-f", "%{file}", "Noto Serif").stdout
    shutil.copyfile(font_path, fonts / pathlib.Path(font_path).name)
    result = subprocess.run(
        [
            binary,
            "render",
            str(work / "document.md"),
            "--output",
            str(target),
            "--width",
            str(width * scale),
            "--height",
            "6000",
            "--column",
            str(column),
            "--font-size",
            str(FONT_SIZE),
            "--scale",
            str(scale),
            "--light",
            "--offline",
            "--fonts",
            str(fonts),
            "--ignore-system-fonts",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    for token in result.stdout.split() + result.stderr.split():
        if token.endswith("px"):
            return Image.open(target).convert("RGB"), int(float(token[:-2]))
    sys.exit("markview render did not report the document height")


def webview_panel(work, width, height, scale):
    """Render the same paragraphs the way a browser-based preview would."""
    page = work / "page.html"
    page.write_text(
        PAGE.format(
            width=width,
            top=INSET_Y,
            left=INSET_X,
            size=FONT_SIZE,
            font_uri=(
                work
                / "fonts"
                / pathlib.Path(
                    run("fc-match", "-f", "%{file}", "Noto Serif").stdout
                ).name
            ).as_uri(),
            body=(work / "document.html").read_text(),
        )
    )
    target = work / "webview.png"
    run(
        "chromium",
        "--headless",
        "--no-sandbox",
        "--disable-gpu",
        f"--user-data-dir={work}/profile",
        f"--force-device-scale-factor={scale}",
        f"--window-size={width},{height}",
        "--hide-scrollbars",
        "--virtual-time-budget=1000",
        "--default-background-color=00000000",
        f"--screenshot={target}",
        f"file://{page}",
    )
    return Image.open(target).convert("RGB")


def ink_height(image, scale):
    """Height of the last inked row, in logical pixels."""
    dark = np.array(image.convert("L")) < 128
    rows = np.flatnonzero(dark.any(axis=1))
    return 0 if rows.size == 0 else int(rows[-1] // scale) + 1


def text_lines(image):
    """Number of bands of inked rows, which is the number of set lines."""
    dark = np.array(image.convert("L")) < 128
    rows = dark.any(axis=1)
    edges = np.diff(np.concatenate(([False], rows, [False])).astype(np.int8))
    return int((edges == 1).sum())


def label(draw, font, x, y, text):
    draw.text((x, y), text, font=font, fill=INK)


def prepare_text(source, work):
    """Give both engines the same literal paragraphs, with no Markdown syntax."""
    paragraphs = [
        " ".join(part.split())
        for part in re.split(r"\n\s*\n", source.read_text(encoding="utf-8").strip())
    ]
    markdown = [
        re.sub(r"([\\`*_{}\[\]()#+.!<>~|=-])", r"\\\1", paragraph)
        for paragraph in paragraphs
    ]
    (work / "document.md").write_text("\n\n".join(markdown), encoding="utf-8")
    (work / "document.html").write_text(
        "\n".join(f"<p>{html.escape(paragraph)}</p>" for paragraph in paragraphs),
        encoding="utf-8",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--column", type=int, default=400, help="reading column in logical pixels"
    )
    parser.add_argument("--scale", type=int, default=2, help="device pixels")
    parser.add_argument("--binary", default=str(ROOT / "target/release/markview"))
    parser.add_argument("--out", default=str(OUTPUT))
    parser.add_argument(
        "--source",
        type=pathlib.Path,
        default=SOURCE,
        help="UTF-8 plain text; blank lines separate paragraphs",
    )
    parser.add_argument("--language", choices=("en", "zh"), default="en")
    args = parser.parse_args()
    if not shutil.which("chromium"):
        sys.exit("this script needs chromium on PATH")

    width = args.column + 2 * INSET_X
    with tempfile.TemporaryDirectory() as directory:
        work = pathlib.Path(directory)
        prepare_text(args.source, work)
        markview, content = markview_panel(
            args.binary, work, width, args.column, args.scale
        )
        # The viewport is generous: a browser that cannot hyphenate needs more
        # lines for the same text, and the crop below picks the taller panel.
        webview = webview_panel(work, width, content * 2, args.scale)
        height = max(content, ink_height(webview, args.scale)) + FOOT
        pixels = height * args.scale
        markview = markview.crop((0, 0, width * args.scale, pixels))
        webview = webview.crop((0, 0, width * args.scale, pixels))
        lines = text_lines(webview), text_lines(markview)

        panel = width * args.scale
        margin, gap, band = (value * args.scale for value in (MARGIN, GAP, HEADER))
        figure = Image.new(
            "RGB",
            (panel * 2 + gap + margin * 2, band + pixels + FOOTER * args.scale),
            "#F9FAFC",
        )
        figure.paste(webview, (margin, band))
        figure.paste(markview, (margin + panel + gap, band))
        draw = ImageDraw.Draw(figure)
        chinese = args.language == "zh"

        utility = "Noto Sans CJK SC" if chinese else "Noto Sans"
        display = utility if chinese else "Noto Serif"
        caption = font(12 * args.scale, args.language, family=utility)
        heading = font(28 * args.scale, args.language, family=display)
        title = "齐整的边缘，自然的阅读。" if chinese else "A paragraph, in balance."
        note = (
            "相同文字、字体与栏宽，感受段落排版的差异。"
            if chinese
            else "Same text, type and measure. A different rhythm on the page."
        )
        label(draw, heading, margin + INSET_X * args.scale, 27 * args.scale, title)
        draw.text(
            (margin + INSET_X * args.scale, 76 * args.scale),
            note,
            font=caption,
            fill="#687587",
        )
        labels = (
            ("浏览器默认排版", "Markview")
            if chinese
            else ("Browser default", "Markview")
        )
        subtitles = (
            ("左对齐", "两端对齐 · 整段换行")
            if chinese
            else ("Left aligned", "Justified · Paragraph line breaking")
        )
        for index, left in enumerate((margin, margin + panel + gap)):
            x = left + INSET_X * args.scale
            label(
                draw,
                font(17 * args.scale, args.language, family=utility),
                x,
                116 * args.scale,
                labels[index],
            )
            draw.text(
                (x, 143 * args.scale), subtitles[index], font=caption, fill="#687587"
            )
            edge = left + (INSET_X + args.column) * args.scale + 8 * args.scale
            for y in range(
                band + INSET_Y * args.scale,
                band + pixels - FOOT * args.scale,
                6 * args.scale,
            ):
                draw.line((edge, y, edge, y + 2 * args.scale), fill="#BCC9D8", width=1)
        footer = (
            "两端对齐，让右边缘保持齐整，词间距依然自然。"
            if chinese
            else "An even right edge. Comfortable space between words."
        )
        draw.text(
            (margin + INSET_X * args.scale, band + pixels + 15 * args.scale),
            footer,
            font=caption,
            fill="#687587",
        )
        path = pathlib.Path(args.out)
        path.parent.mkdir(parents=True, exist_ok=True)
        figure.save(path, optimize=True)
        print(
            f"{path}: {figure.width}x{figure.height}, "
            f"{args.column}px column, "
            f"{lines[0]} lines in the webview and {lines[1]} in Markview"
        )


if __name__ == "__main__":
    main()
