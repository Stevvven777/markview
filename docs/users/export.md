# Export

[User documentation](README.md) · [Documentation](../README.md)

Markview exports without a browser or a print dialog. In the reader, `Ctrl+E` or
the toolbar's export button opens an export panel: it writes the document to
PDF, or to one PNG of the whole document, and opens the result with the
operating system. **Export and Watch…**, beside it, keeps rewriting the same
file whenever the document is saved. The panel carries its own text size
(12 pt by default), first-line
indent, paper, orientation, margins, PNG scale and stylesheet sequence — the
bundled `print` sheet is layered with whatever the panel selects — all kept
under `[export]` in `settings.toml`. Changing them never reflows the reading
view.

The same exports are on the command line, for scripts and batch runs:

```sh
markview pdf document.md --output document.pdf
markview pdf document.md -o paper.pdf --paper letter --margin 20,25
markview pdf document.md -o paper.pdf --footer "{title} — {page}/{pages}"
markview pdf document.md -o document.pdf --watch
```

The bundled `print` stylesheet supplies the paper: A4 with 20 mm side margins,
black on white, and a centred page number. Body text is 12 pt unless
`--font-size` says otherwise. `--paper` takes `a3`, `a4`, `a5`, `a6`, `b5`,
`letter`, `legal`, `tabloid`, or `WIDTHxHEIGHT` in millimetres; `--margin` takes
one, two, or four millimetres; `--landscape` swaps the sides.
The six header and footer slots are set with `--header`, `--footer` and the
`-left`/`-right` variants, and their templates may use `{page}`, `{pages}`,
`{title}` and `{path}`. PDF commands use `--paper` and `--margin` for page geometry
and `--style` for colors; window dimensions, reading column and reader theme
flags do not apply. Put command options after the subcommand; only `--offline`
is global. The `render` command alone accepts `--scroll`, in logical pixels.

`--watch` keeps the command running after the first export and rebuilds the PDF
whenever the document, or a local image it references, changes; Ctrl+C ends the
session. Every rebuild reuses the unchanged parse, block layout and decoded
images, so an unchanged save is skipped and a small edit pays only for the part
that changed.

A paragraph keeps two lines on each side of a page break, a heading travels with
the block it introduces, code blocks wrap, and a table too wide for the page is
scaled down with a warning on stderr. Web and mail links become clickable
annotations, and a `#heading` link becomes an internal jump.

The PDF information dictionary takes `--title`, `--author` (repeat it for
several authors), `--subject`, `--keywords`, `--language` and `--creator`.
Nothing else is invented, and no creation or modification date is ever written,
which is what keeps two exports of one document byte for byte identical.
