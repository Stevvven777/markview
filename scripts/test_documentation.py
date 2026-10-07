"""Check the documentation's local links and audience entry points."""

import re
import unittest
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
GUIDES = set((ROOT / "docs").glob("*/*.md")) | set(
    (ROOT / "docs/developers/history").glob("*.md")
)
PAGES = GUIDES | {ROOT / name for name in (
    "README.md", "README.zh-cn.md", "CONTRIBUTING.md", "docs/README.md",
    "android/README.md", "web/README.md", "fuzz/README.md",
    "tests/fixtures/fonts/README.md", "crates/markview-web/tests/fonts/README.md",
    ".agents/skills/mvss-theme/SKILL.md", ".agents/skills/markview-release/SKILL.md",
)}


def prose(path):
    return re.sub(r"^```[^\n]*\n.*?^```[^\n]*$", "", path.read_text(), flags=re.M | re.S)


def links(path):
    text = re.sub(r"`+[^`]+`+", "", prose(path))
    return re.findall(r"\]\(([^\s)]+)\)", text) + re.findall(
        r'(?:href|src)="([^"]+)"', text
    )


def anchors(path):
    result = set(re.findall(r'(?:id|name)="([^"]+)"', prose(path)))
    counts = {}
    for title in re.findall(r"^#{1,6}\s+(.+?)\s*#*\s*$", prose(path), re.M):
        title = re.sub(r"\[([^]]+)\]\([^)]+\)", r"\1", title)
        title = re.sub(r"<[^>]+>", "", title).lower()
        slug = re.sub(r"[^\w\s-]", "", title).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        result.add(slug + (f"-{count}" if count else ""))
    return result


class DocumentationTest(unittest.TestCase):
    def test_local_links_images_and_anchors(self):
        for page in sorted(PAGES):
            for link in links(page):
                url = urlsplit(link)
                if url.scheme or url.netloc:
                    continue
                target = (page.parent / unquote(url.path)).resolve() if url.path else page
                with self.subTest(page=str(page.relative_to(ROOT)), link=link):
                    self.assertTrue(target.exists(), f"Missing target: {target}")
                    if target.suffix == ".md" and url.fragment:
                        self.assertIn(unquote(url.fragment), anchors(target))

    def test_audience_indexes_cover_all_guides(self):
        indexes = [ROOT / "docs" / audience / "README.md" for audience in (
            "users", "library", "developers",
        )]
        linked = {
            (page.parent / urlsplit(link).path).resolve()
            for page in indexes for link in links(page)
            if not urlsplit(link).scheme
        }
        self.assertFalse(GUIDES - linked - set(indexes), "Guide missing from audience indexes")
        for index in indexes:
            self.assertIn(index, {
                (ROOT / "docs" / urlsplit(link).path).resolve()
                for link in links(ROOT / "docs/README.md")
            })


if __name__ == "__main__":
    unittest.main()
