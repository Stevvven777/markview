# Markview Preview & Export

Read Markdown beside your VS Code editor and export the current content to PDF or a full-document PNG. Preview uses MVaaC; export uses the bundled native Markview engine.

- Open a Markdown document and click the **M** editor-title icon, or run **Markview: 打开并排预览**. Unsaved edits appear in the side preview.
- Right-click a Markdown editor to export PDF/PNG, or choose **使用模板导出…**. Export does not require an open preview. Untitled documents are supported; choose a resource directory when relative images need one.
- Preview follows VS Code's theme. Reading options and scroll synchronization are under **Settings → Markview**. Editing follows hidden input with minimal scrolling; manual preview input cancels pending following.
- Use **阅读字体管理…** from the preview's **…** menu for reading fonts, or **Markview: 字体管理…** for export fonts. Inspect matched families, add directories, and download missing fonts individually or in bulk, with cancellation and retry.
- `markview.exportTemplate` selects the direct-export template. `markview.previewStyleFile` accepts an absolute `.mvss.toml` path supporting the `ui` destination.
- Preview tabs restore after **Developer: Reload Window**, including their document connection and reading position. Unsaved text remains managed by VS Code.

Install the VSIX matching your desktop OS and architecture. Rust and a separate CLI installation are not needed. Web VS Code, virtual/untrusted workspaces and remote-workspace operation are not supported or validated by this release.

If upgrading from the two local trial extensions, uninstall them before installing this unified package. Old preview tabs may need a one-time reopen. Font caches from the old extension ID are not automatically migrated; configured additional font directories remain available.
