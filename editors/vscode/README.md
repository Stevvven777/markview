# Markview Preview

Read Markdown beside your source in VS Code, typeset by the native Markview engine. The extension bundles its engine; a separate Markview installation is not required.

- Live preview of unsaved Markdown, with bidirectional scroll synchronization.
- Native typography, math, tables, syntax highlighting, and images.
- Selection, copying, finding text, and clicking back to the source.
- PDF and PNG export with bundled or custom MVSS templates.
- Colors follow the VS Code theme; layout settings resolve per document.

## Usage

Open a Markdown document and choose **Markview: Open Preview to the Side** from the command palette or the preview icon in the editor title. Edit the source normally; the preview follows the buffer. Select preview text and choose **Markview: Copy Preview Selection** from the context menu or command palette to copy it. Use **Markview: Close Preview** to close it.

Choose **Markview: Export to PDF**, **Markview: Export to PNG**, or **Markview: Export with Template…** to export the current document. Select a destination; successful exports are revealed in your file manager.

## Settings

Settings can be placed in user or workspace `settings.json`, including `[markdown]` overrides.

| Setting | Default | Meaning |
| --- | --- | --- |
| `markview.fontSize` | `18` | Body text size in layout pixels. |
| `markview.columnWidth` | `760` | Reading column width in layout pixels. |
| `markview.justify` | `false` | Justify prose. |
| `markview.hyphenate` | `true` | Allow hyphenation. |
| `markview.paragraphIndent` | `0` | First-line indent in text-size units. |
| `markview.codeblockWrap` | `false` | Wrap long code lines. |
| `markview.scrollSync` | `true` | Synchronize the source and preview. |
| `markview.template` | `""` | Export template name or `.mvss.toml` path. |
| `markview.enginePath` | `""` | Optional developer override for the bundled executable. |

## Templates

Select a bundled template such as `mondrian`, or select a `.mvss.toml` file when exporting. Custom templates travel with the export request and do not need installation. MVSS controls fonts, colors, spacing, paper size, and page decorations; it is not CSS.

See the [MVSS documentation](https://github.com/szdytom/markview/blob/main/docs/stylesheets.md) for the schema and authoring examples.

## Compatibility

This preview requires desktop VS Code and a GPU supported by the bundled engine. Browser editors, Remote-SSH, and development containers are outside the current scope. Platform packages contain their own native executable.

Better markdown PDF is the separate export-only extension. Both extensions share the native Markview engine and template format.
