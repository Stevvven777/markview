# Markview Preview & Export

This development slice provides **Markview: 字体管理…** in the Command Palette.

The manager shows fonts required by reading and export templates, including their
local paths, licenses, and download sources. Add a font directory, download one
missing family or all missing families, cancel a transfer, or retry failed items.
Downloads go to extension storage; they do not install fonts system-wide.
Availability means a matching font family was found, not that every glyph exists.

The bundled native CLI handles catalog resolution and validated downloads.
`markview.fontDirectories` adds local directories and `markview.previewStyleFile`
selects a custom reading template. The manager runs on the local UI host.

Use the M icon in a Markdown editor title to open a side-by-side reading preview.
The preview renders unsaved edits with MVaaC/WASM, follows the editor color theme,
supports selection/copy, local and remote images, links and a visible scrollbar.
Saved reading position is restored when VS Code reloads the window.

Fonts are resolved through the existing native font catalog and local font index.
Only local trusted workspaces are supported.

Right-click a Markdown editor to export PDF or PNG, or choose a built-in or custom MVSS template before exporting.

The preview keeps the edited source position visible after reflow and wrapped input. It scrolls only when that position leaves the viewport; a new reading gesture cancels the movement.

Choose a reading template with **Markview: 选择阅读模板…**. VS Code settings expose font size, column width, justification, hyphenation, paragraph indentation and code wrapping. Custom `.mvss.toml` files must support the `ui` destination and reload when edited.

Template settings inherit from the nearest `.vscode/settings.json`, starting beside the Markdown file and continuing through parent directories, including outside the opened workspace. A directory without template settings inherits from its parent; reading and export choices inherit independently. `previewTemplate`/`previewStyleFile` and `exportTemplate`/`exportStyleFile` each form one choice, so a child preset replaces an inherited custom file. An explicit `"default"` resets inheritance for that destination. Relative stylesheet paths resolve from the directory containing `.vscode`.

```jsonc
{
  "markview.previewTemplate": "celadon",
  "markview.exportStyleFile": "styles/paper.mvss.toml"
}
```

Configuration lookup stops at the filesystem root or an access-denied error, then falls back to VS Code settings and defaults. Untitled documents use VS Code settings. Invalid configurations and missing selected stylesheets are reported rather than skipped. Configuration and stylesheet changes update the preview and font manager automatically.
