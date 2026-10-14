# Markview Preview & Export

This development slice provides **Markview: 字体管理…** in the Command Palette.
Preview and document export commands will arrive in subsequent changes.

The manager shows fonts required by reading and export templates, including their
local paths, licenses, and download sources. Add a font directory, download one
missing family or all missing families, cancel a transfer, or retry failed items.
Downloads go to extension storage; they do not install fonts system-wide.
Availability means a matching font family was found, not that every glyph exists.

The bundled native CLI handles catalog resolution and validated downloads.
`markview.fontDirectories` adds local directories and `markview.previewStyleFile`
selects a custom reading template. The manager runs on the local UI host.
