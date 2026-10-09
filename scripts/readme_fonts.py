"""Load the requested regional face for README captions and charts."""

import subprocess
import tempfile
from pathlib import Path

from PIL import ImageFont


def font_match(family):
    # `Noto Sans CJK SC` shares a collection with the Japanese default face.
    match = subprocess.check_output(
        ["fc-match", "-f", "%{file}\t%{index}", family], text=True
    ).strip()
    path, _, index = match.partition("\t")
    return path, int(index or 0)


def font(size, language, family=None):
    family = family or ("Noto Sans CJK SC" if language == "zh" else "DejaVu Sans")
    path, index = font_match(family)
    return ImageFont.truetype(path, size, index=index)


def save_chart(figure, destination, language, **kwargs):
    if language != "zh":
        figure.savefig(destination, **kwargs)
        return

    from fontTools.ttLib import TTFont
    from matplotlib.font_manager import weight_dict
    from matplotlib.text import Text

    # Matplotlib cannot select collection faces; give it standalone SC fonts.
    with tempfile.TemporaryDirectory() as directory:
        fonts = {}
        for text in figure.findobj(Text):
            properties = text.get_fontproperties().copy()
            weight = properties.get_weight()
            bold = weight_dict.get(weight, weight) >= 600
            if bold not in fonts:
                family = "Noto Sans CJK SC" + (":weight=bold" if bold else "")
                path, index = font_match(family)
                target = Path(directory) / f"sc-{bold}.otf"
                with TTFont(path, fontNumber=index) as face:
                    face.save(target)
                fonts[bold] = target
            properties.set_file(fonts[bold])
            text.set_fontproperties(properties)
        figure.savefig(destination, **kwargs)
