# Markview Preview & Export

`vscode/` contains one desktop extension, `szdytom.markview`. Its preview uses MVaaC in a Webview; PDF/PNG export calls the bundled native CLI. Both features share one font index, download cache and management panel. Export works without opening a preview. Browser and remote-workspace support are outside this release's validation scope.

Build from the repository root with Rust, Python 3.11+, Node and pnpm available:

```sh
cargo build --release --locked
pnpm --dir web install --frozen-lockfile
pnpm --dir web run setup
pnpm --dir web build
npm ci --prefix editors
npm --prefix editors run build
npm --prefix editors run lint
npm --prefix editors test
npm --prefix editors run package
npm --prefix editors run test:host -- --installed
npm --prefix editors run test:host -- --installed --export-only
pnpm --dir web exec playwright install chromium --only-shell
npm --prefix editors run test:browser
```

Packaging copies the current release binary and shared assets. Outputs are in `artifacts/vscode/`. The optional package argument is a VS Code target, such as `darwin-arm64`, `win32-x64` or `linux-x64`; `MARKVIEW_BINARY` can select another matching exporter built for the current host. Build on the target platform; the build step executes the exporter to obtain bundled font metadata. Do not label a binary as another platform merely by changing the target argument.

Install `markview-0.2.0-<platform>.vsix` using **Extensions: Install from VSIX…**. For earlier local trial builds, uninstall `szdytom.markview-preview` and `szdytom.markview-export` to avoid duplicate commands; existing `markview.*` settings remain valid. The new extension has its own font cache; additional font directories remain configured.

Host tests install the VSIX into an isolated VS Code profile, then run their installed bytes through the extension test runner. They exercise unsaved-buffer PDF export, cancellation, font download and reuse, rendered Webview content, edits, first save and disposal. Set `VSCODE_EXECUTABLE` to use an existing VS Code executable. On Linux, use `xvfb-run -a` for host tests. Run `npm --prefix editors run test:host -- --installed --export-only` to verify export activation without a preview.

The native `markview export` command accepts Markdown through stdin, `--base-dir`, `--style`, `--style-file`, repeatable `--fonts` directories or `--font-file` paths, `--format`, `--scale` and `--output`. `--catalog` reports effective font definitions and template choices as JSON. `--download FAMILY --cache DIR` downloads declared sources with JSON progress. A host-created `--cancel-file` marker requests cancellation; the host also enforces a bounded process shutdown.

The local font index is cached by path, size and modification time in the extension's global storage. The preview receives only selected families' files. Downloads reuse Markview's native validation and archive limits. Each family is installed as one directory transaction; its metadata keeps the declared source URLs, checksums and license information. Read the extension README for user-facing settings and behavior.

The CI workflow builds platform VSIX and runs extension-host tests on macOS, Windows and Linux. It uploads build artifacts without publishing the extension.

See [validation results](../docs/vscode-validation.md) for the executed checks, artifacts and platform limits.
