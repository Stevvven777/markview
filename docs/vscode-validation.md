# VS Code implementation validation

Validated locally on **2026-10-07**, on macOS arm64. The implementation reuses MVaaC's Viewer, FontLoader, resource decoder and ScrollSync, together with Markview's existing PDF, full-document PNG and font-download engines.

## Results

| Check | Result |
| --- | --- |
| Rust library tests | 567 passed; 19 existing tests skipped by their `ignore` attributes. |
| Native export integration | 3 passed: stdin/CRLF text, PDF pagination and embedded images, full PNG across tiles, custom templates, font catalogs, invalid inputs and cancellation preserving existing output. |
| Rust formatting and Clippy | Passed, including all native targets with warnings denied. |
| Editor TypeScript and formatting | Passed. |
| Shared editor and preview host integration | 14 passed: fonts/process/downloads, opening viewport and focus-independent scrolling, feedback suppression, selection preservation, sync toggle, original M-and-lines icons and coalescing fast scroll packets without losing the final position. |
| Production preview browser adapter | 15 passed: selection/copy, navigation/stale requests, continuous text/image mapping, font-loading navigation, settings/theme/images/links, plus pixel-wheel traces, immediate reversals, no extra animation tail, bounded message counts and scrollbar dragging/keyboard/hiding. |
| MVaaC browser regressions | Original implementation: 32 passed across navigation, interaction, resources, stylesheets and fonts. Touchpad update: all 11 interaction checks passed, including immediate fractional wheel deltas and reversals. Chromium used its software graphics backend. |
| Web TypeScript and ScrollSync | Type checking passed; 4 sync tests passed. |
| Installed VSIX host, unified extension | Passed: untitled content/unsaved changes, first-save rebinding/images, PDF/PNG cancellation, font download/embedding/cache, opening viewport and source scrolling in both directions while the preview has focus, unchanged editor selection and disposal. |
| Export without opening preview | Passed in a fresh profile containing only the unified VSIX. |
| VSIX contents | Platform metadata and absence of extension dependencies checked. Packaged native binary matches the release binary and retains executable permissions; preview contains JS, WASM and reused SVG assets. No test modules, source maps or `node_modules` are shipped. |
| CI workflow syntax | Passed `actionlint`. |

Font-download tests serve controlled bytes through a local HTTP proxy. They do not fetch fonts from a public CDN or weaken production URL checks. Installed-host tests install real VSIX packages into fresh profiles, then execute those installed files using VS Code's extension test runner. They leave the user's normal profile untouched.

Scroll regression checks use the production host bundle with VS Code event fixtures, the production Webview with Chromium wheel/keyboard input and screenshot comparisons, and installed VSIX code in a real VS Code window. The host runner disables background rendering throttles so obscured test windows continue loading. Native editor ranges may include VS Code's viewport padding; assertions use the actual visible position rather than assuming `revealRange()` places a requested line at an exact pixel.

Touchpad regression checks replay fractional pixel deltas, fast travel and reversals through the production wheel handler. Each frame must match accumulated input within one logical pixel, remain still after input stops, and emit at most one reading-position message per frame. These are automated input traces, not physical-touchpad measurements. The original preview icon is reused from the local prior extension and checked byte-for-byte in both themes.

## Reproduce

First follow the build instructions in [editors/README.md](../editors/README.md). Then run from the repository root:

```sh
cargo fmt --all --check
cargo clippy --release --all-targets --locked -- -D warnings
cargo test --release --lib
cargo test --release --test vscode_export
pnpm --dir web typecheck
pnpm --dir web test:scroll-sync
pnpm --dir web exec playwright install chromium --only-shell
node web/tests/build-reader.mjs
pnpm --dir web exec playwright test tests/navigation.spec.mjs tests/interaction.spec.mjs tests/resources.spec.mjs tests/stylesheets.spec.mjs tests/font-sets.spec.mjs --project=chromium
npm --prefix editors run lint
npm --prefix editors test
npm --prefix editors run test:browser
npm --prefix editors run package
npm --prefix editors run test:host -- --installed
npm --prefix editors run test:host -- --installed --export-only
```

Set `VSCODE_EXECUTABLE` to an existing VS Code executable to avoid downloading another copy. Linux extension-host tests need `xvfb-run -a`. The [VS Code CI workflow](../.github/workflows/vscode.yml) builds and packages on macOS arm64, Windows x64 and Linux x64, and runs browser and installed-host checks.

## Deliverables and limits

