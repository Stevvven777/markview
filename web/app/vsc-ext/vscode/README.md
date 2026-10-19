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

This slice does not add synchronized scrolling or configurable reading templates.
Fonts are resolved through the existing native font catalog and local font index.
Only local trusted workspaces are supported.

Right-click a Markdown editor to export PDF or PNG, or choose a built-in or custom MVSS template before exporting.
