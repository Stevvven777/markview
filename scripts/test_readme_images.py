"""Check that the README composites reproduce from their real captures."""

import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import numpy as np
from generate_readme_comparison import SOURCE, draw, measurements
from generate_readme_images import capture_profile
from generate_readme_showcase import KINDS, SOURCES, extract_web
from PIL import Image, ImageChops
from render_typography_comparison import (
    FOOTER,
    GAP,
    HEADER,
    INSET_X,
    MARGIN,
    prepare_text,
)

ROOT = Path(__file__).resolve().parents[1]
IMAGES = ROOT / "docs/screenshots/readme"


class ReadmeImagesTest(unittest.TestCase):
    def test_web_previews_replay_the_production_extractor(self):
        expected = {
            language: (SOURCES / language / "web-extracted.md").read_text()
            for language in ("en", "zh")
        }
        extract_web()
        for language in ("en", "zh"):
            self.assertEqual(
                (SOURCES / language / "web-extracted.md").read_text(),
                expected[language],
            )
            display = (
                (SOURCES / language / "web.md").read_text().split("\n\n", 1)[1].strip()
            )
            self.assertIn(display, expected[language])

    def test_showcase_sources_and_generated_frames(self):
        for language in ("en", "zh"):
            for kind in KINDS:
                source = (SOURCES / language / f"{kind}.md").read_text()
                self.assertTrue(source.startswith("#"))
                with Image.open(IMAGES / f"{language}-showcase-{kind}.png") as picture:
                    self.assertEqual(picture.size, (1440, 1664))
            technical = (SOURCES / language / "technical.md").read_text()
            self.assertIn("```rust", technical)
            self.assertIn("| `i128`", technical)
            math = (SOURCES / language / "math.md").read_text()
            self.assertGreaterEqual(math.count("$$"), 6)

    def test_plain_text_is_literal_in_both_engines(self):
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            source = work / "example.txt"
            source.write_text(
                "# A *literal* heading <tag> & text\ncontinues here.\n\nAnother paragraph."
            )
            prepare_text(source, work)
            markdown = (work / "document.md").read_text()
            html = (work / "document.html").read_text()
            self.assertIn(r"\# A \*literal\* heading \<tag\>", markdown)
            self.assertIn("&lt;tag&gt; &amp; text continues here.", html)
            self.assertEqual(html.count("<p>"), 2)

    def test_typography_comparison_shows_an_even_right_edge(self):
        for language in ("en", "zh"):
            with Image.open(
                ROOT / f"docs/screenshots/{language}-comparison.png"
            ) as figure:
                scale = 2
                margin, gap = MARGIN * scale, GAP * scale
                panel = (figure.width - gap - 2 * margin) // 2
                spreads = []
                for left in (margin, margin + panel + gap):
                    ink = (
                        np.asarray(
                            figure.crop(
                                (
                                    left + INSET_X * scale,
                                    HEADER * scale,
                                    left + panel - INSET_X * scale,
                                    figure.height - FOOTER * scale,
                                )
                            ).convert("L")
                        )
                        < 128
                    )
                    edges = np.diff(np.r_[False, ink.any(axis=1), False].astype(int))
                    bands = zip(np.where(edges == 1)[0], np.where(edges == -1)[0])
                    right_edges = [
                        np.where(ink[start:end].any(axis=0))[0][-1]
                        for start, end in bands
                    ]
                    spreads.append(
                        np.percentile(right_edges, 90) - np.percentile(right_edges, 25)
                    )
                self.assertLess(spreads[1], 5)
                self.assertGreater(spreads[0], 30)

    def test_comparison_charts_use_recorded_measurements(self):
        opening, memory = measurements(SOURCE.read_text())
        self.assertEqual(len(opening), 4)
        self.assertEqual(len(memory), 4)
        self.assertEqual(opening[0], [0.101, 0.957])
        self.assertEqual(memory[-1], [53.8, 1147.6])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            for language in ("en", "zh"):
                draw(language, output)
                with Image.open(
                    output / f"{language}-performance-comparison.png"
                ) as chart:
                    self.assertEqual(chart.size, (1800, 920))
                    self.assertGreater(len(chart.getcolors(100000)), 100)

    def test_capture_preserves_fonts_and_live_preferences(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            live = root / "config/markview"
            live.mkdir(parents=True)
            settings = (
                "single-instance = true\nrestore-session = true\nfont_size = 18.0\n"
            )
            (live / "settings.toml").write_text(settings)
            (live / "fonts").mkdir()
            (live / "fonts/custom.otf").write_bytes(b"font resource")
            capture = root / "capture"
            capture.mkdir()
            with patch.dict("os.environ", {"XDG_CONFIG_HOME": str(live.parent)}):
                capture_profile(capture)
            self.assertEqual((live / "settings.toml").read_text(), settings)
            copied = (capture / "markview/settings.toml").read_text()
            self.assertIn("single-instance = false", copied)
            self.assertIn("restore-session = false", copied)
            self.assertIn("font_size = 18.0", copied)
            self.assertEqual(
                (capture / "markview/fonts/custom.otf").read_bytes(), b"font resource"
            )

    def test_recompose_from_desktop_and_android_captures(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            for capture in IMAGES.glob("*.png"):
                if not any(
                    kind in capture.stem for kind in ("hero", "details", "themes")
                ):
                    shutil.copyfile(capture, output / capture.name)
            subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "scripts/generate_readme_images.py"),
                    "--output-directory",
                    str(output),
                ],
                check=True,
            )
            for language in ("en", "zh"):
                for kind in ("hero", "details", "themes"):
                    name = f"{language}-{kind}.png"
                    with (
                        self.subTest(image=name),
                        Image.open(IMAGES / name) as expected,
                        Image.open(output / name) as actual,
                    ):
                        self.assertEqual(actual.size, expected.size)
                        self.assertIsNone(
                            ImageChops.difference(actual, expected).getbbox()
                        )


if __name__ == "__main__":
    unittest.main()