- `artifacts/vscode/markview-0.2.0-darwin-arm64.vsix` contains the unified extension, native CLI and MVaaC WASM.
- Install the single platform VSIX; there is no companion extension or Marketplace dependency.
- Windows and Linux CI paths are provided but were not executed locally. Browser VS Code and remote workspaces are outside this release's validation scope.
- Font status checks family availability and candidate selection; it does not diagnose glyph coverage or guarantee every weight.
- Native PNG export needs a working graphics backend. Browser checks exercised Chromium's software renderer, while the macOS native PNG integration exercised the available native backend.
- Dialog choices and opening a file manager use VS Code's native APIs; automated host checks exercise the export service directly rather than operating native save dialogs.

No commit, push, tag or publication was performed.

## Preview startup and reading surface follow-up

Removed the preview's internal toolbar; reading settings, sync toggle and font management remain available through VS Code commands. Side padding adapts to the pane width, in addition to MVaaC's internal text margins. Browser checks cover 360, 640 and 1280 pixel panes.

The cached system font scan took about 22 ms in the local trial. The larger avoidable cost was eagerly sending the 183 MiB Apple Color Emoji collection into the viewer even for ordinary text. The preview now requests it only when source characters or HTML entities may need it; live edits update the requested font set. Export and font-management catalogs stay complete.

Installed VSIX host verification measured 286,401,704 bytes of font input before filtering and 94,281,244 bytes for ordinary Chinese/English text, a 67% reduction. This measures font input, not end-to-end launch latency. TypeScript/formatting, 10 editor checks, 6 browser checks and the installed VSIX integration pass. The earlier two-package build was checked against its packaged artifacts; the final unified package is validated separately.

See [the startup, update and scrolling analysis](vscode-performance.md) for stage timings, the stale trial-process diagnosis and remaining optimization work.

Margin regression checks now inject VS Code’s default `body { padding: 0 20px; }`, verify equal canvas insets and no horizontal overflow at 360/640/1280 px. The command palette exposes `Markview: 阅读字体管理…` for UI fonts and `Markview: 字体管理…` for export templates.

The shared graphical font manager reuses the font index/download service. Two browser integration checks exercise the production host/controller and UI for state, progress, failed-download retry, cancellation/disposal, template switching, directory configuration and escaped font names. Installed-host verification opens and closes the actual packaged management page.

A Van Gogh trial reader variant declares both UI/PDF targets. Browser checks verify its gold rules in the production renderer; installed-host tests exercise custom reader stylesheets and template font resolution. Trial PDF/PNG exports use the bundled PDF Van Gogh template.

Bulk font download browser checks cover family deduplication, continuing after individual failures, retrying only remaining missing fonts, reporting unavailable sources, and cancellation before the next download. Preview focus checks retain keyboard focus while suppressing the canvas outline.

Edit-follow checks (2026-10-08) cover input after a long code block, no movement for visible input, cancellation by preview gestures, sync disabled, rapid version replacement and simulated Chinese composition replacements. A 500-line code fixture measured browser update-to-render medians of approximately 16.6 ms without follow and 16.7 ms with follow (12 updates each); this excludes VS Code transport and does not substitute for physical IME testing.

A narrow-preview wrapped-code regression checks scrolling while updates are still arriving, as well as the final input position. The old animation-reset behavior fails this check; retaining animation progress while replacing its destination passes.

Window reload validation (2026-10-08): host fixtures restore the original panel and reconnect document updates; browser tests persist document URI, resource base and reading offset without copying Markdown. An isolated normal VS Code profile with the installed VSIX executed `workbench.action.reloadWindow`: the same preview restored automatically, dirty text survived, later edits rendered and no duplicate preview tab appeared. Extension-development/test mode was not used for the dirty-document assertion because it did not retain the backup in this environment. Legacy panels created before state persistence may need a one-time reopen after upgrading.

The final package is **Markview Preview & Export** (`szdytom.markview`), with one activation entry and one `Services` instance. Export-only tests use this same package without opening a preview. Earlier timings and feature-specific checks above were collected before the final packaging consolidation unless otherwise stated.

Final unified-package validation on 2026-10-08: native/WASM rebuilt from the current tree; editor lint and 14 host/unit checks, 15 browser checks, 11 MVaaC interaction checks and 4 ScrollSync checks passed. The 22.08 MiB macOS arm64 VSIX passed installed-host tests with and without a preview, plus a normal-profile reload with dirty text. Packaged native/WASM bytes were compared to current build outputs; source/test files, source maps and dependencies directories are excluded. Windows/Linux remain CI targets, not locally verified results.
