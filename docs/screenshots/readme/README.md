# README images

The English and Chinese READMEs use three compositions: a desktop monitor with
an Android phone and tablet, enlarged text/math and code details, and a rectangular
page joining Light and Dark along a single diagonal. All screen pixels come
from Markview. Device frames and shadows are presentation graphics.

Desktop inputs (`en-*.png`, `zh-*.png`, excluding compositions) show the localized
[`reading.md` examples](../source/en/reading.md) with the same window size,
column width and body text size (1440 × 900 logical pixels, 850 px column,
20 px body text). The README theme page uses Light and Dark; the separate
[theme gallery](../../users/themes.md) shows Light, Dark, Celadon, Rosewood,
Blueprint and 8-bit individually. Capture waits for syntax highlighting before taking the
active window through Spectacle on KDE/Wayland.

The capture configuration copies the live `settings.toml`, disabling only
single-instance forwarding and session restoration. Its font and stylesheet
directories link to the live resources, so downloaded/custom fonts remain
available. Capture runs offline and does not modify the live settings. The Light/Dark split uses matching typography. Other themes retain their own
fonts and layout in separate complete screenshots.

Android inputs (`android-*.png`, `tablet-*.png`) show those same examples shared
to the real app on headless API 35 Pixel 6 and Pixel Tablet emulators. These are
separate captures, not desktop screenshots reshaped into mobile devices. Both
use host Vulkan and pass their layout integration tests before capture. Body
text is 16 px, the tablet reading width is 760 px, and the same downloaded font
resources are available in each app's private `markview/fonts` directory.
System bars remain in the capture.

With Pillow, Fontconfig, DejaVu Sans and Noto Sans CJK installed, recompose:

```sh
python3 scripts/generate_readme_images.py
python3 scripts/test_readme_images.py
```

The localized `*-performance-comparison.png` charts read first-readable-frame
time and total process-group RSS directly from the recorded tables in the
[comparison report](../../developers/comparison.md). They use linear axes starting
at zero and show all four fixtures, measured over three runs on September 22–23,
2026. With Matplotlib and the same fonts installed, regenerate with:

```sh
python3 scripts/generate_readme_comparison.py
```

With a release binary, Spectacle and a graphical session, refresh desktop inputs
and use freshly captured localized Android PNGs:

```sh
python3 scripts/generate_readme_images.py --capture \
  --android-en /path/to/android-en.png --android-zh /path/to/android-zh.png
```

`--binary` selects the reader executable; `--output-directory` selects a working
directory for captures and compositions. Readers launched for capture are
closed after each screenshot. Source captures remain here so compositions can
be reproduced without a desktop or an emulator.

To capture Android directly instead of supplying PNGs, first install and verify
the app with `android/test.py --serial SERIAL --layout phone --layout-only`.
Choose Light and 16 px body text in the emulator's app settings, then run:

```sh
python3 scripts/generate_readme_images.py --android-serial SERIAL
```

This shares each localized example through Android's normal text-sharing path,
returns to the document start and captures the screen using `adb`. Use a
dedicated headless emulator; this changes its open document and reading position.
For a tablet, verify with `--layout tablet --layout-only`, choose a 760 px reading
width and capture with `--tablet-serial SERIAL`. Place its installed fonts in
the app's private `files/markview/fonts` directory before capture. Existing
`tablet-en.png` and `tablet-zh.png` inputs are retained when refreshing only
desktop or phone captures.
